use crate::wire::OperationCode;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::{EmptyResponse, Operation};

/// Op 11 request: record a cash-drawer adjustment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CashInOutRequest {
    /// Amount; must be greater than zero. Sent as a JSON number on the wire.
    #[serde(with = "rust_decimal::serde::float")]
    #[cfg_attr(feature = "schema", schemars(with = "f64"))]
    pub amount: Decimal,
    /// `true` for cash-in (deposit), `false` for cash-out (withdrawal).
    #[serde(rename = "isCashIn")]
    pub is_cash_in: bool,
    /// Cashier number.
    #[serde(rename = "cashierId", skip_serializing_if = "Option::is_none")]
    pub cashier_id: Option<u32>,
    /// Free-text comment.
    #[serde(default)]
    pub description: String,
}

impl Operation for CashInOutRequest {
    const CODE: OperationCode = OperationCode::CashInOut;
    type Response = EmptyResponse;
}
