//! Render a fiscal receipt as human-friendly text from the wire request/response pair.
//!
//! The HDM device prints the legally-binding fiscal receipt on its own printer (working mode P)
//! and returns only structured identifiers; the body it printed is not otherwise recoverable. This
//! module reconstructs a **faithful summary** of that receipt from what we sent
//! ([`PrintReceiptRequest`] — items, departments, the cash/card split) and what came back
//! ([`ReceiptResponse`] — the fiscal number, registration number, totals). It is for archival
//! (store the text beside the raw JSON), operator/CLI display, and — on a working-mode-R device
//! that does *not* self-print — as the source a caller rasterises onto its own printer.
//!
//! It is deliberately **not** a pixel-faithful clone of the government layout the device prints:
//! the per-line VAT extraction and the department taxation captions depend on data outside the
//! request/response pair (and on firmware), so they are omitted. The device's own printout remains
//! the legal document; this is a record of it, not a replacement.
//!
//! The output is a width- and locale-agnostic [`ReceiptLayout`] of semantic [`ReceiptLine`]s. Time
//! zone and paper width are presentation concerns owned by the caller: [`ReceiptLayout::to_plain_text`]
//! renders a monospace block at a chosen column width, and a richer consumer can map the lines onto
//! its own printer primitives.

use core::fmt;

use rust_decimal::Decimal;

use crate::operations::{PrintMode, PrintReceiptRequest, ReceiptResponse};

/// Receipt labels, exactly as the Armenian fiscal device prints them.
mod label {
    /// Taxpayer registration number (ՀՎՀՀ).
    pub(super) const TIN: &str = "ՀՎՀՀ";
    /// HDM registration number (Գրանցման համար).
    pub(super) const CRN: &str = "Գ/Հ";
    /// HDM hardware serial number (Արտադրական համար).
    pub(super) const SERIAL: &str = "ԱՀ";
    /// Receipt sequence number (Կտրոնի համար).
    pub(super) const RSEQ: &str = "ԿՀ";
    /// Department caption for a simple (lump-sum) receipt.
    pub(super) const DEPARTMENT: &str = "Բաժին";
    /// Grand total (Ընդամենը).
    pub(super) const TOTAL: &str = "Ընդամենը";
    /// Cash tendered (Առձեռն).
    pub(super) const CASH: &str = "Առձեռն";
    /// Cashless tendered (Անկանխիկ).
    pub(super) const CARD: &str = "Անկանխիկ";
    /// Change due (Մանր).
    pub(super) const CHANGE: &str = "Մանր";
    /// Fiscal number footer (ՖԻՍԿԱԼ ՀԱՄԱՐ).
    pub(super) const FISCAL: &str = "ՖԻՍԿԱԼ ՀԱՄԱՐ";
    /// Receipt verification number (Ստուգիչ համար).
    pub(super) const VERIFY: &str = "Ստուգիչ";
    /// Lottery ticket number (Վիճակախաղ).
    pub(super) const LOTTERY: &str = "Վիճակախաղ";
}

/// One semantic line of a rendered receipt. Carries meaning, not geometry — width, alignment, and
/// emphasis are applied by a renderer ([`ReceiptLayout::to_plain_text`] or a consumer's own).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReceiptLine {
    /// Centred, emphasised heading (the taxpayer name, the fiscal-number footer).
    Title(String),
    /// Centred plain text (the taxpayer address).
    Centered(String),
    /// Left-aligned plain text.
    Text(String),
    /// A labelled identifier: label on the left, value on the right.
    Field {
        /// The field caption.
        label: String,
        /// The field value.
        value: String,
    },
    /// A goods line: name on the left, line total on the right.
    Item {
        /// Item display name (or the department caption for a simple receipt).
        name: String,
        /// Formatted line amount.
        amount: String,
    },
    /// A money row: caption on the left, amount on the right. `emphasize` marks the grand total.
    Amount {
        /// The amount caption.
        label: String,
        /// Formatted amount.
        value: String,
        /// Whether this row is the emphasised grand total.
        emphasize: bool,
    },
    /// A horizontal separator spanning the receipt width.
    Divider,
}

