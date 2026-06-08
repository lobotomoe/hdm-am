//! Bridge configuration and the connection merge/resolve logic.
//!
//! A request may carry a partial `connection` override; it is merged field-by-field over the
//! bridge's configured default, then resolved to exactly the fields an operation needs (endpoint
//! only for probe, endpoint + password for the operator listing, the full session tuple for
//! everything else). Missing required fields are reported together so the caller fixes them in one
//! round-trip.

use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

/// Default HDM TCP port (the value the spec and every tested device use).
pub const DEFAULT_PORT: u16 = 1025;
/// Default socket timeout. The spec (§4.2 step 7) caps response wait at 50 seconds.
pub const DEFAULT_TIMEOUT_SECS: u64 = 50;
/// Hard upper bound on the socket timeout, mandated by the spec.
pub const MAX_TIMEOUT_SECS: u64 = 50;

/// Connection parameters with every field optional, used both for the configured default and for a
/// per-request override. Secrets are redacted in `Debug`.
#[derive(Clone, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartialConn {
    /// HDM host (IP or name).
    pub host: Option<String>,
    /// HDM TCP port; defaults to [`DEFAULT_PORT`].
    pub port: Option<u16>,
    /// HDM access password (used to derive the password key).
    pub password: Option<String>,
    /// Operator (cashier) numeric id for login.
    pub cashier: Option<u32>,
    /// Operator PIN for login.
    pub pin: Option<String>,
    /// Socket timeout in seconds; defaults to [`DEFAULT_TIMEOUT_SECS`], clamped to [`MAX_TIMEOUT_SECS`].
    pub timeout_secs: Option<u64>,
}

impl fmt::Debug for PartialConn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PartialConn")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .field("cashier", &self.cashier)
            .field("pin", &self.pin.as_ref().map(|_| "[REDACTED]"))
            .field("timeout_secs", &self.timeout_secs)
            .finish()
    }
}

/// A resolved endpoint: enough to open a socket and probe. No secrets.
#[derive(Clone, Debug)]
pub struct EndpointConn {
    /// HDM host (IP or name).
    pub host: String,
    /// HDM TCP port.
    pub port: u16,
    /// Socket timeout applied to connect, read, and write.
    pub timeout: Duration,
}

/// A resolved endpoint plus the access password (the password-key operations: probe-with-auth and
/// the operator/department listing).
#[derive(Clone)]
pub struct PasswordConn {
    /// The resolved endpoint.
    pub endpoint: EndpointConn,
    /// HDM access password.
    pub password: String,
}

impl fmt::Debug for PasswordConn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PasswordConn")
            .field("endpoint", &self.endpoint)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

/// A fully resolved operator session: endpoint, password, and the cashier credentials needed to
/// log in.
#[derive(Clone)]
pub struct SessionConn {
    /// The resolved endpoint.
    pub endpoint: EndpointConn,
    /// HDM access password.
    pub password: String,
    /// Operator (cashier) numeric id.
    pub cashier: u32,
    /// Operator PIN.
    pub pin: String,
}

impl fmt::Debug for SessionConn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionConn")
            .field("endpoint", &self.endpoint)
            .field("password", &"[REDACTED]")
            .field("cashier", &self.cashier)
            .field("pin", &"[REDACTED]")
            .finish()
    }
}

/// Names of the connection fields that were required but absent after merging the override over the
/// configured default.
#[derive(Clone, Debug)]
pub struct ResolveError {
    /// The missing field names, in declaration order.
    pub missing: Vec<&'static str>,
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "missing required connection field(s): {}",
            self.missing.join(", ")
        )
    }
}

impl std::error::Error for ResolveError {}

/// Everything the bridge needs to run: where to listen, how to authenticate callers, which origins
/// to allow, and the default device connection.
#[derive(Clone, Debug)]
pub struct BridgeConfig {
    /// Address to bind. Should be loopback in production.
    pub bind: SocketAddr,
    /// Shared bearer token required on every route except `/v1/health`.
    pub token: Option<String>,
    /// Allow starting without a token (localhost development only).
    pub insecure_no_auth: bool,
    /// Allowed CORS origins (exact matches). Empty means no browser origin is permitted.
    pub allow_origins: Vec<String>,
    /// The default device connection, overridable per request.
    pub default_conn: PartialConn,
}

fn clamp_timeout(secs: u64) -> Duration {
    Duration::from_secs(secs.clamp(1, MAX_TIMEOUT_SECS))
}

impl BridgeConfig {
    /// Merge a per-request override over the configured default (override wins field-by-field).
    fn merged(&self, over: Option<PartialConn>) -> PartialConn {
        let Some(o) = over else {
            return self.default_conn.clone();
        };
        let d = &self.default_conn;
        PartialConn {
            host: o.host.or_else(|| d.host.clone()),
            port: o.port.or(d.port),
            password: o.password.or_else(|| d.password.clone()),
            cashier: o.cashier.or(d.cashier),
            pin: o.pin.or_else(|| d.pin.clone()),
            timeout_secs: o.timeout_secs.or(d.timeout_secs),
        }
    }

