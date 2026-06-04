//! Credential-less endpoint identification.
//!
//! [`identify`] decides whether a TCP endpoint speaks the HDM protocol without any password,
//! session, or sequence number. It is the primitive a discovery sweep uses to fingerprint the
//! candidates found by a plain TCP connect.
//!
//! The probe sends a single framed operation-1 request (list operators/departments — read-only)
//! carrying a deliberately bogus, un-encrypted payload, then inspects the response header. A real
//! HDM answers in the HDM protocol family (matching protocol *major* version) regardless of the
//! request being unauthenticated: the common outcomes are response code `403` (the caller's IP is
//! not yet whitelisted on the device) or `101` (the bogus ciphertext failed to decrypt) — both
//! confirm identity. A non-HDM service returns bytes that do not begin with the HDM protocol major
//! version, or closes the connection.
//!
//! The probe never reaches device state: operation 1 is read-only and the bogus payload is
//! rejected (IP check / decryption) before anything is listed.

use crate::error::Error;
use crate::wire::{OperationCode, PROTOCOL_VERSION, Request, ResponseHeader};
use std::io::{Read, Write};

/// Bogus probe payload: a single 3DES block of zero bytes. Block-aligned so the device proceeds
/// to (and fails) decryption rather than rejecting the frame on length alone; its content is
/// irrelevant because no valid key produced it.
const PROBE_PAYLOAD: [u8; 8] = [0u8; 8];

/// Protocol identity reported by an endpoint that responded to [`identify`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HdmIdentity {
    /// HDM protocol version reported in the response header (major, minor).
    pub protocol_version: (u8, u8),
    /// HDM firmware/software version (major, minor, patch).
    pub software_version: (u8, u8, u8),
    /// Response code the device returned to the unauthenticated probe.
    ///
    /// Carried for diagnostics rather than as a success/failure signal — reaching this struct
    /// already means the endpoint is an HDM. A `403` ([`crate::ServerErrorKind::UnauthorizedConnection`])
    /// is the expected value during first contact and tells the operator the caller's IP must be
    /// registered on the device's integration screen.
    pub response_code: u16,
}

