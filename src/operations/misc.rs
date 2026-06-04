use crate::wire::OperationCode;
use serde::{Deserialize, Serialize};

use super::{EmptyResponse, Operation};

/// Op 12 request: query the device's current date and time.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct DateTimeRequest {}

impl Operation for DateTimeRequest {
    const CODE: OperationCode = OperationCode::DateTime;
    type Response = DateTimeResponse;
}

/// Op 12 response.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct DateTimeResponse {
    /// Device date and time as a string. Spec does not pin the format; treat as opaque.
    pub dt: String,
}

/// Op 13 request: trigger a sample-receipt print.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ReceiptSampleRequest {}

impl Operation for ReceiptSampleRequest {
    const CODE: OperationCode = OperationCode::ReceiptSample;
    type Response = EmptyResponse;
}

/// Op 14 request: synchronise the HDM with the tax authority.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct HdmTimeSyncRequest {}

impl Operation for HdmTimeSyncRequest {
    const CODE: OperationCode = OperationCode::HdmTimeSync;
    type Response = EmptyResponse;
}

/// Op 15 request: discover the payment-system codes installed on the HDM device.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PaymentSystemsListRequest {}

impl Operation for PaymentSystemsListRequest {
    const CODE: OperationCode = OperationCode::PaymentSystemsList;
    type Response = PaymentSystemsListResponse;
}

/// Op 15 response: list of payment systems configured on the device.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct PaymentSystemsListResponse {
    /// One entry per configured payment system.
    #[serde(rename = "PaymentSystems", default)]
    pub payment_systems: Vec<PaymentSystemEntry>,
}

/// A single payment-system entry from op 15. Codes are documented in spec §4.8 (1 = card,
/// 10-18 = various Armenian mobile wallets).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct PaymentSystemEntry {
    /// Payment-system code (used in [`super::receipt::PrintReceiptRequest::payment_system`]).
    pub code: u32,
    /// Display name.
    #[serde(default)]
    pub name: String,
}

/// Op 16 request: submit a single eMark traceability code.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct SingleEmarkRequest {
    /// eMark code. Per spec §4.9: 29-110 chars, ASCII-printable only (33-126 plus 29 as
    /// delimiter); `"` escaped as `\"`, `\` as `\\`, and ASCII 29 used as the group separator.
    #[serde(rename = "eMark")]
    pub e_mark: String,
}

impl Operation for SingleEmarkRequest {
    const CODE: OperationCode = OperationCode::SingleEmark;
    type Response = EmptyResponse;
}
