use crate::wire::OperationCode;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize, Serializer};

use super::{EmptyResponse, Operation};

// `#[serde(with = "dec")]` sends/receives a `Decimal` as a JSON number (not a string), matching the
// HDM wire format; `dec_opt` is the same for `Option<Decimal>`. Aliased short for per-field use.
use rust_decimal::serde::float as dec;
use rust_decimal::serde::float_option as dec_opt;

/// Deserializers tolerant of an HDM firmware quirk: the op-10 (`GetReturnableReceipt`) response sends
/// numeric fields as JSON **strings** (`"40.00"`, `"3"`, `"232"`) rather than numbers — verified on a
/// live Newland N950 (fw 1.1.3, 2026-06-23). They accept a string *or* a number on the wire (an empty
/// string decodes to `None`) and serialize back as a number so downstream JSON stays well-typed. Only
/// the op-10 response structs use these; request types keep the strict `dec`/`dec_opt`.
mod lenient {
    use rust_decimal::Decimal;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::str::FromStr;

    /// `Option<i64>` field that also accepts a JSON string.
    pub mod opt_i64 {
        use super::{Deserialize, Deserializer, FromStr, Serialize, Serializer};

        // serde's `with` serialize signature is fixed as `&T`; `Option<&T>` would not compile here.
        #[allow(clippy::ref_option)]
        pub fn serialize<S: Serializer>(value: &Option<i64>, ser: S) -> Result<S::Ok, S::Error> {
            value.serialize(ser)
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Option<i64>, D::Error> {
            #[derive(Deserialize)]
            #[serde(untagged)]
            enum Raw {
                Int(i64),
                Str(String),
            }
            match Option::<Raw>::deserialize(de)? {
                None => Ok(None),
                Some(Raw::Int(n)) => Ok(Some(n)),
                Some(Raw::Str(s)) => {
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        Ok(None)
                    } else {
                        i64::from_str(trimmed)
                            .map(Some)
                            .map_err(serde::de::Error::custom)
                    }
                }
            }
        }
    }

    /// `Option<Decimal>` field that also accepts a JSON string.
    pub mod opt_dec {
        use super::{Decimal, Deserialize, Deserializer, FromStr, Serializer};

        // serde's `with` serialize signature is fixed as `&T`; `Option<&T>` would not compile here.
        #[allow(clippy::ref_option)]
        pub fn serialize<S: Serializer>(
            value: &Option<Decimal>,
            ser: S,
        ) -> Result<S::Ok, S::Error> {
            match value {
                Some(_) => rust_decimal::serde::float_option::serialize(value, ser),
                None => ser.serialize_none(),
            }
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Option<Decimal>, D::Error> {
            #[derive(Deserialize)]
            #[serde(untagged)]
            enum Raw {
                Num(f64),
                Str(String),
            }
            match Option::<Raw>::deserialize(de)? {
                None => Ok(None),
                Some(Raw::Num(f)) => Decimal::try_from(f)
                    .map(Some)
                    .map_err(serde::de::Error::custom),
                Some(Raw::Str(s)) => {
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        Ok(None)
                    } else {
                        Decimal::from_str(trimmed)
                            .map(Some)
                            .map_err(serde::de::Error::custom)
                    }
                }
            }
        }
    }

    /// `Vec<T>` field where the firmware sends JSON `null` (not `[]`) for "no items" — simple and
    /// prepayment receipts send `"totals":null`. `#[serde(default)]` only covers an *absent* key, not
    /// an explicit null, so without this a simple receipt fails to decode ("expected a sequence").
    pub mod vec_or_null {
        use serde::{Deserialize, Deserializer};

        pub fn deserialize<'de, D, T>(de: D) -> Result<Vec<T>, D::Error>
        where
            D: Deserializer<'de>,
            T: Deserialize<'de>,
        {
            Ok(Option::<Vec<T>>::deserialize(de)?.unwrap_or_default())
        }
    }
}

/// Receipt print mode (spec §4.5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PrintMode {
    /// `1` — Simple lump-sum receipt. The `items` array is not used; only `dep` matters.
    Simple = 1,
    /// `2` — Itemised receipt with one or more goods.
    Products = 2,
    /// `3` — Prepayment receipt; the `items` array must be empty.
    Prepayment = 3,
}

