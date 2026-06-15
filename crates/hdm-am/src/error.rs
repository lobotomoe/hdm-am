//! Error taxonomy with recovery semantics.
//!
//! The crate exposes a single [`enum@Error`] type that the consumer matches on. Each variant carries
//! enough information to make a recovery decision via [`Error::is_retryable`],
//! [`Error::requires_relogin`], and [`Error::requires_reconnect`].
//!
//! Server-side error codes from the spec (§4.10) are modelled as [`ServerErrorKind`] — an
//! exhaustive enum covering every documented response code, plus an `Unknown(u16)` variant for
//! forward compatibility with future spec revisions.

use thiserror::Error;

/// All errors the HDM client can produce.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// Underlying transport (TCP, mock, etc.) failure.
    ///
    /// Includes timeouts, connection resets, DNS failures. Recovery: reconnect + retry, possibly
    /// with backoff.
    #[error("transport: {0}")]
    Transport(#[from] std::io::Error),

    /// HDM returned a non-success response code. See [`ServerErrorKind`] for the categorisation.
    #[error("server error: code {code} ({kind:?})")]
    Server {
        /// Raw numeric code as returned in the response header (e.g. `141`, `185`).
        code: u16,
        /// Categorised kind. `ServerErrorKind::Unknown(code)` for codes outside the documented set.
        kind: ServerErrorKind,
    },

    /// 3DES decryption or padding validation failed.
    ///
    /// In practice this almost always means the session key is stale (server-side session
    /// timed out, or sequence numbers drifted). Recovery: re-login.
    #[error("cryptographic: {0}")]
    Crypto(#[from] CryptoError),

    /// Response payload could not be parsed as JSON, or required fields were missing.
    ///
    /// Usually indicates spec drift between the crate and the device, or a corrupted payload
    /// that survived decryption. Not recoverable without intervention.
    #[error("response decode: {0}")]
    Decode(serde_json::Error),

    /// Request payload could not be serialised to JSON. Indicates a programming bug — the
    /// crate's request structs are always serialisable by construction.
    #[error("request encode: {0}")]
    Encode(serde_json::Error),

    /// Operation requires an active session but [`crate::Client::login`] has not been called
    /// successfully.
    #[error("operation requires login")]
    NotLoggedIn,

    /// A request payload exceeded the protocol's 2-byte length field (65 535 bytes).
    #[error("request payload too large ({len} bytes, max 65 535)")]
    PayloadTooLarge {
        /// The actual size that overflowed.
        len: usize,
    },

    /// [`crate::identify`] reached a responsive endpoint, but its reply did not begin with the
    /// HDM protocol version — some other TCP service is listening on that address. Distinct from
    /// [`Self::Transport`] (nothing answered) so a discovery sweep can tell "wrong service" from
    /// "unreachable".
    #[error("endpoint is not an HDM (reported protocol version {protocol_version:?})")]
    NotHdm {
        /// The first two response bytes the endpoint sent, interpreted as a protocol version.
        protocol_version: (u8, u8),
    },
}

impl Error {
    /// Whether retrying the *same operation* with no state change might succeed.
    ///
    /// True for transient transport failures (timeout, broken pipe) and a small set of
    /// server-side conditions that resolve themselves (e.g. printer-out-of-paper after operator
    /// intervention). False for logical errors that need data fixes.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Transport(io) => is_retryable_io_kind(io.kind()),
            Self::Server { kind, .. } => kind.is_retryable(),
            _ => false,
        }
    }

    /// Whether the client should drop the current session and call [`crate::Client::login`]
    /// again before the next operation.
    #[must_use]
    pub const fn requires_relogin(&self) -> bool {
        match self {
            Self::NotLoggedIn | Self::Crypto(_) => true,
            Self::Server { kind, .. } => kind.requires_relogin(),
            _ => false,
        }
    }

    /// Whether the underlying TCP connection is in an unrecoverable state and must be
    /// re-established.
    #[must_use]
    pub const fn requires_reconnect(&self) -> bool {
        match self {
            Self::Transport(_) => true,
            Self::Server { kind, .. } => kind.is_fatal_for_connection(),
            _ => false,
        }
    }
}

const fn is_retryable_io_kind(kind: std::io::ErrorKind) -> bool {
    matches!(
        kind,
        std::io::ErrorKind::TimedOut
            | std::io::ErrorKind::WouldBlock
            | std::io::ErrorKind::Interrupted
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::BrokenPipe
    )
}

