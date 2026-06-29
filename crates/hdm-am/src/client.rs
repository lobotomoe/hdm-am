//! High-level HDM client. Owns the transport, the encryption phase (password key before login,
//! session key after), and the sequence-number provider; exposes one method per spec operation.

use crate::crypto::{Codec, decode_session_key};
use crate::error::{Error, ServerErrorKind};
use crate::observer::{WireObserver, WireRequest, WireResponse};
use crate::operations::{
    CashInOutRequest, DateTimeRequest, DateTimeResponse, EmptyResponse, FiscalReportRequest,
    GetReturnableReceiptRequest, HdmTimeSyncRequest, ListOpsAndDepsRequest, ListOpsAndDepsResponse,
    Operation, OperatorLoginRequest, OperatorLogoutRequest, PaymentSystemsListRequest,
    PaymentSystemsListResponse, PrintLastReceiptRequest, PrintReceiptRequest,
    PrintReturnReceiptRequest, ReceiptResponse, ReceiptSampleRequest, ReturnReceiptResponse,
    ReturnableReceiptResponse, SetupHeaderFooterRequest, SetupHeaderLogoRequest,
    SingleEmarkRequest,
};
use crate::seq::SequenceProvider;
use crate::wire::{RESPONSE_CODE_OK, Request, ResponseHeader};
use serde::Serialize;
use std::io::{Read, Write};
use std::time::Instant;

/// Wraps a session request with its per-call sequence number. The HDM expects `seq` as a top-level
/// field next to the operation's own fields, so the request is flattened in. Keeping `seq` here
/// instead of on the public request structs means callers never see (or have to set) it.
#[derive(Serialize)]
struct Sequenced<R> {
    seq: i64,
    #[serde(flatten)]
    body: R,
}

/// Synchronous HDM client.
///
/// Generic over the transport (anything that's `Read + Write`) and the sequence-number provider
/// (anything implementing [`SequenceProvider`]). The split allows in-memory testing via
/// `std::io::Cursor`-style mocks and pluggable persistence (`InMemorySeq`, `FileSeq`, custom).
///
/// **Threading & device serialisation:** a single `Client` cannot be misused from two threads —
/// every operation takes `&mut self`, so the borrow checker already forbids overlapping calls on
/// one instance. The risk this crate cannot see, and therefore cannot guard, is *two different
/// `Client`s* (two TCP connections) reaching the **same physical HDM**: the device is single-session
/// and a second login tears down the first session's key, corrupting whichever flow was mid-receipt.
///
/// Because the shared resource is the device — outside any one `Client`'s ownership — serialisation
/// belongs to the layer that owns device access, not here. The consumer MUST funnel **every** path
/// that touches a given device through one serialisation point: a `Mutex<Client>`, a single owning
/// task/actor, or a one-slot connection pool. "Every path" explicitly includes background work —
/// availability probes, health checks, scheduled syncs — which is the easy thing to forget: a
/// monitor that opens its own connection while a sale is in flight reintroduces exactly the
/// single-session collision. For multi-process consumers the lock must be cross-process (see the
/// same caveat on [`crate::FileSeq`]); a process-local mutex is not enough.
///
/// **Timeouts:** `Client` does not enforce timeouts on its transport. Configure
/// `set_read_timeout`/`set_write_timeout` on a `TcpStream` before passing it to [`Self::new`].
/// The spec's §4.2 step 7 mandates a 50-second cap on response wait time.
///
/// **Connection lifecycle (TCP state hygiene):** `Client` is transport-agnostic and deliberately
/// does not manage socket states — `TIME_WAIT` and `CLOSE_WAIT` belong to the OS kernel, not the
/// application, and there is no per-state "time to hold". `TIME_WAIT` is timed by the kernel (≈60 s
/// on Linux) after an *active* close; `CLOSE_WAIT` has no timer at all and is cleared only by closing
/// the socket. Two rules keep the device's connection table clean:
/// 1. **Always [`logout`](Self::logout) before dropping** (the typical login→op→logout flow does
///    this) so *this* side performs the active close. `TIME_WAIT` then accrues on the integrator host,
///    which has ample ephemeral ports, rather than on the single-session HDM, whose connection slots
///    are scarce — a device drowning in `TIME_WAIT` can start refusing new connections.
/// 2. **Drop the `Client` as soon as you are done, and unconditionally after any
///    [`requires_reconnect`](Error::requires_reconnect) error.** The device closes the connection on
///    the §4.10 fatal codes; a `Client` kept alive past that point leaves its socket in `CLOSE_WAIT` —
///    a file-descriptor leak. Dropping closes the transport. To force an immediate FIN instead of
///    relying on drop timing, recover the socket with [`Self::into_transport`] and call
///    `shutdown(Shutdown::Both)` on the `TcpStream`.
pub struct Client<T: Read + Write, S: SequenceProvider> {
    transport: T,
    password_codec: Codec,
    session_codec: Option<Codec>,
    seq: S,
    password: String,
    observer: Option<Box<dyn WireObserver>>,
}

