use crate::wire::OperationCode;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize, Serializer};

use super::{EmptyResponse, Operation};

// `#[serde(with = "dec")]` sends/receives a `Decimal` as a JSON number (not a string), matching the
// HDM wire format; `dec_opt` is the same for `Option<Decimal>`. Aliased short for per-field use.
use rust_decimal::serde::float as dec;
use rust_decimal::serde::float_option as dec_opt;

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
#[derive(Debug, Clone, Serialize)]
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
    #[serde(rename = "eMarks")]
    pub e_marks: Vec<String>,
    /// Items (required when `mode = Products`). Use an empty `Vec` for simple/prepayment modes.
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

/// Op 6 request: fetch the fiscal contents of a previously-issued receipt you intend to return.
///
/// This is a **read-only lookup**, not a refund. The original spec §4.5.6 is
/// "ՀԴՄ վերադարձվող կտրոնի ստացում" = *get the returnable receipt* (the English spec.md mistitles
/// it "print return receipt"). It returns the receipt's items, amounts, eMarks and sale type so a
/// caller can construct the actual return via [`PrintReturnReceiptRequest`] (op 10). It registers
/// nothing.
#[derive(Debug, Clone, Serialize)]
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

/// Op 6 response: the looked-up receipt's full fiscal contents.
///
/// **Unverified against hardware.** On the available N950 test stand op 6 always returns vendor
/// code 503 with an empty body — the firmware never exposes the server-side `Receipt_ID` this
/// lookup keys on (op 4 omits the `qr` field entirely), so the request can't be satisfied. This
/// struct is therefore modelled purely from spec §4.5.6, which is internally inconsistent: its
/// field table and the Code Block 7 example disagree (the example adds `type`, omits
/// `rseq`/`subType`/`refcrn`, and uses JSON numbers where the table says String). Fields follow the
/// Code Block 7 example — whose numeric types match what real firmware sends for op 4 — and are all
/// optional/lenient. The raw decrypted payload is logged at TRACE so the true shape can be
/// confirmed once a device ever returns one.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct ReturnableReceiptResponse {
    /// Receipt sequence number.
    #[serde(default)]
    pub rseq: Option<i64>,
    /// Cashier ID (`Գանձապահի ID`). The English spec.md mislabels this "Customer ID".
    #[serde(default)]
    pub cid: Option<i64>,
    /// Receipt registration/print time (ms since epoch, Greenwich).
    #[serde(default)]
    pub time: Option<i64>,
    /// Transaction type as the Code Block 7 example's `type` field (mirrors `sale_type`).
    #[serde(rename = "type", default)]
    pub kind: Option<i64>,
    /// Sale type: `0` sale, `2` return, `3` prepayment.
    #[serde(rename = "saleType", default)]
    pub sale_type: Option<i64>,
    /// Receipt sub-type: `1` simple, `2` itemised. (In the field table only; absent from the example.)
    #[serde(rename = "subType", default)]
    pub sub_type: Option<i64>,
    /// Department of a simple receipt.
    #[serde(default)]
    pub did: Option<i64>,
    /// Total amount.
    #[serde(default, with = "dec_opt")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub ta: Option<Decimal>,
    /// Cash paid.
    #[serde(default, with = "dec_opt")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub cash: Option<Decimal>,
    /// Card (cashless) paid.
    #[serde(default, with = "dec_opt")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub card: Option<Decimal>,
    /// Partial-payment amount.
    #[serde(default, with = "dec_opt")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub ppa: Option<Decimal>,
    /// Used prepayment amount.
    #[serde(default, with = "dec_opt")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub ppu: Option<Decimal>,
    /// Buyer TIN (8 digits) for a B2B receipt, or `null`.
    #[serde(rename = "pTin", default)]
    pub partner_tin: Option<String>,
    /// When this receipt is itself a return, the number of the receipt it returned (`ref`).
    #[serde(rename = "ref", default)]
    pub returned_receipt: Option<i64>,
    /// crn of the HDM that printed the returned receipt (set only for return-type receipts).
    #[serde(rename = "refcrn", default)]
    pub returned_crn: Option<String>,
    /// eMark codes of marked goods on the receipt.
    #[serde(rename = "eMarks", default)]
    pub e_marks: Vec<String>,
    /// Line items (empty for simple/prepayment receipts, where the spec sends `null`).
    #[serde(default)]
    pub totals: Vec<ReturnableReceiptItem>,
}

