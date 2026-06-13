use crate::wire::OperationCode;
use serde::{Deserialize, Serialize, Serializer};

use super::{EmptyResponse, Operation};

/// Fiscal-report kinds (spec §4.6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FiscalReportKind {
    /// `1` — X-report (interim summary, does not zero counters).
    X = 1,
    /// `2` — Z-report (end-of-day; zeros counters and finalises the fiscal day).
    Z = 2,
}

impl Serialize for FiscalReportKind {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for FiscalReportKind {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let code = u8::deserialize(de)?;
        Ok(match code {
            1 => Self::X,
            2 => Self::Z,
            other => {
                return Err(serde::de::Error::custom(format!(
                    "unknown fiscal report kind code {other} (expected 1 or 2)"
                )));
            }
        })
    }
}

/// Optional single dimension a fiscal report is filtered by (spec §4.6.2).
///
/// The spec allows at most one filter per report and rejects multiple with code 169. Modelling it
/// as an enum makes "more than one filter" unrepresentable. Each variant maps to one wire key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum ReportFilter {
    /// Restrict to a single department (`deptId`).
    #[serde(rename = "deptId")]
    Department(u32),
    /// Restrict to a single cashier (`cashierId`).
    #[serde(rename = "cashierId")]
    Cashier(u32),
    /// Restrict to a transaction type — cash vs cashless (`transactionTypeId`).
    #[serde(rename = "transactionTypeId")]
    TransactionType(u32),
}

/// Op 9 request: print a fiscal report.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct FiscalReportRequest {
    /// X or Z report.
    #[serde(rename = "reportType")]
    #[cfg_attr(feature = "schema", schemars(with = "u8"))]
    pub kind: FiscalReportKind,
    /// Optional single filter (department, cashier, or transaction type). `None` reports over all.
    #[serde(flatten)]
    pub filter: Option<ReportFilter>,
    /// Start of the report time range (epoch-style integer, per spec example).
    #[serde(rename = "startDate")]
    pub start_date: i64,
    /// End of the report time range.
    #[serde(rename = "endDate")]
    pub end_date: i64,
}

impl Operation for FiscalReportRequest {
    const CODE: OperationCode = OperationCode::PrintFiscalReport;
    type Response = EmptyResponse;
}