impl<T: Read + Write, S: SequenceProvider> Client<T, S> {
    /// Build a new client over `transport`, deriving the password key from `password`.
    pub fn new(transport: T, password: impl Into<String>, seq: S) -> Self {
        let password = password.into();
        let password_codec = Codec::from_password(&password);
        Self {
            transport,
            password_codec,
            session_codec: None,
            seq,
            password,
            observer: None,
        }
    }

    /// Attach a [`WireObserver`] that receives every request/response exchange — the unmasked
    /// plaintext, the framed bytes, and the decrypted response — for diagnostics or audit. Builder
    /// form for the common `Client::new(..).with_observer(..)` construction.
    ///
    /// The observer sees real secrets (password, PIN, session key); persisting them is the
    /// consumer's policy decision — see the [`observer`](crate::observer) module's security note.
    #[must_use]
    pub fn with_observer(mut self, observer: Box<dyn WireObserver>) -> Self {
        self.observer = Some(observer);
        self
    }

    /// Install or replace the wire observer on an existing client. See [`Self::with_observer`].
    pub fn set_observer(&mut self, observer: Option<Box<dyn WireObserver>>) {
        self.observer = observer;
    }

    /// Whether a session has been established via [`Self::login`] (and not invalidated).
    #[must_use]
    pub const fn is_logged_in(&self) -> bool {
        self.session_codec.is_some()
    }

    /// Drop the in-memory session key. Does not notify the HDM — call [`Self::logout`] for that.
    pub const fn forget_session(&mut self) {
        self.session_codec = None;
    }

    /// Consume the client and hand back the underlying transport.
    ///
    /// Lets a consumer that knows the concrete transport type close it deterministically — for a
    /// `TcpStream`, call `shutdown(std::net::Shutdown::Both)` on the returned value to send an
    /// immediate FIN rather than relying on drop timing (see the "Connection lifecycle" note on
    /// [`Client`]). The session key is dropped with `self`, so nothing sensitive outlives the call.
    #[must_use]
    pub fn into_transport(self) -> T {
        self.transport
    }

    // ---------------- Per-operation entry points ----------------

    /// Op 1 (§4.5.1): list configured operators and departments. Does not require login.
    ///
    /// # Errors
    /// See [`Error`].
    pub fn list_operators_and_departments(&mut self) -> Result<ListOpsAndDepsResponse, Error> {
        let request = ListOpsAndDepsRequest {
            password: self.password.clone(),
        };
        self.execute_with_password(&request)
    }

    /// Op 2 (§4.5.2): operator login. On success the session key returned by the HDM is decoded
    /// and installed; subsequent operations use it transparently.
    ///
    /// # Errors
    /// - [`Error::Server`] with `kind = BadOperatorPassword / NoSuchOperator / InactiveOperator`
    ///   on login failure.
    /// - [`Error::Crypto`] (`CryptoError::SessionKeyBase64` / `InvalidKeyLength`) if the HDM
    ///   returns a `key` field that isn't valid 24-byte Base64 (would indicate a device bug).
    pub fn login(&mut self, cashier: u32, pin: impl Into<String>) -> Result<(), Error> {
        let request = OperatorLoginRequest {
            password: self.password.clone(),
            cashier,
            pin: pin.into(),
        };
        let response = self.execute_with_password(&request)?;
        let session_key = decode_session_key(&response.key)?;
        self.session_codec = Some(Codec::from_key(session_key));
        log::info!("hdm-am: session established for cashier {cashier}");
        Ok(())
    }

    /// Op 3 (§4.5.3): operator logout. Drops the session both server-side and locally.
    ///
    /// # Errors
    /// See [`Error`]. Returns [`Error::NotLoggedIn`] if [`Self::login`] hasn't been called.
    pub fn logout(&mut self) -> Result<(), Error> {
        let _: EmptyResponse = self.execute_with_session(OperatorLogoutRequest {})?;
        self.session_codec = None;
        log::info!("hdm-am: session closed");
        Ok(())
    }