/// A single line item in a [`ReturnableReceiptResponse`] (`totals[]`). All fields optional/lenient
/// for the same reason as the parent — modelled from spec §4.5.6 Code Block 7, unverified.
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
    #[serde(default, with = "dec_opt")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub qty: Option<Decimal>,
    /// Unit price (`p`).
    #[serde(rename = "p", default, with = "dec_opt")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub price: Option<Decimal>,
    /// Unit of measure (`mu`).
    #[serde(rename = "mu", default)]
    pub unit: Option<String>,
    /// Row sequence number (`rpid`) — the handle used for per-item partial returns in op 10.
    #[serde(default)]
    pub rpid: Option<i64>,
    /// Primary discount (`dsc`).
    #[serde(rename = "dsc", default, with = "dec_opt")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub discount: Option<Decimal>,
    /// Proportional secondary discount (`adsc`).
    #[serde(rename = "adsc", default, with = "dec_opt")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub additional_discount: Option<Decimal>,
    /// Discount type (`dsct`).
    #[serde(rename = "dsct", default)]
    pub discount_kind: Option<i64>,
    /// Department (`did`).
    #[serde(default)]
    pub did: Option<i64>,
    /// Department VAT amount (`dt`).
    #[serde(rename = "dt", default, with = "dec_opt")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub vat_amount: Option<Decimal>,
    /// Department tax regime (`dtm`).
    #[serde(rename = "dtm", default)]
    pub tax_regime: Option<i64>,
    /// Line total excluding VAT (`t`).
    #[serde(rename = "t", default, with = "dec_opt")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub total_without_vat: Option<Decimal>,
    /// Line total including VAT (`tt`).
    #[serde(rename = "tt", default, with = "dec_opt")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub total_with_vat: Option<Decimal>,
}

/// Op 10 request: print a return/refund receipt — the operation that actually registers a return.
///
/// With no amounts/items it returns the whole receipt; set the `*_for_return` amounts and/or
/// `return_item_list` for a partial return. The original spec §4.5.7 is
/// "ՀԴՄ վերադարձի կտրոնի տպում" = *print return receipt* (the English spec.md mistitled it "get
/// receipt info"). The read-only lookup of the receipt being returned is op 6,
/// [`GetReturnableReceiptRequest`].
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PrintReturnReceiptRequest {
    /// HDM registration number of the device that printed the receipt.
    pub crn: String,
    /// Number of the receipt to be returned.
    #[serde(rename = "returnTicketId")]
    pub return_ticket_id: u64,
    /// Cash amount to return (only set if returning partial payments).
    #[serde(
        rename = "cashAmountForReturn",
        with = "dec_opt",
        skip_serializing_if = "Option::is_none",
        default
    )]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub cash_amount_for_return: Option<Decimal>,
    /// Card amount to return (only set if returning partial payments).
    #[serde(
        rename = "cardAmountForReturn",
        with = "dec_opt",
        skip_serializing_if = "Option::is_none",
        default
    )]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub card_amount_for_return: Option<Decimal>,
    /// Prepayment amount to return (only set if returning partial payments).
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
    #[serde(rename = "eMarks", skip_serializing_if = "Vec::is_empty")]
    pub e_marks: Vec<String>,
    /// Per-item return list (only set if returning specific items partially).
    #[serde(rename = "returnItemList", skip_serializing_if = "Vec::is_empty")]
    pub return_item_list: Vec<ReturnItem>,
}

impl Operation for PrintReturnReceiptRequest {
    const CODE: OperationCode = OperationCode::PrintReturnReceipt;
    type Response = ReturnReceiptResponse;
}

/// Per-item entry in a partial-return request.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ReturnItem {
    /// Row sequence number of the item being returned.
    pub rpid: i64,
    /// Quantity of this row's items to return.
    #[serde(with = "dec")]
    #[cfg_attr(feature = "schema", schemars(with = "f64"))]
    pub quantity: Decimal,
}

/// Op 10 response: full receipt details + return-specific timestamps.
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
