use crate::wire::OperationCode;
use serde::{Deserialize, Serialize};

use super::{EmptyResponse, Operation};

/// Op 2 request. Encrypted with the password key. The response contains the session key for
/// subsequent operations.
///
/// `Debug` is hand-written to redact `password` and `pin` so they never leak through `{:?}`.
#[derive(Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct OperatorLoginRequest {
    /// HDM access password.
    pub password: String,
    /// Operator (cashier) numeric ID — value from `OperatorInfo::id`.
    pub cashier: u32,
    /// Operator's PIN code.
    pub pin: String,
}

impl std::fmt::Debug for OperatorLoginRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OperatorLoginRequest")
            .field("password", &"[redacted]")
            .field("cashier", &self.cashier)
            .field("pin", &"[redacted]")
            .finish()
    }
}

impl Operation for OperatorLoginRequest {
    const CODE: OperationCode = OperationCode::OperatorLogin;
    const USES_PASSWORD_KEY: bool = true;
    const RESPONSE_IS_SECRET: bool = true;
    type Response = OperatorLoginResponse;
}

/// Op 2 response: the Base64-encoded session key.
///
/// Deliberately does **not** derive `Serialize` (so it can't leak via `--json`), and its `Debug` is
/// hand-written to redact `key`: the session secret must never be casually written to logs or
/// output. The client consumes it internally and never exposes it.
#[derive(Clone, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct OperatorLoginResponse {
    /// 24-byte session key, Base64-encoded.
    pub key: String,
}

impl std::fmt::Debug for OperatorLoginResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OperatorLoginResponse")
            .field("key", &"[redacted]")
            .finish()
    }
}

/// Op 3 request. Encrypted with the session key. No meaningful response.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct OperatorLogoutRequest {}

impl Operation for OperatorLogoutRequest {
    const CODE: OperationCode = OperationCode::OperatorLogout;
    type Response = EmptyResponse;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Debug` must redact the password, PIN, and session key — never print them.
    #[test]
    fn debug_redacts_secrets() {
        let request = OperatorLoginRequest {
            password: "super-secret-pw".to_owned(),
            cashier: 3,
            pin: "9999".to_owned(),
        };
        let rendered = format!("{request:?}");
        assert!(rendered.contains("[redacted]"), "{rendered}");
        assert!(
            !rendered.contains("super-secret-pw"),
            "password leaked: {rendered}"
        );
        assert!(!rendered.contains("9999"), "pin leaked: {rendered}");
        assert!(
            rendered.contains("cashier: 3"),
            "non-secret field should show: {rendered}"
        );

        let response = OperatorLoginResponse {
            key: "SECRET-SESSION-KEY".to_owned(),
        };
        let rendered = format!("{response:?}");
        assert!(rendered.contains("[redacted]"), "{rendered}");
        assert!(
            !rendered.contains("SECRET-SESSION-KEY"),
            "session key leaked: {rendered}"
        );
    }
}
