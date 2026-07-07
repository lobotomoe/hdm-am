//! Wire framing: magic, operation codes, request/response envelopes.

use crate::error::Error;
use std::io::{Read, Write};

/// 6-byte magic prefix: UTF-8 encoding of "ՀԴՄ" (HDM).
pub const MAGIC: [u8; 6] = [0xD5, 0x80, 0xD4, 0xB4, 0xD5, 0x84];

/// Wire framing version advertised in the request header (big-endian).
///
/// This is the request *framing* version, not the document version ([`crate::SPEC_VERSION`]): it
/// went `03→04→05` across early manuals and has been frozen at `05` since spec v0.5 (2017), through
/// v0.7.3 — every manual from v0.5 onward documents the request-header version as `05`. We follow
/// the spec and send `05`. (A live N950, firmware 1.1.3, was observed to also accept `07`, but that
/// is device leniency; the spec is authoritative and `05` is what it documents.)
///
/// A device reports its own protocol version in the *response* header, which may differ from what
/// we send — documented behaviour since v0.3 (a v0.7.x terminal answers `0.7` to our `05` request).
pub const PROTOCOL_VERSION: [u8; 2] = [0x00, 0x05];

/// Length of the response header preceding the encrypted payload.
pub const RESPONSE_HEADER_LEN: usize = 2 + 3 + 2 + 2 + 2;

/// Successful response code (200) per spec §4.10.
pub const RESPONSE_CODE_OK: u16 = 200;

/// HDM protocol operation codes. Codes 1-10 are v0.5; 11-16 were added through v0.7.3.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationCode {
    /// List active operators and departments.
    ListOpsAndDeps = 1,
    /// Operator login. Returns the session key used for all subsequent ops.
    OperatorLogin = 2,
    /// Operator logout.
    OperatorLogout = 3,
    /// Print a fiscal receipt.
    PrintReceipt = 4,
    /// Reprint a copy of the last receipt.
    PrintLastReceipt = 5,
    /// Print a return / refund receipt (full, by-amount, or per-item). Registers the return.
    PrintReturnReceipt = 6,
    /// Configure receipt header and footer text lines.
    SetupHeaderFooter = 7,
    /// Upload the receipt header logo image (Base64 BMP, colour depth ≤4 bits).
    SetupHeaderLogo = 8,
    /// Print a fiscal report (1 = X-report, 2 = Z-report).
    PrintFiscalReport = 9,
    /// Get (look up) the contents of a receipt you intend to return. Read-only.
    GetReturnableReceipt = 10,
    /// Cash drawer in/out adjustment.
    CashInOut = 11,
    /// Query device date and time.
    DateTime = 12,
    /// Receipt sample (test print).
    ReceiptSample = 13,
    /// Synchronise HDM with tax authority.
    HdmTimeSync = 14,
    /// List available payment systems (returns codes 1-18, see spec §4.8).
    PaymentSystemsList = 15,
    /// Submit a single eMark traceability code.
    SingleEmark = 16,
}

/// Encoded request to be written to the wire.
#[derive(Debug)]
pub struct Request {
    /// Operation code.
    pub op: OperationCode,
    /// Encrypted payload (ciphertext only — the header is added on encode).
    pub payload: Vec<u8>,
}

impl Request {
    /// Encode into the on-the-wire byte stream.
    ///
    /// # Errors
    /// Returns [`Error::Transport`] if `payload` exceeds `u16::MAX` bytes (HDM frames have a 2-byte length field).
    pub fn encode(&self, writer: &mut impl Write) -> Result<(), Error> {
        let len = u16::try_from(self.payload.len()).map_err(|_| {
            Error::Transport(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "payload exceeds u16::MAX",
            ))
        })?;
        writer.write_all(&MAGIC)?;
        writer.write_all(&PROTOCOL_VERSION)?;
        writer.write_all(&[self.op as u8, 0])?;
        writer.write_all(&len.to_be_bytes())?;
        writer.write_all(&self.payload)?;
        Ok(())
    }
}

/// Decoded response header. The encrypted payload follows immediately on the wire.
#[derive(Debug, Clone, Copy)]
pub struct ResponseHeader {
    /// HDM protocol version reported by the device (major, minor).
    pub protocol_version: (u8, u8),
    /// HDM firmware version (major, minor, patch).
    pub software_version: (u8, u8, u8),
    /// Response code per spec §4.10. 200 = success.
    pub code: u16,
    /// Length of the encrypted payload that follows.
    pub payload_len: u16,
}

impl ResponseHeader {
    /// Read and parse the 11-byte response header from a transport.
    ///
    /// # Errors
    /// Returns [`Error::Transport`] on read failure.
    pub fn read(reader: &mut impl Read) -> Result<Self, Error> {
        let mut buf = [0u8; RESPONSE_HEADER_LEN];
        reader.read_exact(&mut buf)?;
        let [
            pv_major,
            pv_minor,
            sw_major,
            sw_minor,
            sw_patch,
            code_hi,
            code_lo,
            len_hi,
            len_lo,
            _r0,
            _r1,
        ] = buf;
        Ok(Self {
            protocol_version: (pv_major, pv_minor),
            software_version: (sw_major, sw_minor, sw_patch),
            code: u16::from_be_bytes([code_hi, code_lo]),
            payload_len: u16::from_be_bytes([len_hi, len_lo]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn magic_is_hdm_in_utf8() {
        assert_eq!(std::str::from_utf8(&MAGIC).unwrap(), "ՀԴՄ");
    }

    #[test]
    fn request_encode_round_trips_through_header() {
        let req = Request {
            op: OperationCode::OperatorLogin,
            payload: vec![0xAA; 32],
        };
        let mut buf = Vec::new();
        req.encode(&mut buf).unwrap();

        let [
            m0,
            m1,
            m2,
            m3,
            m4,
            m5,
            pv0,
            pv1,
            op,
            reserved,
            len_hi,
            len_lo,
            ..,
        ] = buf.as_slice()
        else {
            panic!("encoded buffer too short");
        };
        assert_eq!([*m0, *m1, *m2, *m3, *m4, *m5], MAGIC);
        assert_eq!([*pv0, *pv1], PROTOCOL_VERSION);
        assert_eq!(*op, OperationCode::OperatorLogin as u8);
        assert_eq!(*reserved, 0);
        assert_eq!(u16::from_be_bytes([*len_hi, *len_lo]), 32);
    }

    #[test]
    fn response_header_decodes() {
        let raw = [
            0x00, 0x05, 0x02, 0x02, 0x10, 0x00, 0xC8, 0x00, 0x10, 0x00, 0x00,
        ];
        let mut cursor = Cursor::new(raw);
        let hdr = ResponseHeader::read(&mut cursor).unwrap();
        assert_eq!(hdr.protocol_version, (0, 5));
        assert_eq!(hdr.software_version, (2, 2, 16));
        assert_eq!(hdr.code, RESPONSE_CODE_OK);
        assert_eq!(hdr.payload_len, 16);
    }
}