impl Serialize for PrintMode {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for PrintMode {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let code = u8::deserialize(de)?;
        Ok(match code {
            1 => Self::Simple,
            2 => Self::Products,
            3 => Self::Prepayment,
            other => {
                return Err(serde::de::Error::custom(format!(
                    "unknown print mode code {other} (expected 1, 2, or 3)"
                )));
            }
        })
    }
}

/// Discount semantics per spec §4.5.4. The integer values are the wire encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscountKind {
    /// `1` — Percentage discount. `total_price * (discount/100)`.
    Percent,
    /// `2` — Price reduction per unit. `(price - discount) * quantity`.
    UnitPriceReduction,
    /// `4` — Total reduction over the line. `(price * quantity) - discount`.
    LineTotalReduction,
    /// `8` — Additional percentage discount on monetary-priced items.
    AdditionalPercent,
    /// `16` — Additional monetary discount.
    AdditionalMonetary,
}

impl DiscountKind {
    /// Numeric wire value.
    #[must_use]
    pub const fn code(self) -> u32 {
        match self {
            Self::Percent => 1,
            Self::UnitPriceReduction => 2,
            Self::LineTotalReduction => 4,
            Self::AdditionalPercent => 8,
            Self::AdditionalMonetary => 16,
        }
    }
}

impl Serialize for DiscountKind {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_u32(self.code())
    }
}

impl<'de> Deserialize<'de> for DiscountKind {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let code = u32::deserialize(de)?;
        Ok(match code {
            1 => Self::Percent,
            2 => Self::UnitPriceReduction,
            4 => Self::LineTotalReduction,
            8 => Self::AdditionalPercent,
            16 => Self::AdditionalMonetary,
            other => {
                return Err(serde::de::Error::custom(format!(
                    "unknown discount kind code {other} (expected 1, 2, 4, 8, or 16)"
                )));
            }
        })
    }
}

/// Op 4 request: print a fiscal receipt. Encrypted with the session key.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PrintReceiptRequest {
    /// Mode (simple / products / prepayment).
    #[cfg_attr(feature = "schema", schemars(with = "u8"))]
    pub mode: PrintMode,
    /// Cash portion of the payment.
    #[serde(rename = "paidAmount", with = "dec")]
    #[cfg_attr(feature = "schema", schemars(with = "f64"))]
    pub paid_amount: Decimal,
    /// Card (cashless) portion of the payment.
    #[serde(rename = "paidAmountCard", with = "dec")]
    #[cfg_attr(feature = "schema", schemars(with = "f64"))]
    pub paid_amount_card: Decimal,
    /// Partial-payment portion.
    #[serde(rename = "partialAmount", with = "dec")]
    #[cfg_attr(feature = "schema", schemars(with = "f64"))]
    pub partial_amount: Decimal,
    /// Prepayment-used portion.
    #[serde(rename = "prePaymentAmount", with = "dec")]
    #[cfg_attr(feature = "schema", schemars(with = "f64"))]
    pub pre_payment_amount: Decimal,
    /// Department for simple/prepayment receipts (when `items` is empty).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dep: Option<u32>,
    /// Partner TIN (8 digits) for B2B receipts. Sent as `null` if absent — the spec requires
    /// this field to appear even when null.
    #[serde(rename = "partnerTin")]
    pub partner_tin: Option<String>,
    /// If `true`, supply `rrn` + `terminal_id` from an external POS terminal. If `false`, the
    /// HDM uses its own configured payment system (set `payment_system`).
    #[serde(rename = "useExtPOS")]
    pub use_ext_pos: bool,
    /// Payment system code from spec §4.8 (1 = card, 10-18 = various Armenian wallets). Used
    /// only when `use_ext_pos = false` and the HDM has multiple payment systems configured.
    #[serde(rename = "PaymentSystem", skip_serializing_if = "Option::is_none")]
    pub payment_system: Option<u32>,
    /// Acquirer transaction RRN (12 chars). Used when `use_ext_pos = true`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rrn: Option<String>,
    /// Payment terminal unique ID (8 chars). Used when `use_ext_pos = true`.
    #[serde(rename = "terminalId", skip_serializing_if = "Option::is_none")]
    pub terminal_id: Option<String>,
    /// eMark traceability codes for marked goods (29-110 chars each, ASCII printable, escaping
    /// rules in spec §4.5.4). Used only in product-mode receipts.
    #[serde(rename = "eMarks", default)]
    pub e_marks: Vec<String>,
    /// Items (required when `mode = Products`). Use an empty `Vec` for simple/prepayment modes.
    #[serde(default)]
    pub items: Vec<ReceiptItem>,
}