    /// Op 4 (§4.5.4): print a fiscal receipt. The sequence number is assigned by the client.
    ///
    /// # Errors
    /// See [`Error`]. Common business errors:
    /// `NoSuchDepartment`, `BadAtgCode`, `PaidLessThanTotal`, `BadEmarkFormat`,
    /// `PrinterOutOfPaper`, `HdmSyncRequired`. Check [`ServerErrorKind::is_retryable`] and
    /// [`ServerErrorKind::requires_relogin`] on the returned error.
    pub fn print_receipt(
        &mut self,
        request: PrintReceiptRequest,
    ) -> Result<ReceiptResponse, Error> {
        self.execute_with_session(request)
    }

    /// Op 5 (§4.5.5): reprint a copy of the operator's most recent receipt.
    ///
    /// # Errors
    /// See [`Error`].
    pub fn print_last_receipt(&mut self) -> Result<EmptyResponse, Error> {
        self.execute_with_session(PrintLastReceiptRequest {})
    }

    /// Op 10 (§4.5.6): look up the contents of a receipt you intend to return.
    ///
    /// Read-only — returns the receipt's items, amounts and eMarks so you can build the actual
    /// return ([`Self::print_return_receipt`], op 6). It registers nothing.
    ///
    /// # Errors
    /// See [`Error`].
    pub fn get_returnable_receipt(
        &mut self,
        receipt_id: impl Into<String>,
        crn: impl Into<String>,
    ) -> Result<ReturnableReceiptResponse, Error> {
        self.execute_with_session(GetReturnableReceiptRequest {
            receipt_id: receipt_id.into(),
            crn: crn.into(),
        })
    }

    /// Op 7 (§4.6.3): configure the header/footer lines printed on every receipt.
    ///
    /// # Errors
    /// See [`Error`].
    pub fn setup_header_footer(
        &mut self,
        request: SetupHeaderFooterRequest,
    ) -> Result<EmptyResponse, Error> {
        self.execute_with_session(request)
    }

    /// Op 8 (§4.6.4): upload a header logo image (Base64-encoded BMP, colour depth ≤4 bits).
    ///
    /// # Errors
    /// See [`Error`].
    pub fn setup_header_logo(
        &mut self,
        logo_base64: impl Into<String>,
    ) -> Result<EmptyResponse, Error> {
        self.execute_with_session(SetupHeaderLogoRequest {
            header_logo: logo_base64.into(),
        })
    }

    /// Op 9 (§4.6.2): print a fiscal report (X-report = interim, Z-report = end-of-day).
    ///
    /// # Errors
    /// See [`Error`].
    pub fn fiscal_report(&mut self, request: FiscalReportRequest) -> Result<EmptyResponse, Error> {
        self.execute_with_session(request)
    }

    /// Op 6 (§4.5.7): print a return/refund receipt — the operation that actually registers a
    /// return. Full, by-amount or per-item returns are driven via the request's optional fields.
    /// The read-only lookup of the receipt being returned is op 10 ([`Self::get_returnable_receipt`]).
    ///
    /// # Errors
    /// See [`Error`].
    pub fn print_return_receipt(
        &mut self,
        request: PrintReturnReceiptRequest,
    ) -> Result<ReturnReceiptResponse, Error> {
        self.execute_with_session(request)
    }

    /// Op 11 (§4.5.8): record a cash-drawer in/out adjustment.
    ///
    /// # Errors
    /// See [`Error`].
    pub fn cash_in_out(&mut self, request: CashInOutRequest) -> Result<EmptyResponse, Error> {
        self.execute_with_session(request)
    }

    /// Op 12 (§4.6): query the HDM's current date and time.
    ///
    /// # Errors
    /// See [`Error`].
    pub fn date_time(&mut self) -> Result<DateTimeResponse, Error> {
        self.execute_with_session(DateTimeRequest {})
    }

    /// Op 13 (§4.6.1): print a sample receipt for layout/operator verification.
    ///
    /// # Errors
    /// See [`Error`].
    pub fn receipt_sample(&mut self) -> Result<EmptyResponse, Error> {
        self.execute_with_session(ReceiptSampleRequest {})
    }

    /// Op 14 (§4.7): synchronise the HDM with the tax authority's clock/state.
    ///
    /// # Errors
    /// See [`Error`].
    pub fn hdm_time_sync(&mut self) -> Result<EmptyResponse, Error> {
        self.execute_with_session(HdmTimeSyncRequest {})
    }

    /// Op 15 (§4.8): list the payment systems configured on the HDM. Use this once at startup
    /// to discover the code-to-name mapping for [`PrintReceiptRequest::payment_system`] rather
    /// than hardcoding codes.
    ///
    /// # Errors
    /// See [`Error`].
    pub fn payment_systems_list(&mut self) -> Result<PaymentSystemsListResponse, Error> {
        self.execute_with_session(PaymentSystemsListRequest {})
    }