/// Cryptographic failure modes encountered when encrypting requests or decrypting responses.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum CryptoError {
    /// 3DES key was not 24 bytes long. Indicates a programming bug — the crate's internals
    /// always produce 24-byte keys.
    #[error("3DES key must be exactly 24 bytes")]
    InvalidKeyLength,

    /// Ciphertext length was not a multiple of the 3DES block size (8 bytes).
    #[error("ciphertext length is not a multiple of 8")]
    InvalidBlockSize,

    /// PKCS7 padding verification failed during decryption. Usually means the session key is
    /// stale or the ciphertext was tampered with mid-flight.
    #[error("PKCS7 padding is malformed (likely stale session key)")]
    BadPadding,

    /// Session key returned by the server did not Base64-decode cleanly.
    #[error("session key Base64 decode failed: {0}")]
    SessionKeyBase64(#[from] base64::DecodeError),
}

/// Categorised server response codes per spec §4.10.
///
/// Every documented code maps to a named variant. Codes outside the documented set become
/// `Unknown(u16)` so that future spec revisions don't break existing builds.
///
/// The variants are grouped roughly by the spec's own categories: generic errors, login errors,
/// receipt-print errors. Recovery semantics are exposed via [`Self::is_retryable`],
/// [`Self::requires_relogin`], and [`Self::is_fatal_for_connection`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ServerErrorKind {
    // --- General (§4.10 "Ընդհանուր") ---
    /// `500` — Internal HDM error.
    InternalHdmError,
    /// `400` — Request error (request not processed).
    BadRequest,
    /// `402` — Bad protocol version.
    BadProtocolVersion,
    /// `403` — Unauthorised connection (HDM-registered IP doesn't match the caller's).
    UnauthorizedConnection,
    /// `404` — Bad operation code in the header.
    BadOperationCode,
    /// `101` — Cryptographic encryption error.
    CryptographicError,
    /// `102` — Session-key encryption error.
    SessionEncryptionError,
    /// `103` — Header format error.
    HeaderFormatError,
    /// `104` — Request sequence number error.
    BadSequenceNumber,
    /// `105` — JSON format error.
    BadJsonFormat,
    /// `141` — Last receipt archive is empty.
    LastReceiptArchiveEmpty,
    /// `142` — Last receipt belongs to a different user.
    LastReceiptDifferentUser,
    /// `143` — Generic print error.
    GenericPrintError,
    /// `144` — Printer initialisation error.
    PrinterInitError,
    /// `145` — Printer is out of paper.
    PrinterOutOfPaper,

    // --- Operator / login errors ---
    /// `111` — Operator password error.
    BadOperatorPassword,
    /// `112` — No such operator (role mismatch, inactive user, or not registered).
    NoSuchOperator,
    /// `113` — Operator is inactive.
    InactiveOperator,
    /// `121` — Print error (during login flow).
    GenericLoginPrintError,

    // --- Receipt-print errors ---
    /// `151` — No such department (or operator lacks access).
    NoSuchDepartment,
    /// `152` — Paid amount is less than the total.
    PaidLessThanTotal,
    /// `153` — Receipt amount exceeds the configured limit.
    AmountExceedsLimit,
    /// `154` — Receipt amount must be positive.
    AmountMustBePositive,
    /// `155` — HDM synchronisation required before this operation can succeed.
    HdmSyncRequired,
    /// `156` — Synchronisation not completed.
    SyncIncomplete,
    /// `157` — Bad return-receipt number.
    BadReturnReceiptNumber,
    /// `158` — Receipt already returned.
    ReceiptAlreadyReturned,
    /// `159` — Non-positive product price or quantity.
    NonPositiveProductPrice,
    /// `160` — Discount percent out of range (must be 0..100).
    DiscountPercentOutOfRange,
    /// `161` — Bad product code.
    BadProductCode,
    /// `162` — Bad product name.
    BadProductName,
    /// `163` — Empty product unit-of-measure.
    EmptyProductUnit,
    /// `164` — Cashless payment failure.
    CashlessPaymentFailure,
    /// `165` — Product price cannot be zero.
    ZeroProductPrice,
    /// `166` — Final price calculation error.
    FinalPriceCalculationError,
    /// `167` — Card amount is greater than the receipt's total.
    CardAmountExceedsTotal,
    /// `168` — Card amount covers the total (cash amount is redundant).
    CardAmountCoversAllCashRedundant,
    /// `169` — Fiscal-report filter conflict (more than one filter sent).
    ReportFiltersError,
    /// `170` — Fiscal-report time range exceeds 2 months.
    ReportTimeRangeError,
    /// `171` — Invalid item price value.
    InvalidItemPrice,
    /// `172` — Wrong receipt type (not product/simple/prepayment).
    WrongReceiptType,
    /// `173` — Invalid discount type.
    InvalidDiscountType,
    /// `174` — Return-target receipt does not exist.
    ReturnReceiptNotFound,
    /// `175` — Bad registration number on the return-target receipt.
    BadReturnReceiptRegNum,
    /// `176` — Last receipt does not exist.
    LastReceiptNotFound,
    /// `177` — Return not supported for this receipt type.
    ReturnNotSupportedForType,
    /// `178` — Requested return amount cannot be processed.
    AmountCannotBeReturned,
    /// `179` — Partial-payment receipt must be returned in full.
    PartialMustBeReturnedInFull,
    /// `180` — Full-return amount exceeds available.
    FullReturnExceedsAmount,
    /// `181` — Bad return-product quantity.
    BadReturnProductQuantity,
    /// `182` — Return receipt is itself a return-type receipt.
    ReturnReceiptIsReturn,
    /// `183` — Bad ATG/ADG code (see `taxservice.am` for the canonical list).
    BadAtgCode,
    /// `184` — Inappropriate prepayment-return request.
    InvalidPrepaymentReturn,
    /// `185` — Cannot return partial-payment receipt; HDM software sync required.
    PartialReturnSyncRequired,
    /// `186` — Bad amount in prepayment case.
    BadPrepaymentAmount,
    /// `187` — Bad list in prepayment case.
    BadPrepaymentList,
    /// `188` — Bad amounts in general.
    BadAmounts,
    /// `189` — Bad rounding.
    BadRounding,
    /// `190` — Payment not available.
    PaymentUnavailable,
    /// `191` — Cash in/out amount must be greater than zero.
    NonPositiveCashAmount,
    /// `192` — ATG code is mandatory.
    AtgCodeRequired,
    /// `193` — Bad partner-TIN format.
    BadPartnerTinFormat,
    /// `194` — eMark codes not allowed in prepayment receipts.
    EmarksNotAllowedInPrepayment,
    /// `195` — Bad eMark code format.
    BadEmarkFormat,
    /// `196` — eMark code belongs to another country ("Այլ երկրի ծածկագիր").
    ForeignCountryEmark,

    /// A vendor/firmware-specific code that is not part of the SRC spec §4.10. Kept in its own
    /// [`VendorErrorKind`] enum so spec codes and vendor extensions never blur together; the two
    /// sets are joined only here and in [`Self::from_code`].
    Vendor(VendorErrorKind),

    /// Response code present in neither spec v0.7.3 nor the known vendor set. Carried verbatim for
    /// forward compatibility.
    Unknown(u16),
}