/// Send an unauthenticated probe and decide whether `transport` speaks the HDM protocol.
///
/// Writes one framed operation-1 request with a bogus payload, then reads the 11-byte response
/// header. Identity is confirmed when the header's protocol *major* version matches the HDM
/// protocol family this crate targets (the major byte of [`PROTOCOL_VERSION`]). The minor version
/// is device-specific — a spec-v0.7.x terminal answers `0.7` to our `0.5`-framed request — and is
/// reported back via [`HdmIdentity::protocol_version`] rather than gated on.
///
/// Performs no encryption and needs no credentials, so it is safe to run against unknown
/// endpoints during a network sweep. It is one-shot: the connection is left with the response
/// payload undrained — use a fresh connection per call and drop it afterwards (which is the
/// natural shape of a connect-probe-discard sweep).
///
/// **Timeouts** are the transport's responsibility — configure `set_read_timeout` /
/// `set_write_timeout` on a `TcpStream` before calling, exactly as for [`crate::Client`]. Without
/// a read timeout a silent (connected-but-mute) endpoint will block indefinitely.
///
/// # Errors
/// - [`Error::Transport`] if the endpoint closes the connection, never answers, or times out —
///   the common "not a listening HDM" outcome on a swept host.
/// - [`Error::NotHdm`] if the endpoint answered but the reply did not begin with the HDM protocol
///   version, i.e. some other service is bound to that port.
pub fn identify<T: Read + Write>(transport: &mut T) -> Result<HdmIdentity, Error> {
    let request = Request {
        op: OperationCode::ListOpsAndDeps,
        payload: PROBE_PAYLOAD.to_vec(),
    };
    request.encode(transport)?;

    let header = ResponseHeader::read(transport)?;
    let protocol_version = header.protocol_version;
    // Identity is the protocol *major* version, not an exact match. A device reports its own
    // protocol version in responses (a spec-v0.7.x terminal answers 0.7) while this crate frames
    // requests at PROTOCOL_VERSION's level and the device accepts that older request version.
    // Pinning the minor here yields a false "not an HDM" on a perfectly good newer device —
    // observed on a Newland N950 that answers 0.7 to our 0.5 request. The major version identifies
    // the protocol family; the minor is device-specific and is carried through for the caller.
    let [exp_major, _exp_minor] = PROTOCOL_VERSION;
    if protocol_version.0 != exp_major {
        return Err(Error::NotHdm { protocol_version });
    }

    Ok(HdmIdentity {
        protocol_version,
        software_version: header.software_version,
        response_code: header.code,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{MAGIC, RESPONSE_HEADER_LEN};
    use std::io::{self, Cursor};

    /// Loopback transport: captures written bytes, replays a pre-loaded incoming buffer.
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

    /// Build a response header (no payload) with the given protocol version and code.
    fn response_header(protocol: [u8; 2], code: u16) -> Vec<u8> {
        let mut out = Vec::with_capacity(RESPONSE_HEADER_LEN);
        out.extend_from_slice(&protocol); // protocol version (2)
        out.extend_from_slice(&[0x02, 0x02, 0x10]); // software version (3)
        out.extend_from_slice(&code.to_be_bytes()); // response code (2)
        out.extend_from_slice(&[0x00, 0x00]); // payload length (2) — zero
        out.extend_from_slice(&[0x00, 0x00]); // reserved (2)
        out
    }

    /// A device replying with the HDM protocol version is identified, whatever the response code.
    /// `403` (IP not yet whitelisted) is the canonical first-contact case.
    #[test]
    fn identifies_hdm_from_unauthorized_response() {
        let mut transport = Loopback::new(response_header([0x00, 0x05], 403));
        let identity = identify(&mut transport).expect("should identify an HDM");
        assert_eq!(identity.protocol_version, (0, 5));
        assert_eq!(identity.software_version, (2, 2, 16));
        assert_eq!(identity.response_code, 403);
    }

    /// A device reporting a *newer* protocol minor version than this crate frames requests at is
    /// still identified — only the major version gates identity. Regression test for a real Newland
    /// N950 that answers protocol `0.7` (software `1.1.0`, code `101`) to our `0.5`-framed probe.
    #[test]
    fn identifies_hdm_reporting_newer_protocol_minor() {
        let mut header = response_header([0x00, 0x07], 101);
        // Override the canned software version with the value the real N950 reports.
        header[2] = 0x01;
        header[3] = 0x01;
        header[4] = 0x00;
        let mut transport = Loopback::new(header);
        let identity = identify(&mut transport).expect("a 0.7 device is still an HDM");
        assert_eq!(identity.protocol_version, (0, 7));
        assert_eq!(identity.software_version, (1, 1, 0));
        assert_eq!(identity.response_code, 101);
    }

    /// The probe writes a well-formed op-1 frame (magic + version + op code) with the bogus block.
    #[test]
    fn probe_frame_is_well_formed_op1() {
        let mut transport = Loopback::new(response_header([0x00, 0x05], 101));
        identify(&mut transport).expect("identify");
        let written = &transport.written;
        assert!(written.starts_with(&MAGIC));
        assert_eq!(&written[6..8], PROTOCOL_VERSION);
        assert_eq!(written[8], OperationCode::ListOpsAndDeps as u8);
        assert_eq!(written[9], 0); // reserved
        assert_eq!(u16::from_be_bytes([written[10], written[11]]), 8); // payload length
    }

    /// A foreign service whose reply does not start with the HDM protocol version is rejected as
    /// `NotHdm`, carrying the bytes seen (here an SSH banner's first two bytes).
    #[test]
    fn rejects_non_hdm_service() {
        let banner = b"SSH-2.0-OpenSSH_9.6\r\n".to_vec();
        let mut transport = Loopback::new(banner);
        let err = identify(&mut transport).expect_err("should reject non-HDM");
        match err {
            Error::NotHdm { protocol_version } => {
                assert_eq!(protocol_version, (b'S', b'S'));
            }
            other => panic!("expected NotHdm, got {other:?}"),
        }
    }

    /// An endpoint that accepts the connection but sends nothing surfaces as a transport error
    /// (short read), distinct from `NotHdm`.
    #[test]
    fn silent_endpoint_is_transport_error() {
        let mut transport = Loopback::new(Vec::new());
        let err = identify(&mut transport).expect_err("should fail to read a header");
        assert!(matches!(err, Error::Transport(_)));
    }
}