    /// Op 16 (§4.9): submit a single eMark traceability code.
    ///
    /// # Errors
    /// See [`Error`]. `BadEmarkFormat` for malformed codes (see §4.9 escaping rules).
    pub fn single_emark(&mut self, e_mark: impl Into<String>) -> Result<EmptyResponse, Error> {
        self.execute_with_session(SingleEmarkRequest {
            e_mark: e_mark.into(),
        })
    }

    // ---------------- Internals ----------------

    /// Execute an operation that uses the password-derived key (ops 1 and 2).
    fn execute_with_password<R: Operation>(&mut self, request: &R) -> Result<R::Response, Error> {
        const { assert!(R::USES_PASSWORD_KEY, "expected password-key op") };
        let codec = self.password_codec.clone();
        self.round_trip_op::<R>(request, &codec, None)
    }

    /// Execute an operation that uses the session key (all ops except 1 and 2).
    ///
    /// If the operation fails in a way that ends the session — a relogin-class error (stale key,
    /// server-side session timeout, or a code that mandates re-login) or a reconnect-class error
    /// (transport failure, or a §4.10 code after which the device tears down the connection) — the
    /// local session is dropped so [`Self::is_logged_in`] reflects reality rather than claiming a
    /// session the protocol has already torn down.
    fn execute_with_session<R: Operation>(&mut self, request: R) -> Result<R::Response, Error> {
        const { assert!(!R::USES_PASSWORD_KEY, "expected session-key op") };
        let codec = self
            .session_codec
            .as_ref()
            .ok_or(Error::NotLoggedIn)?
            .clone();
        let seq = self.next_seq()?;
        let body = Sequenced { seq, body: request };
        let result = self.round_trip_op::<R>(&body, &codec, Some(seq));
        if let Err(ref err) = result {
            if err.requires_relogin() || err.requires_reconnect() {
                self.session_codec = None;
                log::debug!("hdm-am: session invalidated after a session-ending error");
            }
        }
        result
    }

    /// Perform a full request/response round-trip: JSON-encode → encrypt → write framing →
    /// read header → check server code → read payload → decrypt → JSON-decode.
    ///
    /// `seq` is the sequence number carried in `body` (session ops) or `None` (op 1 / op 2); it is
    /// purely forwarded to the [`WireObserver`], not used for the exchange itself.
    fn round_trip_op<R: Operation>(
        &mut self,
        body: &impl Serialize,
        codec: &Codec,
        seq: Option<i64>,
    ) -> Result<R::Response, Error> {
        let plaintext = serde_json::to_vec(body).map_err(Error::Encode)?;
        let ciphertext = codec.encrypt(&plaintext)?;
        let payload_len = ciphertext.len();
        u16::try_from(payload_len).map_err(|_| Error::PayloadTooLarge { len: payload_len })?;

        log::debug!(
            "hdm-am: -> op {:?} ({} plaintext / {} ciphertext bytes)",
            R::CODE,
            plaintext.len(),
            ciphertext.len()
        );

        // Frame into a buffer rather than straight to the transport: the observer can then see the
        // exact bytes that go on the wire, and the whole frame is written in a single call.
        let wire = Request {
            op: R::CODE,
            payload: ciphertext,
        };
        // 12-byte request prefix: magic(6) + version(2) + op + reserved(2) + length(2), then payload.
        let mut frame = Vec::with_capacity(12 + wire.payload.len());
        wire.encode(&mut frame)?;

        let started = Instant::now();
        let write_result = self.transport.write_all(&frame);

        // Record exactly what was sent BEFORE surfacing any failure (even a partial write may have
        // reached the device) — this is what captures a wedged device's last request.
        if let Some(observer) = self.observer.as_deref() {
            observer.on_request(&WireRequest {
                op: R::CODE,
                seq,
                plaintext: &plaintext,
                frame: &frame,
            });
        }
        if let Err(err) = write_result {
            self.observe_failure::<R>(started, &err);
            return Err(Error::Transport(err));
        }

        let header = match ResponseHeader::read(&mut self.transport) {
            Ok(header) => header,
            Err(err) => {
                self.observe_failure::<R>(started, &err);
                return Err(err);
            }
        };

        log::debug!(
            "hdm-am: <- op {:?} response code {} ({} bytes payload)",
            R::CODE,
            header.code,
            header.payload_len
        );

        // Always drain the payload bytes from the transport, even on error, so that the
        // connection stays in sync for the next request.
        let mut payload = vec![0u8; usize::from(header.payload_len)];
        if header.payload_len > 0 {
            if let Err(err) = self.transport.read_exact(&mut payload) {
                self.observe_failure::<R>(started, &err);
                return Err(Error::Transport(err));
            }
        }

        if header.code != RESPONSE_CODE_OK {
            let kind = ServerErrorKind::from_code(header.code);
            log::warn!(
                "hdm-am: op {:?} returned server code {} ({:?})",
                R::CODE,
                header.code,
                kind
            );
            // A non-200 body is not decrypted (the client surfaces the code, not the contents), so
            // the observer gets the raw ciphertext with no plaintext.
            self.observe_received::<R>(started, header, &payload, None);
            return Err(Error::Server {
                code: header.code,
                kind,
            });
        }

        // Empty success payload — most "no-content" ops (logout, sample, etc.) come through here.
        if payload.is_empty() {
            self.observe_received::<R>(started, header, &payload, None);
            return serde_json::from_slice(b"{}").map_err(Error::Decode);
        }

        let plaintext_response = match codec.decrypt(&payload) {
            Ok(plaintext_response) => plaintext_response,
            Err(err) => {
                // Decryption failed — typically a stale session key. Record the ciphertext we could
                // not read so the trace still shows the device answered, then surface the error.
                self.observe_received::<R>(started, header, &payload, None);
                return Err(Error::Crypto(err));
            }
        };
        if R::RESPONSE_IS_SECRET {
            // e.g. the login response carries the session key — never log it, even at TRACE.
            log::trace!(
                "hdm-am: <- op {:?} decrypted payload: [redacted: response carries a secret]",
                R::CODE
            );
        } else {
            log::trace!(
                "hdm-am: <- op {:?} decrypted payload: {}",
                R::CODE,
                String::from_utf8_lossy(&plaintext_response)
            );
        }
        self.observe_received::<R>(started, header, &payload, Some(&plaintext_response));
        serde_json::from_slice(&plaintext_response).map_err(Error::Decode)
    }

