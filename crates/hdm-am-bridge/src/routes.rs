//! HTTP surface: shared state, the request envelope, auth/CORS/PNA wiring, and one handler per
//! operation. Handlers resolve the connection, then run the blocking device call on a worker thread
//! while holding a single-permit semaphore so only one HDM session is ever in flight.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{FromRequest, Request, State};
use axum::http::HeaderValue;
use axum::http::Method;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde::de::{DeserializeOwned, IgnoredAny};
use tokio::sync::Semaphore;
use tower_http::cors::CorsLayer;

use hdm_am::{
    CashInOutRequest, DateTimeResponse, EmptyResponse, FiscalReportRequest,
    GetReturnableReceiptRequest, HdmIdentity, ListOpsAndDepsResponse, PaymentSystemsListResponse,
    PrintReceiptRequest, PrintReturnReceiptRequest, ReceiptResponse, ReturnReceiptResponse,
    ReturnableReceiptResponse, SetupHeaderFooterRequest, SetupHeaderLogoRequest,
    SingleEmarkRequest,
};

use crate::config::{BridgeConfig, PartialConn};
use crate::device::Device;
use crate::error::ApiError;

/// Shared, cheaply-cloneable handler state.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<BridgeConfig>,
    pub device: Arc<dyn Device>,
    /// One permit: serializes device access (the HDM holds one session at a time).
    pub device_lock: Arc<Semaphore>,
}

impl AppState {
    /// Build state around the production TCP device.
    pub fn new(config: BridgeConfig) -> Self {
        Self {
            config: Arc::new(config),
            device: Arc::new(crate::device::TcpDevice),
            device_lock: Arc::new(Semaphore::new(1)),
        }
    }
}

/// The uniform request body: an optional per-request connection override plus optional operation
/// params. Doubles as its own `FromRequest` extractor so malformed bodies become a `400` envelope.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload<P> {
    connection: Option<PartialConn>,
    params: Option<P>,
}

impl<S, P> FromRequest<S> for Payload<P>
where
    S: Send + Sync,
    P: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = Bytes::from_request(req, state)
            .await
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;
        if bytes.is_empty() {
            // No body is a valid call for ops that need no params and a configured default device.
            return Ok(Self {
                connection: None,
                params: None,
            });
        }
        serde_json::from_slice(&bytes)
            .map_err(|e| ApiError::BadRequest(format!("invalid JSON body: {e}")))
    }
}

fn require_params<P>(params: Option<P>, name: &str) -> Result<P, ApiError> {
    params.ok_or_else(|| ApiError::BadRequest(format!("missing \"params\" ({name})")))
}

/// Acquire the device permit, run the blocking session on a worker thread, and JSON-encode the
/// result. The permit is held for the whole device round-trip so sessions never overlap.
async fn run_blocking<R, F>(state: &AppState, f: F) -> Result<Json<R>, ApiError>
where
    F: FnOnce(&dyn Device) -> Result<R, hdm_am::Error> + Send + 'static,
    R: serde::Serialize + Send + 'static,
{
    let permit = Arc::clone(&state.device_lock)
        .acquire_owned()
        .await
        .map_err(|_| ApiError::Internal("device semaphore closed".to_owned()))?;
    let device = Arc::clone(&state.device);
    let outcome = tokio::task::spawn_blocking(move || {
        let _permit = permit; // held until the blocking call returns
        f(device.as_ref())
    })
    .await
    .map_err(|err| ApiError::Internal(format!("device worker task failed: {err}")))?;
    Ok(Json(outcome?))
}

// ---------------- Auth ----------------

/// Length-aware constant-time comparison so token checks don't leak via timing.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    diff == 0
}

async fn require_token(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if state.config.token.is_none() && state.config.insecure_no_auth {
        return Ok(next.run(req).await);
    }
    // serve() refuses to start with no token unless --insecure-no-auth, so `expected` is Some here.
    let Some(expected) = state.config.token.as_deref() else {
        return Err(ApiError::Unauthorized);
    };
    let provided = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));
    match provided {
        Some(token) if constant_time_eq(expected, token) => Ok(next.run(req).await),
        _ => Err(ApiError::Unauthorized),
    }
}

// ---------------- CORS ----------------

fn build_cors(config: &BridgeConfig) -> CorsLayer {
    let mut origins = Vec::new();
    for raw in &config.allow_origins {
        match raw.parse::<HeaderValue>() {
            Ok(value) => origins.push(value),
            Err(_) => log::warn!("hdm-bridge: ignoring unparseable allow-origin {raw:?}"),
        }
    }
    CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE])
        .allow_origin(origins)
        // Answer Chrome's Private Network Access preflight for public-page -> localhost calls.
        .allow_private_network(true)
}