/// Vendor/firmware-specific response codes observed on real devices but absent from the SRC
/// integration spec (§4.10).
///
/// These are deliberately segregated from [`ServerErrorKind`]'s documented variants: the spec is
/// the stable contract, whereas these depend on a particular firmware. Descriptions come from the
/// device's own built-in response-code reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum VendorErrorKind {
    /// `408` — The HDM's own UI is in use (a user is operating the device's screens), so
    /// external-program access is temporarily blocked. Newland N950:
    /// "Գործողություն է կատարվում ՀԴՄ-ի էջերում: Արտաքին ծրագրի աշխատանքը բլոկավորված է:".
    /// Transient — retry once the device screen is idle.
    ExternalProgramBlocked,

    /// `503` — Mismatch in the data received from the (tax-authority) server. Newland N950:
    /// "Սերվերից ստացված տվյալների անհամապատասխանություն". Indicates the device's view of a
    /// record disagrees with the central server; not resolved by a blind retry.
    ServerDataMismatch,
}

impl VendorErrorKind {
    /// Map a numeric response code to a vendor variant, if it is a known vendor code.
    #[must_use]
    pub const fn from_code(code: u16) -> Option<Self> {
        match code {
            408 => Some(Self::ExternalProgramBlocked),
            503 => Some(Self::ServerDataMismatch),
            _ => None,
        }
    }

    /// The numeric code for this vendor variant.
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Self::ExternalProgramBlocked => 408,
            Self::ServerDataMismatch => 503,
        }
    }

    /// Whether retrying unchanged might succeed. Only the transient "UI busy" condition qualifies.
    #[must_use]
    pub const fn is_retryable(self) -> bool {
        matches!(self, Self::ExternalProgramBlocked)
    }
}

