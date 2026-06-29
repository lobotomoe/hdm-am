//! Wire-level capture hook for diagnostics and audit.
//!
//! [`WireObserver`] receives every request/response exchange a [`Client`](crate::Client) performs,
//! with the FULL, UNMASKED bytes: the request plaintext, the framed bytes that went on the wire, the
//! response header, the response ciphertext, and the decrypted response plaintext. It exists so a
//! consumer can persist a faithful, byte-exact trace when diagnosing a misbehaving device — for
//! example an HDM that accepts a request and then stops answering mid-session.
//!
//! The crate's own `log` output stays redacted regardless of any observer; the unmasked bytes are
//! seen only by an observer the consumer explicitly installs via
//! [`Client::with_observer`](crate::Client::with_observer).
//!
//! # Security
//!
//! An observer is handed real secrets: the access password and operator PIN appear in op-1/op-2
//! request plaintext, and the session key appears in the op-2 response plaintext. This is
//! deliberate — a capture that masked them would be useless for the failures it exists to diagnose.
//! **Redaction, retention, and access control are entirely the consumer's policy.** A trace produced
//! from an observer must be access-controlled, short-lived, and never shipped off the device.
//!
//! Note that no *cardholder* data passes through this layer: the HDM protocol never carries a PAN,
//! track, or CVV — only a masked acquirer RRN and terminal id. The sensitive material here is
//! device-access credentials, not regulated card data.

use crate::wire::OperationCode;
use std::time::Duration;

/// Receives a copy of every wire exchange a [`Client`](crate::Client) performs. Install one with
/// [`Client::with_observer`](crate::Client::with_observer).
///
/// Both callbacks run **synchronously on the thread driving the client**, in the middle of a
/// request/response round-trip, so an implementation must be cheap and non-blocking: append to an
/// in-memory buffer or a buffered writer, never do network I/O or take a contended lock. Treat them
/// as infallible — a panic propagates out of the client call like any other.
///
/// Every [`on_request`](Self::on_request) is paired with exactly one
/// [`on_response`](Self::on_response) for the same operation, in order, so a consumer can match them
/// positionally without correlating fields.
pub trait WireObserver: Send {
    /// Invoked immediately after a request frame is handed to the transport and **before** the
    /// response is read. Recording at this point — not only on completion — is what captures a
    /// request whose response never arrives: when the device wedges, the paired
    /// [`on_response`](Self::on_response) reports the timeout while this call already recorded
    /// exactly what was sent.
    fn on_request(&self, request: &WireRequest<'_>);

    /// Invoked once the exchange resolves: either a framed response was read (for any code,
    /// including non-200), or the read failed (timeout / connection reset — the wedge signature).
    fn on_response(&self, response: &WireResponse<'_>);
}

/// A request exactly as it went onto the wire. Borrows the client's buffers — copy out what you
/// need to retain.
#[derive(Debug, Clone, Copy)]
pub struct WireRequest<'a> {
    /// Operation code.
    pub op: OperationCode,
    /// The sequence number carried in the body, for session ops that use one. `None` for the
    /// password-key ops (op 1 list, op 2 login), which carry no `seq`.
    pub seq: Option<i64>,
    /// Plaintext request JSON, exactly as serialised before encryption.
    pub plaintext: &'a [u8],
    /// The full bytes written to the transport: magic + version + op + reserved + length +
    /// ciphertext.
    pub frame: &'a [u8],
}

/// The outcome of a request, delivered to [`WireObserver::on_response`].
#[derive(Debug)]
pub enum WireResponse<'a> {
    /// A framed response was read from the device — for ANY response code, including non-200.
    Received {
        /// Operation the response is for.
        op: OperationCode,
        /// Device protocol version reported in the response header (major, minor).
        protocol_version: (u8, u8),
        /// Device firmware version reported in the response header (major, minor, patch).
        software_version: (u8, u8, u8),
        /// Response code (200 = success; see spec §4.10).
        code: u16,
        /// Encrypted payload exactly as read from the wire (empty for no-content responses).
        ciphertext: &'a [u8],
        /// Decrypted payload plaintext, when there was a payload and it decrypted. `None` for an
        /// empty payload, for a non-200 response (whose body the client does not decrypt), or for a
        /// decryption failure (e.g. a stale session key).
        plaintext: Option<&'a [u8]>,
        /// Wall-clock from just before the request write to just after the response read.
        elapsed: Duration,
    },
    /// No framed response was read — a transport error or timeout. This is the signature of a device
    /// that accepted the request and then stopped answering.
    Failed {
        /// Operation that failed.
        op: OperationCode,
        /// Human-readable failure detail (the transport error's `Display`).
        detail: String,
        /// Wall-clock from just before the request write to the failure.
        elapsed: Duration,
    },
}
