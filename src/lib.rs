//! Client for the Armenian fiscal cash register (HDM) protocol.
//!
//! Specification: State Revenue Committee of Armenia (ՀՀՊԵԿ),
//! "ՀՍԿԻՉ - ԴՐԱՄԱՐԿՂԱՅԻՆ ՄԵՔԵՆԱՅԻ ԻՆՏԵԳՐՈՒՄԸ ԱՐՏԱՔԻՆ (ԱՌԵՎՏՐԱՅԻՆ) ԾՐԱԳՐԵՐԻ ՀԵՏ",
//! version 0.7.3 (April 2025). The original Armenian PDF and an English translation are
//! checked in under `docs/`.
//!
//! # Example
//!
//! ```no_run
//! use hdm_am::{Client, Decimal, InMemorySeq, PrintMode, PrintReceiptRequest};
//! use std::net::TcpStream;
//! use std::time::Duration;
//!
//! let mut tcp = TcpStream::connect("192.168.1.50:9000")?;
//! tcp.set_read_timeout(Some(Duration::from_secs(50)))?;
//! tcp.set_write_timeout(Some(Duration::from_secs(50)))?;
//!
//! let mut client = Client::new(tcp, "hdm-password", InMemorySeq::default());
//! client.login(1, "1234")?;
//!
//! let receipt = client.print_receipt(PrintReceiptRequest {
//!     mode: PrintMode::Simple,
//!     paid_amount: Decimal::from(1000),
//!     paid_amount_card: Decimal::ZERO,
//!     partial_amount: Decimal::ZERO,
//!     pre_payment_amount: Decimal::ZERO,
//!     dep: Some(1),
//!     partner_tin: None,
//!     use_ext_pos: false,
//!     payment_system: None,
//!     rrn: None,
//!     terminal_id: None,
//!     e_marks: vec![],
//!     items: vec![],
//! })?;
//!
//! println!("fiscal number: {}", receipt.fiscal);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Versions and forward compatibility
//!
//! Three version axes are easy to confuse, so the crate keeps them separate:
//!
//! - **Wire framing version** — the byte in the request header ([`PROTOCOL_VERSION`]). Frozen
//!   at `05` since spec v0.5 (2017); one framing covers the whole live protocol.
//! - **Spec (document) version** — which manual revision the operation/field set follows
//!   ([`SPEC_VERSION`], currently `0.7.3`). This is what grows over time.
//! - **Crate version** — ordinary semver; not coupled to the spec version (the mapping lives in the
//!   changelog, not the version number).
//!
//! The crate targets the current `0.7.x` line and is built to tolerate *newer* devices without code
//! changes:
//!
//! - **Responses are forward-compatible.** They are `#[non_exhaustive]`, never use
//!   `#[serde(deny_unknown_fields)]`, and put `#[serde(default)]` on later-added fields — so a newer
//!   firmware that returns extra fields still deserialises cleanly. Preserve this when adding ops.
//! - **Requests track a spec revision exactly.** They are ordinary (exhaustive) structs, so a new
//!   *request* field introduced by a future spec is a breaking change shipped in a major release —
//!   a deliberate signal that the targeted [`SPEC_VERSION`] moved.
//!
//! `docs/history/` holds the full version lineage (v0.3–v0.7.3) and a runbook for adopting a new
//! spec.

#![warn(missing_docs)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod client;
pub mod error;
pub mod operations;
pub mod probe;
pub mod seq;

// Framing and crypto are implementation details: the genuinely-public items (OperationCode,
// PROTOCOL_VERSION) are re-exported below; everything else stays internal.
pub(crate) mod crypto;
pub(crate) mod wire;

pub use client::Client;
pub use error::{CryptoError, Error, ServerErrorKind, VendorErrorKind};
pub use operations::{
    CashInOutRequest, DateTimeRequest, DateTimeResponse, DepartmentInfo, DiscountKind,
    EmptyResponse, FiscalReportKind, FiscalReportRequest, GetReturnableReceiptRequest,
    HdmTimeSyncRequest, ListOpsAndDepsRequest, ListOpsAndDepsResponse, Operation, OperatorInfo,
    OperatorLoginRequest, OperatorLoginResponse, OperatorLogoutRequest, PaymentSystemEntry,
    PaymentSystemsListRequest, PaymentSystemsListResponse, PrintLastReceiptRequest, PrintMode,
    PrintReceiptRequest, PrintReturnReceiptRequest, ReceiptItem, ReceiptResponse,
    ReceiptSampleRequest, ReportFilter, ReturnItem, ReturnReceiptResponse, ReturnableReceiptItem,
    ReturnableReceiptResponse, SetupHeaderFooterRequest, SetupHeaderLogoRequest,
    SingleEmarkRequest, TaxationKind, TextAlign, TextLine,
};
pub use probe::{HdmIdentity, identify};
pub use seq::{FileSeq, InMemorySeq, SequenceProvider};
pub use wire::{OperationCode, PROTOCOL_VERSION};

/// Re-exported so consumers can name monetary/quantity values without depending on `rust_decimal`
/// directly. All amount and quantity fields use this type.
///
/// The wire encoding is a JSON **number** (the HDM format), so values round-trip through `f64` and
/// are bounded by `f64` precision — about 15–16 significant digits. This is inherent to the protocol,
/// not the crate; it is far above any realistic AMD receipt magnitude (totals ≪ 10¹³ with 2 decimal
/// places), but a pathological quantity × price could lose precision.
pub use rust_decimal::Decimal;

/// HDM specification (document) version this crate targets — the operation and field set follow
/// this manual revision.
///
/// Distinct from the wire framing version [`PROTOCOL_VERSION`] (`05`, frozen since spec v0.5)
/// and from the crate's own semver. The full version lineage and an upgrade runbook are in
/// `docs/history/`.
pub const SPEC_VERSION: &str = "0.7.3";