impl Operation for PrintReceiptRequest {
    const CODE: OperationCode = OperationCode::PrintReceipt;
    type Response = ReceiptResponse;
}

/// A single item in a printed receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ReceiptItem {
    /// Department this item belongs to.
    pub dep: u32,
    /// Quantity, max 3 decimal places.
    #[serde(with = "dec")]
    #[cfg_attr(feature = "schema", schemars(with = "f64"))]
    pub qty: Decimal,
    /// Unit price, max 2 decimal places.
    #[serde(with = "dec")]
    #[cfg_attr(feature = "schema", schemars(with = "f64"))]
    pub price: Decimal,
    /// Vendor SKU (≤50 chars, must be non-empty).
    #[serde(rename = "productCode")]
    pub product_code: String,
    /// Display name (≤50 chars, must be non-empty).
    #[serde(rename = "productName")]
    pub product_name: String,
    /// ATG/ADG tax classification code. Lookup at `taxservice.am`. Mandatory unless waived by
    /// item-level discount logic per spec §4.5.4.
    #[serde(rename = "adgCode", skip_serializing_if = "Option::is_none")]
    pub adg_code: Option<String>,
    /// Unit of measure (≤50 chars, must be non-empty).
    pub unit: String,
    /// Primary discount amount (skip when no discount applies).
    #[serde(with = "dec_opt", skip_serializing_if = "Option::is_none", default)]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub discount: Option<Decimal>,
    /// Primary discount kind (skip when no discount applies).
    #[serde(rename = "discountType", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<u32>"))]
    pub discount_kind: Option<DiscountKind>,
    /// Stacked secondary discount amount (skip when no discount applies).
    #[serde(
        rename = "additionalDiscount",
        with = "dec_opt",
        skip_serializing_if = "Option::is_none",
        default
    )]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub additional_discount: Option<Decimal>,
    /// Stacked secondary discount kind (skip when no discount applies).
    #[serde(
        rename = "additionalDiscountType",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "schema", schemars(with = "Option<u32>"))]
    pub additional_discount_kind: Option<DiscountKind>,
}

/// Op 4 response: fiscal data for a successfully-printed receipt.
///
/// Three fields (`qr`, `verification_number`, `emarks_count`) were added in spec revisions past
/// v0.5; they are marked `Option` with `serde(default)` so older HDM firmware still deserialises
/// without errors.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct ReceiptResponse {
    /// HDM-assigned receipt sequence number.
    pub rseq: i64,
    /// HDM registration number (unique per device).
    #[serde(default)]
    pub crn: String,
    /// HDM hardware serial number.
    #[serde(default)]
    pub sn: String,
    /// Taxpayer TIN.
    #[serde(default)]
    pub tin: String,
    /// Taxpayer legal name.
    #[serde(default)]
    pub taxpayer: String,
    /// Taxpayer registered address.
    #[serde(default)]
    pub address: String,
    /// Receipt timestamp (milliseconds since Unix epoch, Greenwich time).
    #[serde(default)]
    pub time: i64,
    /// Fiscal receipt number — the legally-binding identifier.
    #[serde(default)]
    pub fiscal: String,
    /// Lottery ticket number associated with the receipt.
    #[serde(default)]
    pub lottery: String,
    /// `0` = no prize, `1` = prize won. (Currently no longer used by the lottery system.)
    #[serde(default)]
    pub prize: u32,
    /// Total amount on the receipt.
    #[serde(default, with = "dec")]
    #[cfg_attr(feature = "schema", schemars(with = "f64"))]
    pub total: Decimal,
    /// Change due to the customer.
    #[serde(default, with = "dec")]
    #[cfg_attr(feature = "schema", schemars(with = "f64"))]
    pub change: Decimal,
    /// QR-code payload to render on the printed receipt. Added post-v0.5.
    #[serde(default)]
    pub qr: Option<String>,
    /// Number of eMark codes processed. Added post-v0.5. Documented as `string` in spec.
    #[serde(rename = "emarksCount", default)]
    pub emarks_count: Option<String>,
    /// Short alphanumeric verification number (≤13 chars) printed on the receipt. Added post-v0.5.
    #[serde(rename = "verificationNumber", default)]
    pub verification_number: Option<String>,
}

