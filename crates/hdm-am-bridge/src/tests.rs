//! In-crate handler tests. They drive the real `axum` router via `oneshot` (no socket) and
//! substitute a fake [`Device`], so envelope parsing, config merge, auth, CORS/PNA, and error
//! mapping are exercised without a real HDM or any crypto.

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use tokio::sync::Semaphore;
use tower::ServiceExt as _;

use hdm_am::{
    CashInOutRequest, DateTimeResponse, EmptyResponse, Error, FiscalReportRequest,
    GetReturnableReceiptRequest, ListOpsAndDepsResponse, PaymentSystemsListResponse,
    PrintReceiptRequest, PrintReturnReceiptRequest, ReceiptResponse, ReturnReceiptResponse,
    ReturnableReceiptResponse, ServerErrorKind, SetupHeaderFooterRequest, SetupHeaderLogoRequest,
    SingleEmarkRequest,
};

use crate::config::{BridgeConfig, PartialConn, PasswordConn, SessionConn};
use crate::device::Device;
use crate::routes::{AppState, app};

const TOKEN: &str = "secret-token";

#[derive(Clone, Copy)]
enum Fail {
    NotLoggedIn,
    Server(u16),
}

impl Fail {
    fn into_error(self) -> Error {
        match self {
            Self::NotLoggedIn => Error::NotLoggedIn,
            Self::Server(code) => Error::Server {
                code,
                kind: ServerErrorKind::from_code(code),
            },
        }
    }
}

/// A test double. Records the session connections it sees; either succeeds (empty results) or
/// fails with a configured error. Operations whose success type is `#[non_exhaustive]` are only
/// reached on the failure path, so they never need to construct one.
struct FakeDevice {
    fail: Option<Fail>,
    seen: Arc<Mutex<Vec<(String, u32)>>>,
}

impl FakeDevice {
    fn empty(&self) -> Result<EmptyResponse, Error> {
        self.fail
            .map_or_else(|| Ok(EmptyResponse::default()), |f| Err(f.into_error()))
    }

    fn deny<T>(&self) -> Result<T, Error> {
        Err(self.fail.unwrap_or(Fail::NotLoggedIn).into_error())
    }
}

impl Device for FakeDevice {
    fn operators(&self, _conn: &PasswordConn) -> Result<ListOpsAndDepsResponse, Error> {
        self.deny()
    }
    fn verify_login(&self, conn: &SessionConn) -> Result<(), Error> {
        self.seen
            .lock()
            .expect("lock")
            .push((conn.endpoint.host.clone(), conn.cashier));
        self.fail.map_or(Ok(()), |f| Err(f.into_error()))
    }
    fn print_receipt(
        &self,
        _conn: &SessionConn,
        _req: PrintReceiptRequest,
    ) -> Result<ReceiptResponse, Error> {
        self.deny()
    }
    fn print_last_receipt(&self, _conn: &SessionConn) -> Result<EmptyResponse, Error> {
        self.empty()
    }
    fn lookup_receipt(
        &self,
        _conn: &SessionConn,
        _req: GetReturnableReceiptRequest,
    ) -> Result<ReturnableReceiptResponse, Error> {
        self.deny()
    }
    fn print_return(
        &self,
        _conn: &SessionConn,
        _req: PrintReturnReceiptRequest,
    ) -> Result<ReturnReceiptResponse, Error> {
        self.deny()
    }
    fn fiscal_report(
        &self,
        _conn: &SessionConn,
        _req: FiscalReportRequest,
    ) -> Result<EmptyResponse, Error> {
        self.empty()
    }
    fn cash_in_out(
        &self,
        _conn: &SessionConn,
        _req: CashInOutRequest,
    ) -> Result<EmptyResponse, Error> {
        self.empty()
    }
    fn date_time(&self, _conn: &SessionConn) -> Result<DateTimeResponse, Error> {
        self.deny()
    }
    fn time_sync(&self, _conn: &SessionConn) -> Result<EmptyResponse, Error> {
        self.empty()
    }
    fn payment_systems(&self, _conn: &SessionConn) -> Result<PaymentSystemsListResponse, Error> {
        self.deny()
    }
    fn single_emark(
        &self,
        _conn: &SessionConn,
        _req: SingleEmarkRequest,
    ) -> Result<EmptyResponse, Error> {
        self.empty()
    }
    fn receipt_sample(&self, _conn: &SessionConn) -> Result<EmptyResponse, Error> {
        self.empty()
    }
    fn header_footer(
        &self,
        _conn: &SessionConn,
        _req: SetupHeaderFooterRequest,
    ) -> Result<EmptyResponse, Error> {
        self.empty()
    }
    fn header_logo(
        &self,
        _conn: &SessionConn,
        _req: SetupHeaderLogoRequest,
    ) -> Result<EmptyResponse, Error> {
        self.empty()
    }
}