impl ServerErrorKind {
    /// Map a numeric response code to its categorised variant.
    #[must_use]
    pub const fn from_code(code: u16) -> Self {
        match code {
            500 => Self::InternalHdmError,
            400 => Self::BadRequest,
            402 => Self::BadProtocolVersion,
            403 => Self::UnauthorizedConnection,
            404 => Self::BadOperationCode,
            101 => Self::CryptographicError,
            102 => Self::SessionEncryptionError,
            103 => Self::HeaderFormatError,
            104 => Self::BadSequenceNumber,
            105 => Self::BadJsonFormat,
            141 => Self::LastReceiptArchiveEmpty,
            142 => Self::LastReceiptDifferentUser,
            143 => Self::GenericPrintError,
            144 => Self::PrinterInitError,
            145 => Self::PrinterOutOfPaper,
            111 => Self::BadOperatorPassword,
            112 => Self::NoSuchOperator,
            113 => Self::InactiveOperator,
            121 => Self::GenericLoginPrintError,
            151 => Self::NoSuchDepartment,
            152 => Self::PaidLessThanTotal,
            153 => Self::AmountExceedsLimit,
            154 => Self::AmountMustBePositive,
            155 => Self::HdmSyncRequired,
            156 => Self::SyncIncomplete,
            157 => Self::BadReturnReceiptNumber,
            158 => Self::ReceiptAlreadyReturned,
            159 => Self::NonPositiveProductPrice,
            160 => Self::DiscountPercentOutOfRange,
            161 => Self::BadProductCode,
            162 => Self::BadProductName,
            163 => Self::EmptyProductUnit,
            164 => Self::CashlessPaymentFailure,
            165 => Self::ZeroProductPrice,
            166 => Self::FinalPriceCalculationError,
            167 => Self::CardAmountExceedsTotal,
            168 => Self::CardAmountCoversAllCashRedundant,
            169 => Self::ReportFiltersError,
            170 => Self::ReportTimeRangeError,
            171 => Self::InvalidItemPrice,
            172 => Self::WrongReceiptType,
            173 => Self::InvalidDiscountType,
            174 => Self::ReturnReceiptNotFound,
            175 => Self::BadReturnReceiptRegNum,
            176 => Self::LastReceiptNotFound,
            177 => Self::ReturnNotSupportedForType,
            178 => Self::AmountCannotBeReturned,
            179 => Self::PartialMustBeReturnedInFull,
            180 => Self::FullReturnExceedsAmount,
            181 => Self::BadReturnProductQuantity,
            182 => Self::ReturnReceiptIsReturn,
            183 => Self::BadAtgCode,
            184 => Self::InvalidPrepaymentReturn,
            185 => Self::PartialReturnSyncRequired,
            186 => Self::BadPrepaymentAmount,
            187 => Self::BadPrepaymentList,
            188 => Self::BadAmounts,
            189 => Self::BadRounding,
            190 => Self::PaymentUnavailable,
            191 => Self::NonPositiveCashAmount,
            192 => Self::AtgCodeRequired,
            193 => Self::BadPartnerTinFormat,
            194 => Self::EmarksNotAllowedInPrepayment,
            195 => Self::BadEmarkFormat,
            196 => Self::ForeignCountryEmark,
            other => match VendorErrorKind::from_code(other) {
                Some(vendor) => Self::Vendor(vendor),
                None => Self::Unknown(other),
            },
        }
    }