/// Op 5 request: reprint a copy of the operator's most recent receipt.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PrintLastReceiptRequest {}

impl Operation for PrintLastReceiptRequest {
    const CODE: OperationCode = OperationCode::PrintLastReceipt;
    type Response = EmptyResponse;
}

/// Op 10 request: fetch the fiscal contents of a previously-issued receipt you intend to return.
///
/// This is a **read-only lookup**, not a refund. The original spec §4.5.6 is
/// "ՀԴՄ վերադարձվող կտրոնի ստացում" = *get the returnable receipt*. The spec describes it in section
/// §4.5.6, but its wire operation code is **10** per the operation-codes table (§4.4.1) — the section
/// number is not the operation code. It returns the receipt's items, amounts, eMarks and sale type
/// so a caller can construct the actual return via [`PrintReturnReceiptRequest`] (op 6). It registers
/// nothing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct GetReturnableReceiptRequest {
    /// Number of the receipt to look up.
    #[serde(rename = "receiptId")]
    pub receipt_id: String,
    /// HDM registration number of the device that printed the original receipt.
    pub crn: String,
}

impl Operation for GetReturnableReceiptRequest {
    const CODE: OperationCode = OperationCode::GetReturnableReceipt;
    type Response = ReturnableReceiptResponse;
}

/// Op 10 response: the looked-up receipt's full fiscal contents.
///
/// **Verified against a live Newland N950 (fw 1.1.3, 2026-06-23).** The lookup keys on the original
/// sale's `rseq` (the op-4 `rseq`, passed as `receiptId`); the fiscal number does not resolve. A
/// valid lookup returns code 200 with the receipt body.
///
/// **Firmware quirk handled.** The firmware sends almost every numeric field as a JSON **string**,
/// not a number. An observed body:
/// ```json
/// {"card":"40.00","cash":"0.00","cid":"3","eMarks":[],"pTin":"","ppa":"0.00","ppu":"0.00",
///  "ref":"232","refcrn":"51815332","rseq":232,"saleType":"0","subType":"2","ta":"40.00",
///  "time":"4109851456","totals":[{"adg":"56.10","p":"20.00","qty":"1.000","rpid":"0",
///  "t":"16.67","tt":"20.00",...}]}
/// ```
/// Only `rseq` is a JSON number; `cid`/`saleType`/`subType`/`time`/`rpid`/all amounts are strings.
/// The integer/decimal fields use the `lenient` string-or-number deserializers so a real 200 body
/// decodes (it used to fail on `cid`). Callers use op 10 as a returnability **pre-check** before a
/// refund: a 200 here means the receipt can be returned; server code 185/174/155/156 means it is not
/// yet returnable (post-sale sync pending). The raw decrypted payload is logged at TRACE.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct ReturnableReceiptResponse {
    /// Receipt sequence number. The PDF field table calls this the return-receipt sequence number,
    /// but Code Block 7 omits it.
    #[serde(default, with = "lenient::opt_i64")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<i64>"))]
    pub rseq: Option<i64>,
    /// Cashier ID (`Գանձապահի ID`).
    #[serde(default, with = "lenient::opt_i64")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<i64>"))]
    pub cid: Option<i64>,
    /// Receipt registration/print time (ms since epoch, Greenwich).
    #[serde(default, with = "lenient::opt_i64")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<i64>"))]
    pub time: Option<i64>,
    /// Transaction type as the Code Block 7 example's `type` field (mirrors `sale_type`).
    #[serde(rename = "type", default, with = "lenient::opt_i64")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<i64>"))]
    pub kind: Option<i64>,
    /// Sale type: `0` sale, `2` return, `3` prepayment.
    #[serde(rename = "saleType", default, with = "lenient::opt_i64")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<i64>"))]
    pub sale_type: Option<i64>,
    /// Receipt sub-type: `1` simple, `2` itemised. (In the field table only; absent from the example.)
    #[serde(rename = "subType", default, with = "lenient::opt_i64")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<i64>"))]
    pub sub_type: Option<i64>,
    /// Department of a simple receipt.
    #[serde(default, with = "lenient::opt_i64")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<i64>"))]
    pub did: Option<i64>,
    /// Total amount.
    #[serde(default, with = "lenient::opt_dec")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub ta: Option<Decimal>,
    /// Cash paid.
    #[serde(default, with = "lenient::opt_dec")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub cash: Option<Decimal>,
    /// Card (cashless) paid.
    #[serde(default, with = "lenient::opt_dec")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub card: Option<Decimal>,
    /// Partial-payment amount.
    #[serde(default, with = "lenient::opt_dec")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub ppa: Option<Decimal>,
    /// Used prepayment amount.
    #[serde(default, with = "lenient::opt_dec")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub ppu: Option<Decimal>,
    /// Buyer TIN (8 digits) for a B2B receipt, or `null`.
    #[serde(rename = "pTin", default)]
    pub partner_tin: Option<String>,
    /// When this receipt is itself a return, the number of the receipt it returned (`ref`).
    #[serde(rename = "ref", default, with = "lenient::opt_i64")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<i64>"))]
    pub returned_receipt: Option<i64>,
    /// crn of the HDM that printed the returned receipt (set only for return-type receipts).
    #[serde(rename = "refcrn", default)]
    pub returned_crn: Option<String>,
    /// eMark codes of marked goods on the receipt.
    #[serde(
        rename = "eMarks",
        default,
        deserialize_with = "lenient::vec_or_null::deserialize"
    )]
    pub e_marks: Vec<String>,
    /// Line items (empty for simple/prepayment receipts, where the spec sends `null`).
    #[serde(default, deserialize_with = "lenient::vec_or_null::deserialize")]
    pub totals: Vec<ReturnableReceiptItem>,
}