fn full_default() -> PartialConn {
    PartialConn {
        host: Some("10.0.0.5".into()),
        port: Some(1025),
        password: Some("pw".into()),
        cashier: Some(3),
        pin: Some("1234".into()),
        timeout_secs: Some(50),
    }
}

struct Harness {
    router: axum::Router,
    seen: Arc<Mutex<Vec<(String, u32)>>>,
}

fn harness(default_conn: PartialConn, fail: Option<Fail>) -> Harness {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let device = FakeDevice {
        fail,
        seen: Arc::clone(&seen),
    };
    let config = BridgeConfig {
        bind: "127.0.0.1:0".parse().expect("addr"),
        token: Some(TOKEN.into()),
        insecure_no_auth: false,
        allow_origins: vec!["http://shop.example".into()],
        default_conn,
    };
    let state = AppState {
        config: Arc::new(config),
        device: Arc::new(device),
        device_lock: Arc::new(Semaphore::new(1)),
    };
    Harness {
        router: app(state),
        seen,
    }
}

fn post(uri: &str, token: Option<&str>, body: &str) -> Request<Body> {
    let mut b = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::from(body.to_owned())).expect("request")
}

async fn send(router: axum::Router, req: Request<Body>) -> (StatusCode, Value) {
    let resp = router.oneshot(req).await.expect("response");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("body");
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

#[tokio::test]
async fn health_is_public() {
    let h = harness(full_default(), None);
    let req = Request::builder()
        .uri("/v1/health")
        .body(Body::empty())
        .expect("req");
    let (status, body) = send(h.router, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn protected_route_rejects_missing_and_wrong_token() {
    let h = harness(full_default(), None);
    let (status, body) = send(h.router, post("/v1/sample", None, "{}")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["kind"], "unauthorized");

    let h = harness(full_default(), None);
    let (status, _) = send(h.router, post("/v1/sample", Some("wrong"), "{}")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn login_uses_configured_default_device() {
    let h = harness(full_default(), None);
    let (status, body) = send(h.router, post("/v1/login", Some(TOKEN), "{}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], true);
    let seen = h.seen.lock().expect("lock").clone();
    assert_eq!(seen.as_slice(), &[("10.0.0.5".to_owned(), 3)]);
}

#[tokio::test]
async fn per_request_connection_override_wins() {
    let h = harness(full_default(), None);
    let body = r#"{"connection":{"host":"192.168.1.4","cashier":9}}"#;
    let (status, _) = send(h.router, post("/v1/login", Some(TOKEN), body)).await;
    assert_eq!(status, StatusCode::OK);
    let seen = h.seen.lock().expect("lock").clone();
    // host overridden, cashier overridden, password/pin fall back to the configured default.
    assert_eq!(seen.as_slice(), &[("192.168.1.4".to_owned(), 9)]);
}

#[tokio::test]
async fn missing_connection_fields_yield_400_listing_them() {
    let h = harness(PartialConn::default(), None);
    let (status, body) = send(h.router, post("/v1/login", Some(TOKEN), "{}")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let msg = body["error"]["message"].as_str().expect("message");
    for field in ["host", "password", "cashier", "pin"] {
        assert!(msg.contains(field), "expected {field} in {msg}");
    }
}

#[tokio::test]
async fn device_server_error_maps_to_422_with_code() {
    let h = harness(full_default(), Some(Fail::Server(174)));
    let (status, body) = send(h.router, post("/v1/sample", Some(TOKEN), "{}")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["kind"], "device_error");
    assert_eq!(body["error"]["code"], 174);
}

#[tokio::test]
async fn not_logged_in_maps_to_409() {
    let h = harness(full_default(), Some(Fail::NotLoggedIn));
    let (status, body) = send(h.router, post("/v1/sample", Some(TOKEN), "{}")).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["kind"], "not_logged_in");
}

#[tokio::test]
async fn malformed_json_yields_400() {
    let h = harness(full_default(), None);
    let (status, body) = send(h.router, post("/v1/sample", Some(TOKEN), "{not json")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["kind"], "bad_request");
}

#[tokio::test]
async fn unknown_envelope_field_yields_400() {
    let h = harness(full_default(), None);
    let (status, _) = send(h.router, post("/v1/sample", Some(TOKEN), r#"{"oops":1}"#)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn missing_params_yields_400() {
    // /v1/receipt needs params; an empty envelope must be rejected, not sent to the device.
    let h = harness(full_default(), None);
    let (status, body) = send(h.router, post("/v1/receipt", Some(TOKEN), "{}")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let msg = body["error"]["message"].as_str().expect("message");
    assert!(msg.contains("params"), "expected params hint in {msg}");
}

#[tokio::test]
async fn cors_preflight_allows_configured_origin_and_private_network() {
    let h = harness(full_default(), None);
    let req = Request::builder()
        .method("OPTIONS")
        .uri("/v1/receipt")
        .header("origin", "http://shop.example")
        .header("access-control-request-method", "POST")
        .header("access-control-request-private-network", "true")
        .body(Body::empty())
        .expect("req");
    let resp = h.router.oneshot(req).await.expect("response");
    let headers = resp.headers();
    assert_eq!(
        headers
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("http://shop.example")
    );
    assert_eq!(
        headers
            .get("access-control-allow-private-network")
            .and_then(|v| v.to_str().ok()),
        Some("true")
    );
}

// ---------------- OpenAPI document ----------------

#[cfg(feature = "schema")]
mod openapi {
    use super::{TOKEN, full_default, harness, post, send};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::Value;

    /// Every operation the document describes must resolve to a real route (no 404), so the
    /// document can't advertise an endpoint the bridge doesn't serve.
    #[tokio::test]
    async fn documents_every_route() {
        for path in crate::openapi::operation_paths() {
            let h = harness(full_default(), None);
            let (status, _) = send(h.router, post(path, Some(TOKEN), "{}")).await;
            assert_ne!(
                status,
                StatusCode::NOT_FOUND,
                "documented operation {path} is not routed"
            );
        }
        assert_eq!(
            crate::openapi::operation_paths().len(),
            15,
            "operation count drifted from the 15 protocol operations"
        );
    }

    /// The served document and the embedded copy must be byte-identical, so `/v1/openapi.json`
    /// can't drift from what `dump-openapi` generates.
    #[tokio::test]
    async fn served_document_matches_generated() {
        let h = harness(full_default(), None);
        let req = Request::builder()
            .uri("/v1/openapi.json")
            .body(Body::empty())
            .expect("req");
        let (status, served) = send(h.router, req).await;
        assert_eq!(status, StatusCode::OK);

        let generated = crate::openapi::document(env!("CARGO_PKG_VERSION"));
        assert_eq!(
            served, generated,
            "served /v1/openapi.json drifted from document()"
        );
    }

    /// Structural sanity: correct version, every `$ref` resolves, and every operation carries a
    /// request body, a `200`, and the error envelope.
    #[test]
    fn document_is_structurally_valid() {
        let doc = crate::openapi::document(env!("CARGO_PKG_VERSION"));

        assert_eq!(doc["openapi"], "3.1.0");

        let schemas = doc["components"]["schemas"]
            .as_object()
            .expect("components.schemas object");
        assert!(!schemas.is_empty());

        // Every `$ref` must point at an existing component schema.
        let mut refs = Vec::new();
        collect_refs(&doc, &mut refs);
        assert!(!refs.is_empty(), "expected the document to use $ref");
        for r in &refs {
            let name = r
                .strip_prefix("#/components/schemas/")
                .unwrap_or_else(|| panic!("unexpected $ref target: {r}"));
            assert!(schemas.contains_key(name), "dangling $ref: {r}");
        }

        // Every protected operation has a request body, a 200, and the default error response.
        for path in crate::openapi::operation_paths() {
            let op = &doc["paths"][path]["post"];
            assert!(op.is_object(), "missing POST item for {path}");
            assert!(op["requestBody"].is_object(), "{path} missing requestBody");
            assert!(op["responses"]["200"].is_object(), "{path} missing 200");
            assert!(
                op["responses"]["default"].is_object(),
                "{path} missing error response"
            );
            assert!(op["security"].is_array(), "{path} missing security");
        }
    }

    fn collect_refs(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    if key == "$ref" {
                        if let Some(s) = child.as_str() {
                            out.push(s.to_owned());
                        }
                    } else {
                        collect_refs(child, out);
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|item| collect_refs(item, out)),
            _ => {}
        }
    }
}