/// Assemble the router: `/v1/health` is public; everything else sits behind the bearer-token layer;
/// CORS (incl. the PNA preflight header) wraps the whole thing.
pub fn app(state: AppState) -> Router {
    let cors = build_cors(&state.config);
    let protected = Router::new()
        .route("/v1/info", get(info))
        .route("/v1/probe", post(probe))
        .route("/v1/operators", post(operators))
        .route("/v1/login", post(login))
        .route("/v1/receipt", post(receipt))
        .route("/v1/receipt/last", post(receipt_last))
        .route("/v1/receipt/lookup", post(lookup_receipt))
        .route("/v1/return", post(print_return))
        .route("/v1/report", post(report))
        .route("/v1/cash", post(cash))
        .route("/v1/datetime", post(datetime))
        .route("/v1/time-sync", post(time_sync))
        .route("/v1/payment-systems", post(payment_systems))
        .route("/v1/emark", post(emark))
        .route("/v1/sample", post(sample))
        .route("/v1/header-footer", post(header_footer))
        .route("/v1/logo", post(header_logo))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_token));

    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/openapi.json", get(openapi_json))
        .route("/docs", get(docs_ui))
        .merge(protected)
        .layer(cors)
        .with_state(state)
}

/// The committed `OpenAPI` 3.1 document, embedded at build time. Kept in sync with the route surface
/// by `examples/dump-openapi.rs` and the CI `--check` gate, so the served bytes always match the
/// types the handlers use. Served publicly so client generators can read it off a running bridge.
const OPENAPI_JSON: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/openapi.json"));

/// Minimal Scalar-based API explorer that renders [`OPENAPI_JSON`] from `/v1/openapi.json`.
const DOCS_HTML: &str = include_str!("docs.html");

async fn openapi_json() -> Response {
    (
        [(CONTENT_TYPE, HeaderValue::from_static("application/json"))],
        OPENAPI_JSON,
    )
        .into_response()
}

async fn docs_ui() -> Response {
    (
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        )],
        DOCS_HTML,
    )
        .into_response()
}

// ---------------- Meta ----------------

/// Liveness response for `GET /v1/health`.
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct HealthOk {
    /// Always `"ok"`.
    status: &'static str,
}

/// Boolean-outcome response (e.g. login confirmation).
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct StatusOk {
    /// Whether the operation succeeded.
    ok: bool,
}

/// Bridge metadata for `GET /v1/info`.
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Info {
    /// Server name (`hdm-bridge`).
    name: &'static str,
    /// Bridge crate version.
    version: &'static str,
    /// HDM specification version the underlying client targets.
    spec_version: &'static str,
    /// Whether a default device connection is configured (so requests may omit `connection`).
    default_device_configured: bool,
    /// The operation names the bridge exposes.
    operations: &'static [&'static str],
}

async fn health() -> Json<HealthOk> {
    Json(HealthOk { status: "ok" })
}

async fn info(State(state): State<AppState>) -> Json<Info> {
    Json(Info {
        name: "hdm-bridge",
        version: env!("CARGO_PKG_VERSION"),
        spec_version: hdm_am::SPEC_VERSION,
        default_device_configured: state.config.default_conn.host.is_some(),
        operations: &[
            "probe",
            "operators",
            "login",
            "receipt",
            "receipt/last",
            "receipt/lookup",
            "return",
            "report",
            "cash",
            "datetime",
            "time-sync",
            "payment-systems",
            "emark",
            "sample",
            "header-footer",
            "logo",
        ],
    })
}

// ---------------- Operation handlers ----------------

async fn probe(
    State(state): State<AppState>,
    body: Payload<IgnoredAny>,
) -> Result<Json<HdmIdentity>, ApiError> {
    let conn = state.config.resolve_endpoint(body.connection)?;
    run_blocking(&state, move |dev| dev.probe(&conn)).await
}

async fn operators(
    State(state): State<AppState>,
    body: Payload<IgnoredAny>,
) -> Result<Json<ListOpsAndDepsResponse>, ApiError> {
    let conn = state.config.resolve_password(body.connection)?;
    run_blocking(&state, move |dev| dev.operators(&conn)).await
}

async fn login(
    State(state): State<AppState>,
    body: Payload<IgnoredAny>,
) -> Result<Json<StatusOk>, ApiError> {
    let conn = state.config.resolve_session(body.connection)?;
    run_blocking(&state, move |dev| {
        dev.verify_login(&conn).map(|()| StatusOk { ok: true })
    })
    .await
}