/// A single line item in a [`ReturnableReceiptResponse`] (`totals[]`). Numeric fields use the same
/// `lenient` string-or-number deserializers as the parent — the firmware sends them as strings.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct ReturnableReceiptItem {
    /// Product code (`gc`).
    #[serde(rename = "gc", default)]
    pub product_code: Option<String>,
    /// Product name (`gn`).
    #[serde(rename = "gn", default)]
    pub product_name: Option<String>,
    /// Quantity (`qty`).
    #[serde(default, with = "lenient::opt_dec")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub qty: Option<Decimal>,
    /// Unit price (`p`).
    #[serde(rename = "p", default, with = "lenient::opt_dec")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub price: Option<Decimal>,
    /// Unit of measure (`mu`).
    #[serde(rename = "mu", default)]
    pub unit: Option<String>,
    /// Row sequence number (`rpid`) — the handle used for per-item partial returns in op 6.
    #[serde(default, with = "lenient::opt_i64")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<i64>"))]
    pub rpid: Option<i64>,
    /// Primary discount (`dsc`).
    #[serde(rename = "dsc", default, with = "lenient::opt_dec")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub discount: Option<Decimal>,
    /// Proportional secondary discount (`adsc`).
    #[serde(rename = "adsc", default, with = "lenient::opt_dec")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub additional_discount: Option<Decimal>,
    /// Discount type (`dsct`).
    #[serde(rename = "dsct", default, with = "lenient::opt_i64")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<i64>"))]
    pub discount_kind: Option<i64>,
    /// Department (`did`).
    #[serde(default, with = "lenient::opt_i64")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<i64>"))]
    pub did: Option<i64>,
    /// Department VAT amount (`dt`).
    #[serde(rename = "dt", default, with = "lenient::opt_dec")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub vat_amount: Option<Decimal>,
    /// Department tax regime (`dtm`).
    #[serde(rename = "dtm", default, with = "lenient::opt_i64")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<i64>"))]
    pub tax_regime: Option<i64>,
    /// Line total excluding VAT (`t`).
    #[serde(rename = "t", default, with = "lenient::opt_dec")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub total_without_vat: Option<Decimal>,
    /// Line total including VAT (`tt`).
    #[serde(rename = "tt", default, with = "lenient::opt_dec")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub total_with_vat: Option<Decimal>,
}