    /// Reverse mapping back to the numeric code.
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Self::InternalHdmError => 500,
            Self::BadRequest => 400,
            Self::BadProtocolVersion => 402,
            Self::UnauthorizedConnection => 403,
            Self::BadOperationCode => 404,
            Self::CryptographicError => 101,
            Self::SessionEncryptionError => 102,
            Self::HeaderFormatError => 103,
            Self::BadSequenceNumber => 104,
            Self::BadJsonFormat => 105,
            Self::LastReceiptArchiveEmpty => 141,
            Self::LastReceiptDifferentUser => 142,
            Self::GenericPrintError => 143,
            Self::PrinterInitError => 144,
            Self::PrinterOutOfPaper => 145,
            Self::BadOperatorPassword => 111,
            Self::NoSuchOperator => 112,
            Self::InactiveOperator => 113,
            Self::GenericLoginPrintError => 121,
            Self::NoSuchDepartment => 151,
            Self::PaidLessThanTotal => 152,
            Self::AmountExceedsLimit => 153,
            Self::AmountMustBePositive => 154,
            Self::HdmSyncRequired => 155,
            Self::SyncIncomplete => 156,
            Self::BadReturnReceiptNumber => 157,
            Self::ReceiptAlreadyReturned => 158,
            Self::NonPositiveProductPrice => 159,
            Self::DiscountPercentOutOfRange => 160,
            Self::BadProductCode => 161,
            Self::BadProductName => 162,
            Self::EmptyProductUnit => 163,
            Self::CashlessPaymentFailure => 164,
            Self::ZeroProductPrice => 165,
            Self::FinalPriceCalculationError => 166,
            Self::CardAmountExceedsTotal => 167,
            Self::CardAmountCoversAllCashRedundant => 168,
            Self::ReportFiltersError => 169,
            Self::ReportTimeRangeError => 170,
            Self::InvalidItemPrice => 171,
            Self::WrongReceiptType => 172,
            Self::InvalidDiscountType => 173,
            Self::ReturnReceiptNotFound => 174,
            Self::BadReturnReceiptRegNum => 175,
            Self::LastReceiptNotFound => 176,
            Self::ReturnNotSupportedForType => 177,
            Self::AmountCannotBeReturned => 178,
            Self::PartialMustBeReturnedInFull => 179,
            Self::FullReturnExceedsAmount => 180,
            Self::BadReturnProductQuantity => 181,
            Self::ReturnReceiptIsReturn => 182,
            Self::BadAtgCode => 183,
            Self::InvalidPrepaymentReturn => 184,
            Self::PartialReturnSyncRequired => 185,
            Self::BadPrepaymentAmount => 186,
            Self::BadPrepaymentList => 187,
            Self::BadAmounts => 188,
            Self::BadRounding => 189,
            Self::PaymentUnavailable => 190,
            Self::NonPositiveCashAmount => 191,
            Self::AtgCodeRequired => 192,
            Self::BadPartnerTinFormat => 193,
            Self::EmarksNotAllowedInPrepayment => 194,
            Self::BadEmarkFormat => 195,
            Self::ForeignCountryEmark => 196,
            Self::Vendor(vendor) => vendor.code(),
            Self::Unknown(c) => c,
        }
    }

    /// Whether retrying the same operation might succeed without changing state.
    ///
    /// Conservatively false for most variants. True only for transient conditions
    /// (printer-out-of-paper, sync-incomplete, or the HDM's UI being momentarily busy) where the
    /// situation may clear itself between attempts.
    #[must_use]
    pub const fn is_retryable(self) -> bool {
        match self {
            Self::PrinterOutOfPaper | Self::SyncIncomplete => true,
            Self::Vendor(vendor) => vendor.is_retryable(),
            _ => false,
        }
    }

    /// Whether the client should call [`crate::Client::login`] again before the next operation.
    ///
    /// True for session-key invalidation, expired credentials, and operator-state changes
    /// detected mid-session.
    #[must_use]
    pub const fn requires_relogin(self) -> bool {
        matches!(
            self,
            Self::SessionEncryptionError
                | Self::BadOperatorPassword
                | Self::NoSuchOperator
                | Self::InactiveOperator
                | Self::GenericLoginPrintError
        )
    }

    /// Whether the underlying TCP connection should be closed and re-opened.
    ///
    /// Aligned with the "Stops the server connection" column of spec §4.10.
    #[must_use]
    pub const fn is_fatal_for_connection(self) -> bool {
        // Exactly the codes marked "X" in the spec §4.10 "stops the server connection" column
        // (verified against the original PDF). Note: the print errors 143/144/145 are NOT in this
        // set — they are recoverable (e.g. `PrinterOutOfPaper` is retryable after a paper refill).
        matches!(
            self,
            Self::InternalHdmError
                | Self::BadRequest
                | Self::BadProtocolVersion
                | Self::UnauthorizedConnection
                | Self::BadOperationCode
                | Self::CryptographicError
                | Self::HeaderFormatError
                | Self::BadSequenceNumber
                | Self::BadJsonFormat
                | Self::BadOperatorPassword
                | Self::NoSuchOperator
                | Self::InactiveOperator
                | Self::GenericLoginPrintError
                | Self::HdmSyncRequired
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every documented code round-trips through `from_code` and back to `code`.
    #[test]
    fn server_error_kind_round_trips_for_documented_codes() {
        let documented_codes: &[u16] = &[
            500, 400, 402, 403, 404, 101, 102, 103, 104, 105, 141, 142, 143, 144, 145, 111, 112,
            113, 121, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165,
            166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182,
            183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196,
        ];
        for &code in documented_codes {
            let kind = ServerErrorKind::from_code(code);
            assert_ne!(
                kind,
                ServerErrorKind::Unknown(code),
                "code {code} should map to a named variant"
            );
            assert_eq!(kind.code(), code, "round-trip for code {code}");
        }
    }

    /// Undocumented codes survive as `Unknown(code)` without panicking.
    #[test]
    fn server_error_kind_preserves_unknown_codes() {
        let kind = ServerErrorKind::from_code(999);
        assert_eq!(kind, ServerErrorKind::Unknown(999));
        assert_eq!(kind.code(), 999);
    }

    /// Vendor codes (not in spec §4.10) map to `Vendor(..)`, not `Unknown`, and round-trip.
    #[test]
    fn vendor_codes_map_to_vendor_variant() {
        for (code, vendor) in [
            (408, VendorErrorKind::ExternalProgramBlocked),
            (503, VendorErrorKind::ServerDataMismatch),
        ] {
            assert_eq!(
                ServerErrorKind::from_code(code),
                ServerErrorKind::Vendor(vendor)
            );
            assert_eq!(ServerErrorKind::Vendor(vendor).code(), code);
            assert_eq!(vendor.code(), code);
            assert_eq!(VendorErrorKind::from_code(code), Some(vendor));
        }
        // A non-vendor unknown code stays Unknown.
        assert_eq!(VendorErrorKind::from_code(999), None);
    }

    /// Login-related errors should hint at re-login.
    #[test]
    fn relogin_predicate_covers_session_errors() {
        assert!(ServerErrorKind::SessionEncryptionError.requires_relogin());
        assert!(ServerErrorKind::BadOperatorPassword.requires_relogin());
        assert!(ServerErrorKind::NoSuchOperator.requires_relogin());
        assert!(ServerErrorKind::InactiveOperator.requires_relogin());
        // Business errors do NOT need re-login.
        assert!(!ServerErrorKind::NoSuchDepartment.requires_relogin());
        assert!(!ServerErrorKind::BadAtgCode.requires_relogin());
    }

    /// Transient hardware conditions are retryable; everything else is conservatively not.
    #[test]
    fn retry_predicate_is_narrow() {
        assert!(ServerErrorKind::PrinterOutOfPaper.is_retryable());
        assert!(ServerErrorKind::SyncIncomplete.is_retryable());
        assert!(ServerErrorKind::Vendor(VendorErrorKind::ExternalProgramBlocked).is_retryable());
        assert!(!ServerErrorKind::Vendor(VendorErrorKind::ServerDataMismatch).is_retryable());
        assert!(!ServerErrorKind::BadAmounts.is_retryable());
        assert!(!ServerErrorKind::Unknown(999).is_retryable());
    }

    /// `Error::Crypto` requires re-login by default — stale session keys are the dominant cause.
    #[test]
    fn crypto_error_demands_relogin() {
        let err = Error::Crypto(CryptoError::BadPadding);
        assert!(err.requires_relogin());
        assert!(!err.requires_reconnect());
        assert!(!err.is_retryable());
    }

    /// `is_fatal_for_connection` matches the spec §4.10 "stops connection" column exactly.
    #[test]
    fn fatal_for_connection_matches_spec_column() {
        // Codes marked "X" in the spec.
        for code in [
            500, 400, 402, 403, 404, 101, 103, 104, 105, 111, 112, 113, 121, 155,
        ] {
            assert!(
                ServerErrorKind::from_code(code).is_fatal_for_connection(),
                "code {code} should be fatal per §4.10"
            );
        }
        // Print errors 143/144/145 are recoverable, NOT fatal (regression guard).
        for code in [102, 141, 142, 143, 144, 145, 151, 156] {
            assert!(
                !ServerErrorKind::from_code(code).is_fatal_for_connection(),
                "code {code} should NOT be fatal per §4.10"
            );
        }
        // Out-of-paper is recoverable: retryable and not a reconnect trigger.
        assert!(ServerErrorKind::PrinterOutOfPaper.is_retryable());
        assert!(!ServerErrorKind::PrinterOutOfPaper.is_fatal_for_connection());
    }

    /// A transport failure requires reconnect, not relogin or a bare retry of the same call.
    #[test]
    fn transport_error_requires_reconnect() {
        let err = Error::Transport(std::io::Error::from(std::io::ErrorKind::ConnectionReset));
        assert!(err.requires_reconnect());
        assert!(!err.requires_relogin());
        assert!(err.is_retryable());
    }
}