/// A complete receipt as an ordered list of semantic lines. Width- and locale-agnostic; render it
/// with [`ReceiptLayout::to_plain_text`] or consume the lines directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptLayout {
    /// The receipt's lines, top to bottom.
    pub lines: Vec<ReceiptLine>,
}

impl ReceiptLayout {
    /// Render the layout as a monospace text block `width` columns wide. Centring and right-aligning
    /// count Unicode scalar values, not bytes, so the Armenian script lays out correctly. Lines that
    /// do not fit fall back to a single space between the two halves rather than truncating.
    #[must_use]
    pub fn to_plain_text(&self, width: usize) -> String {
        let mut out = String::new();
        for line in &self.lines {
            match line {
                ReceiptLine::Title(text) | ReceiptLine::Centered(text) => {
                    push_line(&mut out, &centered(text, width));
                }
                ReceiptLine::Text(text) => push_line(&mut out, text),
                ReceiptLine::Field { label, value } => {
                    push_line(&mut out, &format!("{label}: {value}"));
                }
                ReceiptLine::Item { name, amount } => {
                    push_line(&mut out, &justified(name, amount, width));
                }
                ReceiptLine::Amount {
                    label,
                    value,
                    emphasize: _,
                } => {
                    push_line(&mut out, &justified(label, value, width));
                }
                ReceiptLine::Divider => push_line(&mut out, &"-".repeat(width)),
            }
        }
        out
    }
}

/// Renders the default 32-column monospace block — the common 80 mm thermal width for the Armenian
/// raster font. Use [`ReceiptLayout::to_plain_text`] for any other width.
impl fmt::Display for ReceiptLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_plain_text(DEFAULT_WIDTH))
    }
}

/// The default column width [`Display`](fmt::Display) renders at (80 mm thermal, Armenian raster).
pub const DEFAULT_WIDTH: usize = 32;

/// Build a faithful receipt summary from the request we sent and the response the device returned.
///
/// Empty response fields (older firmware, or a field the device leaves blank) are skipped rather
/// than printed empty. For a [`PrintMode::Products`] receipt the goods come from the request's
/// `items`; for a simple or prepayment receipt the body is a single department line carrying the
/// response total.
#[must_use]
pub fn format_receipt(request: &PrintReceiptRequest, response: &ReceiptResponse) -> ReceiptLayout {
    let mut lines = Vec::new();

    // Header — taxpayer identity and the device's fiscal registration.
    push_title(&mut lines, &response.taxpayer);
    push_centered(&mut lines, &response.address);
    push_field(&mut lines, label::TIN, &response.tin);
    push_field(&mut lines, label::CRN, &response.crn);
    push_field(&mut lines, label::SERIAL, &response.sn);
    lines.push(ReceiptLine::Field {
        label: label::RSEQ.to_owned(),
        value: response.rseq.to_string(),
    });
    lines.push(ReceiptLine::Divider);

    // Body — the goods, or a single department line for a lump-sum receipt.
    if request.mode == PrintMode::Products && !request.items.is_empty() {
        for item in &request.items {
            lines.push(ReceiptLine::Item {
                name: item.product_name.clone(),
                amount: money(item.qty * item.price),
            });
        }
    } else if let Some(dep) = request.dep {
        lines.push(ReceiptLine::Item {
            name: format!("{} {dep}", label::DEPARTMENT),
            amount: money(response.total),
        });
    }
    lines.push(ReceiptLine::Divider);

    // Totals — the grand total and how it was tendered.
    lines.push(ReceiptLine::Amount {
        label: label::TOTAL.to_owned(),
        value: money(response.total),
        emphasize: true,
    });
    push_amount(&mut lines, label::CASH, request.paid_amount);
    push_amount(&mut lines, label::CARD, request.paid_amount_card);
    push_amount(&mut lines, label::CHANGE, response.change);
    lines.push(ReceiptLine::Divider);

    // Footer — the legally-binding fiscal number and the optional verification / lottery / QR.
    if !response.fiscal.trim().is_empty() {
        lines.push(ReceiptLine::Title(format!(
            "{} {}",
            label::FISCAL,
            response.fiscal
        )));
    }
    if let Some(verify) = meaningful(response.verification_number.as_deref()) {
        push_field(&mut lines, label::VERIFY, verify);
    }
    if let Some(lottery) = meaningful(Some(&response.lottery)) {
        push_field(&mut lines, label::LOTTERY, lottery);
    }
    if let Some(qr) = response.qr.as_deref().filter(|q| !q.trim().is_empty()) {
        lines.push(ReceiptLine::Text(qr.to_owned()));
    }

    ReceiptLayout { lines }
}