/// Op 6 request: print a return/refund receipt — the operation that actually registers a return.
///
/// With no amounts/items it returns the whole receipt; set the `*_for_return` amounts and/or
/// `return_item_list` for a partial return. The original spec §4.5.7 is
/// "ՀԴՄ վերադարձի կտրոնի տպում" = *print return receipt*. The spec describes it in section §4.5.7,
/// but its wire operation code is **6** per the operation-codes table (§4.4.1). The read-only lookup
/// of the receipt being returned is op 10, [`GetReturnableReceiptRequest`].
///
/// **Verified against a live Newland N950 (fw 1.1.3, 2026-06-23).** A full return (only `crn` +
/// `return_ticket_id`) registers and returns code 200. Empirical, overriding the spec where it
/// disagrees: `return_ticket_id` is the original sale's `rseq` (not the fiscal number); the device
/// accepts it as either a JSON number or a string (spec §4.5.7 types it "Integer" but its own
/// example sends `"205"`), so this crate's `u64` is fine. `rrn`/`terminal_id` are optional in
/// practice — returns succeeded with them absent, empty, over-length, and malformed. NOTE: vendor
/// code **174** ("return receipt not found") is **transient**, not a permanent identifier error — it
/// fires when the HDM is in a busy/modal state (on-device payment picker, or error 190
/// "payment not available"); the same receipt returns fine moments later. Callers should retry 174
/// on a fresh session, but only because a 174 means nothing was registered — never blind-retry an
/// unknown-outcome (transport) failure on this write.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PrintReturnReceiptRequest {
    /// HDM registration number of the device that printed the receipt.
    pub crn: String,
    /// Number of the receipt to be returned (the original sale's `rseq`).
    #[serde(rename = "returnTicketId")]
    pub return_ticket_id: u64,
    /// Cash amount to return (only set for a partial return).
    #[serde(
        rename = "cashAmountForReturn",
        with = "dec_opt",
        skip_serializing_if = "Option::is_none",
        default
    )]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub cash_amount_for_return: Option<Decimal>,
    /// Card amount to return (only set for a partial return).
    #[serde(
        rename = "cardAmountForReturn",
        with = "dec_opt",
        skip_serializing_if = "Option::is_none",
        default
    )]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub card_amount_for_return: Option<Decimal>,
    /// Prepayment amount to return (only set for a partial return).
    #[serde(
        rename = "prePaymentAmountForReturn",
        with = "dec_opt",
        skip_serializing_if = "Option::is_none",
        default
    )]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub pre_payment_amount_for_return: Option<Decimal>,
    /// Acquirer RRN (12 chars).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rrn: Option<String>,
    /// Payment terminal ID (8 chars).
    #[serde(rename = "terminalId", skip_serializing_if = "Option::is_none")]
    pub terminal_id: Option<String>,
    /// eMark codes for marked goods.
    #[serde(rename = "eMarks", skip_serializing_if = "Vec::is_empty", default)]
    pub e_marks: Vec<String>,
    /// Per-item return list (only set when partially returning an itemised receipt).
    #[serde(
        rename = "returnItemList",
        skip_serializing_if = "Vec::is_empty",
        default
    )]
    pub return_item_list: Vec<ReturnItem>,
}

impl Operation for PrintReturnReceiptRequest {
    const CODE: OperationCode = OperationCode::PrintReturnReceipt;
    type Response = ReturnReceiptResponse;
}

/// Per-item entry in a partial-return request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ReturnItem {
    /// Row sequence number of the item being returned.
    pub rpid: i64,
    /// Quantity of this row's items to return.
    #[serde(with = "dec")]
    #[cfg_attr(feature = "schema", schemars(with = "f64"))]
    pub quantity: Decimal,
}

/// Op 6 response: full receipt details + return-specific timestamps.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct ReturnReceiptResponse {
    /// Receipt sequence number.
    pub rseq: i64,
    /// HDM registration number.
    #[serde(default)]
    pub crn: String,
    /// HDM hardware serial number.
    #[serde(default)]
    pub sn: String,
    /// Taxpayer TIN.
    #[serde(default)]
    pub tin: String,
    /// Taxpayer name.
    #[serde(default)]
    pub taxpayer: String,
    /// Taxpayer address.
    #[serde(default)]
    pub address: String,
    /// Original receipt timestamp (ms since epoch, Greenwich).
    #[serde(default)]
    pub time: i64,
    /// Return-receipt timestamp (ms since epoch, Greenwich).
    #[serde(default)]
    pub rtime: i64,
    /// Fiscal number.
    #[serde(default)]
    pub fiscal: String,
    /// Lottery number.
    #[serde(default)]
    pub lottery: String,
    /// Prize indicator (currently always 0).
    #[serde(default)]
    pub prize: u32,
    /// Total amount.
    #[serde(default, with = "dec")]
    #[cfg_attr(feature = "schema", schemars(with = "f64"))]
    pub total: Decimal,
    /// Change.
    #[serde(default, with = "dec")]
    #[cfg_attr(feature = "schema", schemars(with = "f64"))]
    pub change: Decimal,
    /// Number of registered marks.
    #[serde(rename = "emarksCount", default)]
    pub emarks_count: Option<String>,
    /// Short verification number printed on the receipt.
    #[serde(rename = "verificationNumber", default)]
    pub verification_number: Option<String>,
}