    /// Notify the observer (if any) that an exchange failed before a framed response was read.
    fn observe_failure<R: Operation>(&self, started: Instant, detail: impl std::fmt::Display) {
        if let Some(observer) = self.observer.as_deref() {
            observer.on_response(&WireResponse::Failed {
                op: R::CODE,
                detail: detail.to_string(),
                elapsed: started.elapsed(),
            });
        }
    }

    /// Notify the observer (if any) that a framed response was read (any code).
    fn observe_received<R: Operation>(
        &self,
        started: Instant,
        header: ResponseHeader,
        ciphertext: &[u8],
        plaintext: Option<&[u8]>,
    ) {
        if let Some(observer) = self.observer.as_deref() {
            observer.on_response(&WireResponse::Received {
                op: R::CODE,
                protocol_version: header.protocol_version,
                software_version: header.software_version,
                code: header.code,
                ciphertext,
                plaintext,
                elapsed: started.elapsed(),
            });
        }
    }

    fn next_seq(&mut self) -> Result<i64, Error> {
        self.seq.next().map_err(Error::Transport)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CryptoError;
    use crate::seq::InMemorySeq;
    use crate::wire::{MAGIC, OperationCode, PROTOCOL_VERSION};
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    use rust_decimal::Decimal;
    use std::io::{self, Cursor};

    /// Loopback transport: write side captures bytes for assertions, read side is pre-loaded
    /// with the bytes we want the server to "send".
    struct Loopback {
        written: Vec<u8>,
        incoming: Cursor<Vec<u8>>,
    }

    impl Loopback {
        fn new(incoming: Vec<u8>) -> Self {
            Self {
                written: Vec::new(),
                incoming: Cursor::new(incoming),
            }
        }

        fn no_incoming() -> Self {
            Self::new(Vec::new())
        }
    }

    impl Read for Loopback {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.incoming.read(buf)
        }
    }

    impl Write for Loopback {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.written.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// Build a fake server response envelope (header + ciphertext) using the given codec.
    fn make_response(code: u16, codec: &Codec, plaintext: &[u8]) -> Vec<u8> {
        let ciphertext = codec.encrypt(plaintext).expect("encrypt");
        let len = u16::try_from(ciphertext.len()).expect("fits in u16");
        let mut out = Vec::new();
        // pv (2) + sw (3) + code (2) + len (2) + reserved (2) = 11 bytes
        out.extend_from_slice(&[0x00, 0x05]); // protocol version
        out.extend_from_slice(&[0x02, 0x02, 0x10]); // sw version
        out.extend_from_slice(&code.to_be_bytes()); // code
        out.extend_from_slice(&len.to_be_bytes()); // payload length
        out.extend_from_slice(&[0x00, 0x00]); // reserved
        out.extend_from_slice(&ciphertext);
        out
    }

