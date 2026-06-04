use crate::wire::OperationCode;
use serde::{Deserialize, Serialize, Serializer};

use super::Operation;

/// Op 1 request. Encrypted with the password key.
///
/// `Debug` is hand-written to redact `password` so it never leaks through `{:?}`.
#[derive(Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ListOpsAndDepsRequest {
    /// HDM access password.
    pub password: String,
}

impl std::fmt::Debug for ListOpsAndDepsRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListOpsAndDepsRequest")
            .field("password", &"[redacted]")
            .finish()
    }
}

impl Operation for ListOpsAndDepsRequest {
    const CODE: OperationCode = OperationCode::ListOpsAndDeps;
    const USES_PASSWORD_KEY: bool = true;
    type Response = ListOpsAndDepsResponse;
}

/// Op 1 response: operators and departments registered on the HDM device.
///
/// The wire keys are the cryptic `c` / `d` from the spec example; the Rust fields are named for
/// what they hold.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct ListOpsAndDepsResponse {
    /// Operators registered on the device (wire key `c`).
    #[serde(rename = "c", default)]
    pub operators: Vec<OperatorInfo>,
    /// Departments registered on the device (wire key `d`).
    #[serde(rename = "d", default)]
    pub departments: Vec<DepartmentInfo>,
}

/// Single entry from the operator list.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct OperatorInfo {
    /// Operator's unique numeric ID, used as the cashier code in [`super::session::OperatorLoginRequest`].
    pub id: u32,
    /// Display name.
    #[serde(default)]
    pub name: String,
    /// Unique IDs of departments assigned to this operator.
    #[serde(default)]
    pub deps: Vec<u32>,
}

/// Single entry from the department list.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct DepartmentInfo {
    /// Department's unique numeric ID.
    pub id: u32,
    /// Display name.
    #[serde(default)]
    pub name: String,
    /// Taxation kind. Numeric on the wire; mapped to [`TaxationKind`].
    #[serde(rename = "type", default = "default_unknown_taxation")]
    #[cfg_attr(feature = "schema", schemars(with = "u32"))]
    pub kind: TaxationKind,
}

const fn default_unknown_taxation() -> TaxationKind {
    TaxationKind::Unknown(0)
}

/// Department taxation kinds per spec §4.5.1 table. Codes 4-6 are documented but not currently
/// in use; they are preserved for completeness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TaxationKind {
    /// `1` — VAT-taxable.
    VatTaxable,
    /// `2` — Not VAT-taxable.
    NotVatTaxable,
    /// `3` — Turnover tax.
    TurnoverTax,
    /// `4` — Production licensee (currently not in use).
    ProductionLicensee,
    /// `5` — Patented (currently not in use).
    Patented,
    /// `6` — Family business (currently not in use).
    FamilyBusiness,
    /// `7` — Micro-business.
    MicroBusiness,
    /// Code outside the documented set, preserved verbatim for forward compatibility.
    Unknown(u32),
}

impl<'de> Deserialize<'de> for TaxationKind {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let code = u32::deserialize(de)?;
        Ok(match code {
            1 => Self::VatTaxable,
            2 => Self::NotVatTaxable,
            3 => Self::TurnoverTax,
            4 => Self::ProductionLicensee,
            5 => Self::Patented,
            6 => Self::FamilyBusiness,
            7 => Self::MicroBusiness,
            other => Self::Unknown(other),
        })
    }
}

impl Serialize for TaxationKind {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        let code = match *self {
            Self::VatTaxable => 1,
            Self::NotVatTaxable => 2,
            Self::TurnoverTax => 3,
            Self::ProductionLicensee => 4,
            Self::Patented => 5,
            Self::FamilyBusiness => 6,
            Self::MicroBusiness => 7,
            Self::Unknown(c) => c,
        };
        ser.serialize_u32(code)
    }
}