async fn receipt(
    State(state): State<AppState>,
    body: Payload<PrintReceiptRequest>,
) -> Result<Json<ReceiptResponse>, ApiError> {
    let conn = state.config.resolve_session(body.connection)?;
    let params = require_params(body.params, "PrintReceiptRequest")?;
    run_blocking(&state, move |dev| dev.print_receipt(&conn, params)).await
}

async fn receipt_last(
    State(state): State<AppState>,
    body: Payload<IgnoredAny>,
) -> Result<Json<EmptyResponse>, ApiError> {
    let conn = state.config.resolve_session(body.connection)?;
    run_blocking(&state, move |dev| dev.print_last_receipt(&conn)).await
}

async fn lookup_receipt(
    State(state): State<AppState>,
    body: Payload<GetReturnableReceiptRequest>,
) -> Result<Json<ReturnableReceiptResponse>, ApiError> {
    let conn = state.config.resolve_session(body.connection)?;
    let params = require_params(body.params, "GetReturnableReceiptRequest")?;
    run_blocking(&state, move |dev| dev.lookup_receipt(&conn, params)).await
}

async fn print_return(
    State(state): State<AppState>,
    body: Payload<PrintReturnReceiptRequest>,
) -> Result<Json<ReturnReceiptResponse>, ApiError> {
    let conn = state.config.resolve_session(body.connection)?;
    let params = require_params(body.params, "PrintReturnReceiptRequest")?;
    run_blocking(&state, move |dev| dev.print_return(&conn, params)).await
}

async fn report(
    State(state): State<AppState>,
    body: Payload<FiscalReportRequest>,
) -> Result<Json<EmptyResponse>, ApiError> {
    let conn = state.config.resolve_session(body.connection)?;
    let params = require_params(body.params, "FiscalReportRequest")?;
    run_blocking(&state, move |dev| dev.fiscal_report(&conn, params)).await
}

async fn cash(
    State(state): State<AppState>,
    body: Payload<CashInOutRequest>,
) -> Result<Json<EmptyResponse>, ApiError> {
    let conn = state.config.resolve_session(body.connection)?;
    let params = require_params(body.params, "CashInOutRequest")?;
    run_blocking(&state, move |dev| dev.cash_in_out(&conn, params)).await
}

async fn datetime(
    State(state): State<AppState>,
    body: Payload<IgnoredAny>,
) -> Result<Json<DateTimeResponse>, ApiError> {
    let conn = state.config.resolve_session(body.connection)?;
    run_blocking(&state, move |dev| dev.date_time(&conn)).await
}

async fn time_sync(
    State(state): State<AppState>,
    body: Payload<IgnoredAny>,
) -> Result<Json<EmptyResponse>, ApiError> {
    let conn = state.config.resolve_session(body.connection)?;
    run_blocking(&state, move |dev| dev.time_sync(&conn)).await
}

async fn payment_systems(
    State(state): State<AppState>,
    body: Payload<IgnoredAny>,
) -> Result<Json<PaymentSystemsListResponse>, ApiError> {
    let conn = state.config.resolve_session(body.connection)?;
    run_blocking(&state, move |dev| dev.payment_systems(&conn)).await
}

async fn emark(
    State(state): State<AppState>,
    body: Payload<SingleEmarkRequest>,
) -> Result<Json<EmptyResponse>, ApiError> {
    let conn = state.config.resolve_session(body.connection)?;
    let params = require_params(body.params, "SingleEmarkRequest")?;
    run_blocking(&state, move |dev| dev.single_emark(&conn, params)).await
}

async fn sample(
    State(state): State<AppState>,
    body: Payload<IgnoredAny>,
) -> Result<Json<EmptyResponse>, ApiError> {
    let conn = state.config.resolve_session(body.connection)?;
    run_blocking(&state, move |dev| dev.receipt_sample(&conn)).await
}

async fn header_footer(
    State(state): State<AppState>,
    body: Payload<SetupHeaderFooterRequest>,
) -> Result<Json<EmptyResponse>, ApiError> {
    let conn = state.config.resolve_session(body.connection)?;
    let params = require_params(body.params, "SetupHeaderFooterRequest")?;
    run_blocking(&state, move |dev| dev.header_footer(&conn, params)).await
}

async fn header_logo(
    State(state): State<AppState>,
    body: Payload<SetupHeaderLogoRequest>,
) -> Result<Json<EmptyResponse>, ApiError> {
    let conn = state.config.resolve_session(body.connection)?;
    let params = require_params(body.params, "SetupHeaderLogoRequest")?;
    run_blocking(&state, move |dev| dev.header_logo(&conn, params)).await
}