    /// `is_logged_in` reflects the session state.
    #[test]
    fn is_logged_in_false_until_login() {
        let client = Client::new(Loopback::no_incoming(), "pw", InMemorySeq::default());
        assert!(!client.is_logged_in());
    }

    /// Calling a session-requiring op before login returns `Error::NotLoggedIn`.
    #[test]
    fn session_ops_require_login() {
        let mut client = Client::new(Loopback::no_incoming(), "pw", InMemorySeq::default());
        let err = client.logout().expect_err("expected NotLoggedIn");
        assert!(matches!(err, Error::NotLoggedIn));
        assert!(err.requires_relogin());
    }

    /// Successful login installs the session codec, returning Ok(()).
    #[test]
    fn login_happy_path_installs_session() {
        // Prepare a fake server response: HTTP 200 with a Base64-encoded 24-byte session key.
        let password = "test-password";
        let password_codec = Codec::from_password(password);
        let session_key = [0xAB_u8; 24];
        let key_b64 = BASE64.encode(session_key);
        let response_json = format!(r#"{{"key":"{key_b64}"}}"#);
        let incoming = make_response(200, &password_codec, response_json.as_bytes());

        let mut client = Client::new(Loopback::new(incoming), password, InMemorySeq::default());
        client.login(42, "1234").expect("login should succeed");
        assert!(client.is_logged_in());
    }

    /// Wire bytes written during login start with the correct framing (magic + version + op).
    #[test]
    fn login_writes_correct_wire_framing() {
        let password = "pw";
        let password_codec = Codec::from_password(password);
        let key_b64 = BASE64.encode([0u8; 24]);
        let response_json = format!(r#"{{"key":"{key_b64}"}}"#);
        let incoming = make_response(200, &password_codec, response_json.as_bytes());

        let mut client = Client::new(Loopback::new(incoming), password, InMemorySeq::default());
        client.login(1, "0000").expect("login");
        let written = &client.transport.written;
        assert!(written.starts_with(&MAGIC));
        assert_eq!(&written[6..8], PROTOCOL_VERSION);
        assert_eq!(written[8], OperationCode::OperatorLogin as u8);
        assert_eq!(written[9], 0); // reserved
    }

    /// A bad-password response (code 111) surfaces as `Error::Server { kind: BadOperatorPassword }`.
    #[test]
    fn login_surfaces_bad_password_error() {
        let password = "wrong";
        let password_codec = Codec::from_password(password);
        // For a non-200 response, body content doesn't matter — but we still encrypt
        // an empty `{}` to keep framing valid.
        let incoming = make_response(111, &password_codec, b"{}");

        let mut client = Client::new(Loopback::new(incoming), password, InMemorySeq::default());
        let err = client.login(1, "0000").expect_err("expected login failure");
        match err {
            Error::Server { code, kind } => {
                assert_eq!(code, 111);
                assert_eq!(kind, ServerErrorKind::BadOperatorPassword);
                assert!(kind.requires_relogin());
            }
            other => panic!("unexpected variant: {other:?}"),
        }
        // Failed login must not install a session.
        assert!(!client.is_logged_in());
    }

    /// `Error::Crypto(BadPadding)` is the typical symptom of a stale session key.
    #[test]
    fn decryption_failure_after_login_surfaces_as_crypto_error() {
        let password = "pw";
        let password_codec = Codec::from_password(password);

        // Stage 1: install session — successful login.
        let session_key = [0x11_u8; 24];
        let key_b64 = BASE64.encode(session_key);
        let login_response = format!(r#"{{"key":"{key_b64}"}}"#);
        let mut incoming = make_response(200, &password_codec, login_response.as_bytes());

        // Stage 2: an op-12 response, but encrypted with the WRONG key. Decryption will fail.
        let wrong_codec = Codec::from_key([0x99; 24]);
        incoming.extend_from_slice(&make_response(200, &wrong_codec, br#"{"dt":"now"}"#));

        let mut client = Client::new(Loopback::new(incoming), password, InMemorySeq::default());
        client.login(1, "0000").expect("login");

        let err = client.date_time().expect_err("expected decryption failure");
        match err {
            Error::Crypto(CryptoError::BadPadding) => {}
            other => panic!("expected Crypto(BadPadding), got {other:?}"),
        }
        assert!(err.requires_relogin());
    }

    /// The client injects the sequence number into the request it sends; callers never set it.
    #[test]
    fn print_receipt_injects_sequence_number() {
        use crate::operations::PrintMode;

        let password = "pw";
        let password_codec = Codec::from_password(password);
        let key_b64 = BASE64.encode([0; 24]);
        let login_resp = format!(r#"{{"key":"{key_b64}"}}"#);
        let session_codec = Codec::from_key([0; 24]);

        let mut wire = make_response(200, &password_codec, login_resp.as_bytes());
        // Receipt response — minimal valid ReceiptResponse.
        let receipt_json = r#"{"rseq":1,"crn":"","sn":"","tin":"","taxpayer":"","address":"","time":0,"fiscal":"","lottery":"","prize":0,"total":0.0,"change":0.0}"#;
        wire.extend_from_slice(&make_response(200, &session_codec, receipt_json.as_bytes()));

        let seq = InMemorySeq::starting_at(99);
        let mut client = Client::new(Loopback::new(wire), password, seq);
        client.login(1, "0").unwrap();

        let request = PrintReceiptRequest {
            mode: PrintMode::Simple,
            paid_amount: Decimal::from(100),
            paid_amount_card: Decimal::ZERO,
            partial_amount: Decimal::ZERO,
            pre_payment_amount: Decimal::ZERO,
            dep: Some(1),
            partner_tin: None,
            use_ext_pos: false,
            payment_system: None,
            rrn: None,
            terminal_id: None,
            e_marks: vec![],
            items: vec![],
        };
        let response = client.print_receipt(request).expect("print receipt");
        assert_eq!(response.rseq, 1);

        // Decrypt the last request the client wrote and confirm it injected the sequence number
        // (login leaves the counter at 99; the first session op takes 100) alongside the flattened
        // request body.
        let written = &client.transport.written;
        let mut off = 0;
        let mut last_payload: &[u8] = &[];
        while off + 12 <= written.len() {
            let len = usize::from(u16::from_be_bytes([written[off + 10], written[off + 11]]));
            last_payload = &written[off + 12..off + 12 + len];
            off += 12 + len;
        }
        let decrypted = session_codec
            .decrypt(last_payload)
            .expect("decrypt request");
        let json: serde_json::Value = serde_json::from_slice(&decrypted).expect("valid JSON");
        assert_eq!(json["seq"], 100, "client must inject the sequence number");
        assert_eq!(json["mode"], 1, "request body is flattened alongside seq");
    }

    /// A reconnect-class server error (here `104`, after which the device closes the connection)
    /// must invalidate the local session, even though it is not a relogin-class code. Otherwise
    /// `is_logged_in()` would keep claiming a session the device has already torn down.
    #[test]
    fn reconnect_class_error_invalidates_session() {
        let password = "pw";
        let password_codec = Codec::from_password(password);
        let key_b64 = BASE64.encode([0u8; 24]);
        let login_response = format!(r#"{{"key":"{key_b64}"}}"#);
        let session_codec = Codec::from_key([0u8; 24]);

        let mut incoming = make_response(200, &password_codec, login_response.as_bytes());
        // Op-12 attempt answered with 104 (BadSequenceNumber): fatal-for-connection, not relogin.
        incoming.extend_from_slice(&make_response(104, &session_codec, b"{}"));

        let mut client = Client::new(Loopback::new(incoming), password, InMemorySeq::default());
        client.login(1, "0000").expect("login");
        assert!(client.is_logged_in());

        let err = client.date_time().expect_err("expected a server error");
        assert!(matches!(
            err,
            Error::Server {
                kind: ServerErrorKind::BadSequenceNumber,
                ..
            }
        ));
        assert!(err.requires_reconnect());
        assert!(!err.requires_relogin());
        // The session must be gone: the device closed the connection, so the key is dead.
        assert!(!client.is_logged_in());
    }

    /// Server-side errors include the raw payload drain — connection stays in sync for the next op.
    #[test]
    fn error_response_drains_payload_to_keep_transport_in_sync() {
        let password = "pw";
        let password_codec = Codec::from_password(password);

        // Two consecutive responses on the same transport: first is an error with non-zero
        // payload, second is a valid one. If we don't drain the error's payload, the second
        // read will be misaligned.
        let mut incoming = make_response(151, &password_codec, b"some-irrelevant-error-body");
        let key_b64 = BASE64.encode([0u8; 24]);
        let login_response = format!(r#"{{"key":"{key_b64}"}}"#);
        incoming.extend_from_slice(&make_response(
            200,
            &password_codec,
            login_response.as_bytes(),
        ));

        let mut client = Client::new(Loopback::new(incoming), password, InMemorySeq::default());

        // First call: server returns 151 (NoSuchDepartment).
        let err = client
            .list_operators_and_departments()
            .expect_err("expected error");
        assert!(matches!(
            err,
            Error::Server {
                kind: ServerErrorKind::NoSuchDepartment,
                ..
            }
        ));

        // Second call must succeed — proves payload from the first response was drained.
        client.login(1, "0000").expect("second call should align");
        assert!(client.is_logged_in());
    }

    /// A recording observer that flattens every callback into a string log for assertions.
    #[derive(Clone, Default)]
    struct Recorder {
        events: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl WireObserver for Recorder {
        fn on_request(&self, request: &WireRequest<'_>) {
            self.events.lock().expect("lock").push(format!(
                "req op={} seq={:?} plaintext={}",
                request.op as u8,
                request.seq,
                String::from_utf8_lossy(request.plaintext)
            ));
        }

        fn on_response(&self, response: &WireResponse<'_>) {
            let line = match response {
                WireResponse::Received {
                    op,
                    code,
                    plaintext,
                    ..
                } => format!(
                    "resp op={} code={} plaintext={}",
                    *op as u8,
                    code,
                    plaintext.map_or_else(
                        || "<none>".to_owned(),
                        |p| String::from_utf8_lossy(p).into_owned()
                    )
                ),
                WireResponse::Failed { op, detail, .. } => {
                    format!("fail op={} detail={detail}", *op as u8)
                }
            };
            self.events.lock().expect("lock").push(line);
        }
    }

    /// The observer sees the real, UNMASKED bytes both ways: the login request carries the
    /// plaintext password, and the login response plaintext carries the session key.
    #[test]
    fn observer_captures_request_and_response_unmasked() {
        let password = "test-password";
        let password_codec = Codec::from_password(password);
        let session_key = [0xAB_u8; 24];
        let key_b64 = BASE64.encode(session_key);
        let login_resp = format!(r#"{{"key":"{key_b64}"}}"#);
        let mut incoming = make_response(200, &password_codec, login_resp.as_bytes());

        let session_codec = Codec::from_key(session_key);
        incoming.extend_from_slice(&make_response(
            200,
            &session_codec,
            br#"{"dt":"2026-06-29 14:00:00"}"#,
        ));

        let recorder = Recorder::default();
        let mut client = Client::new(Loopback::new(incoming), password, InMemorySeq::default())
            .with_observer(Box::new(recorder.clone()));
        client.login(7, "1234").expect("login");
        client.date_time().expect("date_time");

        let events = recorder.events.lock().expect("lock").clone();
        // login request — op 2, no seq, password present in cleartext.
        assert!(
            events.iter().any(|e| e.contains("req op=2")
                && e.contains("seq=None")
                && e.contains(r#""password":"test-password""#)),
            "unmasked login request not captured: {events:?}"
        );
        // login response — op 2, 200, decrypted plaintext carries the session key (unmasked).
        assert!(
            events
                .iter()
                .any(|e| e.contains("resp op=2") && e.contains("code=200") && e.contains(&key_b64)),
            "unmasked login response not captured: {events:?}"
        );
        // session op — op 12 with the injected sequence number, then its decrypted reply.
        assert!(
            events
                .iter()
                .any(|e| e.contains("req op=12") && e.contains("seq=Some(1)")),
            "session-op request/seq not captured: {events:?}"
        );
        assert!(
            events
                .iter()
                .any(|e| e.contains("resp op=12") && e.contains("dt")),
            "session-op response not captured: {events:?}"
        );
    }

    /// The wedge signature: a request goes out, the device never answers. The observer must record
    /// the request AND a `Failed` outcome — the whole reason `on_request` fires before the read.
    #[test]
    fn observer_records_silent_device_as_failed() {
        let password = "pw";
        let password_codec = Codec::from_password(password);
        let key_b64 = BASE64.encode([0u8; 24]);
        let login_resp = format!(r#"{{"key":"{key_b64}"}}"#);
        // Only a login response — nothing for the date_time read, so the device "goes silent".
        let incoming = make_response(200, &password_codec, login_resp.as_bytes());

        let recorder = Recorder::default();
        let mut client = Client::new(Loopback::new(incoming), password, InMemorySeq::default())
            .with_observer(Box::new(recorder.clone()));
        client.login(1, "0").expect("login");
        client.date_time().expect_err("device went silent");

        let events = recorder.events.lock().expect("lock").clone();
        assert!(
            events.iter().any(|e| e.contains("req op=12")),
            "the unanswered request must still be recorded: {events:?}"
        );
        assert!(
            events.iter().any(|e| e.starts_with("fail op=12")),
            "a silent device must surface as Failed: {events:?}"
        );
    }
}