/// Format a monetary [`Decimal`] with exactly two fractional digits, AMD-style (no symbol — the
/// device prints amounts bare).
fn money(value: Decimal) -> String {
    format!("{:.2}", value.round_dp(2))
}

/// A response string is "meaningful" when it is present, non-empty, and not the all-zero
/// placeholder some devices return for an unused verification/lottery slot.
fn meaningful(value: Option<&str>) -> Option<&str> {
    let trimmed = value.map(str::trim)?;
    if trimmed.is_empty() || trimmed.chars().all(|c| c == '0') {
        None
    } else {
        Some(trimmed)
    }
}

fn push_title(lines: &mut Vec<ReceiptLine>, text: &str) {
    if !text.trim().is_empty() {
        lines.push(ReceiptLine::Title(text.trim().to_owned()));
    }
}

fn push_centered(lines: &mut Vec<ReceiptLine>, text: &str) {
    if !text.trim().is_empty() {
        lines.push(ReceiptLine::Centered(text.trim().to_owned()));
    }
}

fn push_field(lines: &mut Vec<ReceiptLine>, label: &str, value: &str) {
    if !value.trim().is_empty() {
        lines.push(ReceiptLine::Field {
            label: label.to_owned(),
            value: value.trim().to_owned(),
        });
    }
}

fn push_amount(lines: &mut Vec<ReceiptLine>, label: &str, value: Decimal) {
    if value > Decimal::ZERO {
        lines.push(ReceiptLine::Amount {
            label: label.to_owned(),
            value: money(value),
            emphasize: false,
        });
    }
}

/// Append `text` and a newline to `out`.
fn push_line(out: &mut String, text: &str) {
    out.push_str(text);
    out.push('\n');
}

/// Centre `text` within `width` columns by Unicode scalar count; left-justify if it overflows.
fn centered(text: &str, width: usize) -> String {
    let len = text.chars().count();
    if len >= width {
        return text.to_owned();
    }
    let pad = (width - len) / 2;
    format!("{}{text}", " ".repeat(pad))
}

