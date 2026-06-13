//! The bridge's HTTP error type and its mapping from [`hdm_am::Error`].
//!
//! Every failure renders as the same JSON envelope so callers can branch on a stable shape:
//!
//! ```jsonc
//! { "error": { "kind": "...", "code": 174, "message": "...",
//!              "retryable": false, "requires_relogin": false, "requires_reconnect": false } }
//! ```

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::config::ResolveError;

/// A request-handling failure, rendered to a status code and the error envelope.
pub enum ApiError {
    /// Malformed body, unknown fields, or unresolvable connection.
    BadRequest(String),
    /// Missing or wrong bearer token.
    Unauthorized,
    /// A failure talking to the device.
    Device(hdm_am::Error),
    /// An unexpected internal failure (e.g. a worker thread panicked).
    Internal(String),
}

impl ApiError {
    fn status_kind_code(&self) -> (StatusCode, &'static str, Option<u16>) {
        match self {
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request", None),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized", None),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal", None),
            Self::Device(err) => device_status_kind_code(err),
        }
    }

    fn message(&self) -> String {
        match self {
            Self::BadRequest(m) | Self::Internal(m) => m.clone(),
            Self::Unauthorized => "missing or invalid bearer token".to_owned(),
            Self::Device(err) => err.to_string(),
        }
    }
}

/// Map a device error to its HTTP status, stable `kind` tag, and the vendor code when present.
fn device_status_kind_code(err: &hdm_am::Error) -> (StatusCode, &'static str, Option<u16>) {
    use hdm_am::Error as E;
    match err {
        E::Transport(io) if io.kind() == std::io::ErrorKind::TimedOut => {
            (StatusCode::GATEWAY_TIMEOUT, "transport_timeout", None)
        }
        E::Transport(_) => (StatusCode::BAD_GATEWAY, "transport", None),
        // The device spoke the protocol and rejected the request — a 4xx-class outcome carrying the
        // spec/vendor code so the caller can branch on it.
        E::Server { code, .. } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "device_error",
            Some(*code),
        ),
        E::Crypto(_) => (StatusCode::BAD_GATEWAY, "crypto", None),
        E::Decode(_) => (StatusCode::BAD_GATEWAY, "decode", None),
        E::Encode(_) => (StatusCode::INTERNAL_SERVER_ERROR, "encode", None),
        E::NotLoggedIn => (StatusCode::CONFLICT, "not_logged_in", None),
        E::PayloadTooLarge { .. } => (StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large", None),
        E::NotHdm { .. } => (StatusCode::BAD_GATEWAY, "not_hdm", None),
        // `Error` is #[non_exhaustive]; treat anything new as an upstream/device fault.
        _ => (StatusCode::BAD_GATEWAY, "device", None),
    }
}

/// The uniform error envelope every failure renders to. Public within the crate so the `OpenAPI`
/// generator can derive its schema from the same type the handlers serialize.
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ErrorBody {
    /// The single error detail object.
    error: ErrorDetail,
}

/// The error detail carried inside [`ErrorBody`].
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ErrorDetail {
    /// Stable machine-readable error tag (e.g. `device_error`, `transport_timeout`).
    kind: &'static str,
    /// Spec/vendor response code when the device rejected the request; absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<u16>,
    /// Human-readable message. Not stable; do not branch on it.
    message: String,
    /// Whether retrying the same request may succeed.
    retryable: bool,
    /// Whether the caller must re-run the login operation before retrying.
    requires_relogin: bool,
    /// Whether the caller must re-establish the device connection before retrying.
    requires_reconnect: bool,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, kind, code) = self.status_kind_code();
        let (retryable, requires_relogin, requires_reconnect) = match &self {
            Self::Device(err) => (
                err.is_retryable(),
                err.requires_relogin(),
                err.requires_reconnect(),
            ),
            _ => (false, false, false),
        };
        let body = ErrorBody {
            error: ErrorDetail {
                kind,
                code,
                message: self.message(),
                retryable,
                requires_relogin,
                requires_reconnect,
            },
        };
        (status, Json(body)).into_response()
    }
}

impl From<hdm_am::Error> for ApiError {
    fn from(err: hdm_am::Error) -> Self {
        Self::Device(err)
    }
}

impl From<ResolveError> for ApiError {
    fn from(err: ResolveError) -> Self {
        Self::BadRequest(err.to_string())
    }
}