    fn endpoint_from(
        merged: &PartialConn,
        missing: &mut Vec<&'static str>,
    ) -> Option<EndpointConn> {
        let host = merged.host.clone();
        if host.is_none() {
            missing.push("host");
        }
        host.map(|host| EndpointConn {
            host,
            port: merged.port.unwrap_or(DEFAULT_PORT),
            timeout: clamp_timeout(merged.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS)),
        })
    }

    /// Resolve just the endpoint (probe). Defaults fill port and timeout; only `host` is required.
    ///
    /// # Errors
    /// [`ResolveError`] listing the missing fields (`host`).
    pub fn resolve_endpoint(
        &self,
        over: Option<PartialConn>,
    ) -> Result<EndpointConn, ResolveError> {
        let merged = self.merged(over);
        let mut missing = Vec::new();
        let endpoint = Self::endpoint_from(&merged, &mut missing);
        endpoint.ok_or(ResolveError { missing })
    }

    /// Resolve endpoint + password (the operator/department listing, op 1).
    ///
    /// # Errors
    /// [`ResolveError`] listing every missing field (`host`, `password`).
    pub fn resolve_password(
        &self,
        over: Option<PartialConn>,
    ) -> Result<PasswordConn, ResolveError> {
        let merged = self.merged(over);
        let mut missing = Vec::new();
        let endpoint = Self::endpoint_from(&merged, &mut missing);
        let password = merged.password.clone();
        if password.is_none() {
            missing.push("password");
        }
        match (endpoint, password) {
            (Some(endpoint), Some(password)) => Ok(PasswordConn { endpoint, password }),
            _ => Err(ResolveError { missing }),
        }
    }

    /// Resolve the full operator session (everything except probe and the listing).
    ///
    /// # Errors
    /// [`ResolveError`] listing every missing field (`host`, `password`, `cashier`, `pin`).
    pub fn resolve_session(&self, over: Option<PartialConn>) -> Result<SessionConn, ResolveError> {
        let merged = self.merged(over);
        let mut missing = Vec::new();
        let endpoint = Self::endpoint_from(&merged, &mut missing);
        let password = merged.password.clone();
        if password.is_none() {
            missing.push("password");
        }
        if merged.cashier.is_none() {
            missing.push("cashier");
        }
        let pin = merged.pin.clone();
        if pin.is_none() {
            missing.push("pin");
        }
        match (endpoint, password, merged.cashier, pin) {
            (Some(endpoint), Some(password), Some(cashier), Some(pin)) => Ok(SessionConn {
                endpoint,
                password,
                cashier,
                pin,
            }),
            _ => Err(ResolveError { missing }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(default_conn: PartialConn) -> BridgeConfig {
        BridgeConfig {
            bind: "127.0.0.1:0".parse().expect("valid addr"),
            token: Some("t".into()),
            insecure_no_auth: false,
            allow_origins: vec![],
            default_conn,
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

    #[test]
    fn override_wins_field_by_field() {
        let c = cfg(full_default());
        let over = PartialConn {
            host: Some("192.168.1.4".into()),
            cashier: Some(7),
            ..PartialConn::default()
        };
        let s = c.resolve_session(Some(over)).expect("resolves");
        assert_eq!(s.endpoint.host, "192.168.1.4");
        assert_eq!(s.cashier, 7);
        assert_eq!(s.password, "pw"); // unchanged fields fall back to default
        assert_eq!(s.pin, "1234");
    }

    #[test]
    fn missing_fields_reported_together() {
        let c = cfg(PartialConn::default());
        let err = c.resolve_session(None).expect_err("nothing configured");
        assert_eq!(err.missing, vec!["host", "password", "cashier", "pin"]);
    }

    #[test]
    fn endpoint_applies_defaults() {
        let c = cfg(PartialConn {
            host: Some("h".into()),
            ..PartialConn::default()
        });
        let e = c.resolve_endpoint(None).expect("host present");
        assert_eq!(e.port, DEFAULT_PORT);
        assert_eq!(e.timeout, Duration::from_secs(DEFAULT_TIMEOUT_SECS));
    }

    #[test]
    fn timeout_is_clamped_to_spec_cap() {
        let c = cfg(PartialConn {
            host: Some("h".into()),
            timeout_secs: Some(9999),
            ..PartialConn::default()
        });
        let e = c.resolve_endpoint(None).expect("host present");
        assert_eq!(e.timeout, Duration::from_secs(MAX_TIMEOUT_SECS));
    }
}