/// Place `left` and `right` at the two ends of a `width`-column line; if they do not fit, separate
/// them with a single space.
fn justified(left: &str, right: &str, width: usize) -> String {
    let used = left.chars().count() + right.chars().count();
    if used + 1 > width {
        return format!("{left} {right}");
    }
    format!("{left}{}{right}", " ".repeat(width - used))
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::format_receipt;
    use crate::operations::{PrintMode, PrintReceiptRequest, ReceiptItem, ReceiptResponse};

    /// A simple (lump-sum) cash request: 10 AMD in department 1 — the live test sale.
    fn simple_request() -> PrintReceiptRequest {
        PrintReceiptRequest {
            mode: PrintMode::Simple,
            paid_amount: Decimal::from(10),
            paid_amount_card: Decimal::ZERO,
            partial_amount: Decimal::ZERO,
            pre_payment_amount: Decimal::ZERO,
            dep: Some(1),
            partner_tin: None,
            use_ext_pos: false,
            payment_system: None,
            rrn: None,
            terminal_id: None,
            e_marks: Vec::new(),
            items: Vec::new(),
        }
    }

    /// The exact response the live Newland device returned for that sale (rseq 197).
    fn live_response() -> ReceiptResponse {
        ReceiptResponse {
            rseq: 197,
            crn: "51815332".to_owned(),
            sn: "NCBB02223374".to_owned(),
            tin: "00218811".to_owned(),
            taxpayer: "«ՔՅՈՒ ՏԵՐՄԻՆԱԼ»".to_owned(),
            address: "ԱՋԱՓՆՅԱԿ ԹԱՂԱՄԱՍ".to_owned(),
            time: 1_781_361_108_000,
            fiscal: "64048749".to_owned(),
            lottery: "00000000".to_owned(),
            prize: 0,
            total: Decimal::from(10),
            change: Decimal::ZERO,
            qr: None,
            emarks_count: Some("0".to_owned()),
            verification_number: Some("0000000".to_owned()),
        }
    }

    #[test]
    fn renders_the_live_simple_sale_with_device_labels() {
        let text = format_receipt(&simple_request(), &live_response()).to_plain_text(32);
        // The registration number prints under Գ/Հ (crn), the serial under ԱՀ (sn) — not swapped.
        assert!(text.contains("ՀՎՀՀ: 00218811"));
        assert!(text.contains("Գ/Հ: 51815332"));
        assert!(text.contains("ԱՀ: NCBB02223374"));
        assert!(text.contains("ԿՀ: 197"));
        // Simple-mode body: a single department line carrying the total.
        assert!(text.contains("Բաժին 1"));
        // The grand total and the cash tender; card / change rows are absent (both zero).
        assert!(text.contains("Ընդամենը"));
        assert!(text.contains("Առձեռն"));
        assert!(!text.contains("Անկանխիկ"));
        assert!(!text.contains("Մանր"));
        // The legally-binding fiscal number.
        assert!(text.contains("ՖԻՍԿԱԼ ՀԱՄԱՐ 64048749"));
        // All-zero verification + lottery placeholders are suppressed, not printed as zeros.
        assert!(!text.contains("Ստուգիչ"));
        assert!(!text.contains("Վիճակախաղ"));
    }

    #[test]
    fn renders_itemised_products_with_card_tender() {
        let request = PrintReceiptRequest {
            mode: PrintMode::Products,
            dep: None,
            paid_amount: Decimal::ZERO,
            paid_amount_card: Decimal::from(40),
            items: vec![ReceiptItem {
                dep: 1,
                qty: Decimal::from(2),
                price: Decimal::from(20),
                product_code: "56.0001".to_owned(),
                product_name: "Կապուչինո".to_owned(),
                adg_code: Some("2106".to_owned()),
                unit: "հատ".to_owned(),
                discount: None,
                discount_kind: None,
                additional_discount: None,
                additional_discount_kind: None,
            }],
            ..simple_request()
        };
        let mut response = live_response();
        response.total = Decimal::from(40);
        let text = format_receipt(&request, &response).to_plain_text(32);
        assert!(text.contains("Կապուչինո"));
        assert!(text.contains("40.00"));
        assert!(text.contains("Անկանխիկ"));
        // No simple-mode department line when we have itemised goods.
        assert!(!text.contains("Բաժին"));
    }

    #[test]
    fn surfaces_verification_lottery_and_qr_when_meaningful() {
        let mut response = live_response();
        response.verification_number = Some("128503".to_owned());
        response.lottery = "00000002".to_owned();
        response.qr = Some("TIN:00218811, CRN:51815332, FISCAL:64048749".to_owned());
        let text = format_receipt(&simple_request(), &response).to_plain_text(32);
        assert!(text.contains("Ստուգիչ: 128503"));
        assert!(text.contains("Վիճակախաղ: 00000002"));
        assert!(text.contains("CRN:51815332"));
    }
}