#[cfg(test)]
mod returnable_receipt_tests {
    use super::{ReturnableReceiptResponse, lenient};
    use rust_decimal::Decimal;
    use serde::Deserialize;
    use std::str::FromStr;

    /// The exact body a live Newland N950 (fw 1.1.3) returned for op-10 on receipt 232 — almost
    /// every numeric field is a JSON string. This is the regression: it used to fail decoding on the
    /// first string-typed integer (`cid`).
    const N950_BODY: &str = r#"{
        "card":"40.00","cash":"0.00","cid":"3","eMarks":[],"pTin":"","ppa":"0.00","ppu":"0.00",
        "ref":"232","refcrn":"51815332","rseq":232,"saleType":"0","subType":"2","ta":"40.00",
        "time":"4109851456",
        "totals":[{"adg":"56.10","p":"20.00","qty":"1.000","rpid":"0","t":"16.67","tt":"20.00",
                   "gc":"56.0001","gn":"Tea","mu":"hat","did":"3","dtm":"3"}]
    }"#;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn decodes_the_real_n950_string_typed_body() {
        let parsed: ReturnableReceiptResponse =
            serde_json::from_str(N950_BODY).expect("real N950 body must decode");

        assert_eq!(parsed.rseq, Some(232));
        assert_eq!(parsed.cid, Some(3));
        assert_eq!(parsed.time, Some(4_109_851_456));
        assert_eq!(parsed.sale_type, Some(0));
        assert_eq!(parsed.sub_type, Some(2));
        assert_eq!(parsed.returned_receipt, Some(232));
        assert_eq!(parsed.returned_crn.as_deref(), Some("51815332"));
        assert_eq!(parsed.ta, Some(dec("40.00")));
        assert_eq!(parsed.card, Some(dec("40.00")));
        assert_eq!(parsed.cash, Some(dec("0.00")));

        assert_eq!(parsed.totals.len(), 1);
        let item = &parsed.totals[0];
        assert_eq!(item.price, Some(dec("20.00")));
        assert_eq!(item.qty, Some(dec("1.000")));
        assert_eq!(item.rpid, Some(0));
        assert_eq!(item.total_without_vat, Some(dec("16.67")));
        assert_eq!(item.total_with_vat, Some(dec("20.00")));
        assert_eq!(item.did, Some(3));
        assert_eq!(item.tax_regime, Some(3));
        assert_eq!(item.product_name.as_deref(), Some("Tea"));
    }

    #[test]
    fn decodes_simple_receipt_with_null_totals() {
        // Simple/prepayment receipts send `"totals":null` (and may null `eMarks`), not `[]`.
        // `#[serde(default)]` alone fails on an explicit null with "expected a sequence".
        let body = r#"{"rseq":245,"cid":"3","saleType":"0","subType":"1","ta":"10.00",
                       "card":"10.00","cash":"0.00","eMarks":null,"totals":null}"#;
        let parsed: ReturnableReceiptResponse =
            serde_json::from_str(body).expect("null totals must decode to empty");
        assert_eq!(parsed.rseq, Some(245));
        assert_eq!(parsed.sub_type, Some(1));
        assert_eq!(parsed.ta, Some(dec("10.00")));
        assert!(parsed.totals.is_empty());
        assert!(parsed.e_marks.is_empty());
    }

    #[test]
    fn decodes_spec_compliant_numeric_body_too() {
        // The spec types these as numbers; a firmware that obeys the spec must also decode.
        let body = r#"{"rseq":7,"cid":3,"time":1700000000000,"saleType":0,"ta":40.0,
                       "totals":[{"p":20.0,"qty":1.0,"rpid":0,"tt":20.0}]}"#;
        let parsed: ReturnableReceiptResponse =
            serde_json::from_str(body).expect("numeric decodes");
        assert_eq!(parsed.rseq, Some(7));
        assert_eq!(parsed.cid, Some(3));
        assert_eq!(parsed.ta, Some(dec("40")));
        assert_eq!(parsed.totals[0].price, Some(dec("20")));
        assert_eq!(parsed.totals[0].rpid, Some(0));
    }

    #[test]
    fn empty_strings_and_absent_fields_decode_to_none() {
        // The firmware sends "" for an absent partner TIN and may omit fields entirely.
        let body = r#"{"rseq":9,"pTin":"","cid":"","ta":"","totals":[]}"#;
        let parsed: ReturnableReceiptResponse = serde_json::from_str(body).expect("empties decode");
        assert_eq!(parsed.rseq, Some(9));
        assert_eq!(parsed.cid, None, "empty numeric string is None, not 0");
        assert_eq!(parsed.ta, None);
        assert_eq!(parsed.partner_tin.as_deref(), Some(""));
        assert_eq!(parsed.time, None, "absent field defaults to None");
    }

    #[test]
    fn whitespace_padded_numeric_strings_are_trimmed() {
        let body = r#"{"rseq":1,"cid":" 12 ","ta":" 5.50 "}"#;
        let parsed: ReturnableReceiptResponse = serde_json::from_str(body).expect("padded decodes");
        assert_eq!(parsed.cid, Some(12));
        assert_eq!(parsed.ta, Some(dec("5.50")));
    }

    #[test]
    fn non_numeric_string_fails_loud() {
        // A genuinely malformed value must surface, not silently become None/0.
        let body = r#"{"rseq":1,"cid":"not-a-number"}"#;
        let err = serde_json::from_str::<ReturnableReceiptResponse>(body)
            .expect_err("garbage integer must error");
        assert!(err.to_string().contains("invalid") || err.to_string().contains("digit"));
    }

    #[test]
    fn serialize_round_trips_back_to_decodable_json() {
        let parsed: ReturnableReceiptResponse = serde_json::from_str(N950_BODY).unwrap();
        let json = serde_json::to_string(&parsed).expect("serialize");
        let reparsed: ReturnableReceiptResponse =
            serde_json::from_str(&json).expect("re-decode our own output");
        assert_eq!(reparsed.rseq, parsed.rseq);
        assert_eq!(reparsed.ta, parsed.ta);
        assert_eq!(reparsed.totals[0].price, parsed.totals[0].price);
    }

    #[derive(Deserialize)]
    struct OptI64 {
        #[serde(default, with = "lenient::opt_i64")]
        v: Option<i64>,
    }

    #[derive(Deserialize)]
    struct OptDec {
        #[serde(default, with = "lenient::opt_dec")]
        v: Option<Decimal>,
    }

    #[test]
    fn lenient_opt_i64_accepts_string_number_and_empty() {
        assert_eq!(
            serde_json::from_str::<OptI64>(r#"{"v":"42"}"#).unwrap().v,
            Some(42)
        );
        assert_eq!(
            serde_json::from_str::<OptI64>(r#"{"v":42}"#).unwrap().v,
            Some(42)
        );
        assert_eq!(
            serde_json::from_str::<OptI64>(r#"{"v":""}"#).unwrap().v,
            None
        );
        assert_eq!(
            serde_json::from_str::<OptI64>(r#"{"v":null}"#).unwrap().v,
            None
        );
        assert_eq!(serde_json::from_str::<OptI64>(r"{}").unwrap().v, None);
    }

    #[test]
    fn lenient_opt_dec_accepts_string_number_and_empty() {
        assert_eq!(
            serde_json::from_str::<OptDec>(r#"{"v":"1.25"}"#).unwrap().v,
            Some(dec("1.25"))
        );
        assert_eq!(
            serde_json::from_str::<OptDec>(r#"{"v":1.25}"#).unwrap().v,
            Some(dec("1.25"))
        );
        assert_eq!(
            serde_json::from_str::<OptDec>(r#"{"v":""}"#).unwrap().v,
            None
        );
        assert_eq!(
            serde_json::from_str::<OptDec>(r#"{"v":null}"#).unwrap().v,
            None
        );
    }
}
