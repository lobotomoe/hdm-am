//! Input validation and GUI-to-wire request construction.

use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use hdm_am::{
    CashInOutRequest, Decimal, DiscountKind, FiscalReportKind, FiscalReportRequest, PrintMode,
    PrintReceiptRequest, PrintReturnReceiptRequest, ReceiptItem, ReportFilter, ReturnItem,
    SetupHeaderFooterRequest, TextLine,
};
use serde::de::DeserializeOwned;

/// Parsed connection settings ready for TCP/client setup.
#[derive(Debug, Clone)]
pub struct ConnectionSettings {
    pub host: String,
    pub port: u16,
    pub password: String,
    pub cashier: u32,
    pub pin: String,
    pub timeout: Duration,
}

/// Raw operation inputs mirrored from the Slint UI.
#[derive(Clone)]
pub struct OperationInputs {
    pub receipt_mode: String,
    pub amount: String,
    pub card_amount: String,
    pub partial_amount: String,
    pub prepayment_amount: String,
    pub department: String,
    pub partner_tin: String,
    pub payment_system: String,
    pub use_ext_pos: bool,
    pub rrn: String,
    pub terminal_id: String,
    pub crn: String,
    pub receipt_id: String,
    pub ticket: String,
    pub emarks: String,
    pub json_path: String,
    pub logo_path: String,
    pub description: String,
    pub cash_in: bool,
    pub report_kind: String,
    pub report_filter_kind: String,
    pub report_filter_value: String,
    pub report_start: String,
    pub report_end: String,
    pub confirm_operation: bool,
}

#[derive(serde::Deserialize)]
struct HeaderFooterFile {
    #[serde(default)]
    headers: Vec<TextLine>,
    #[serde(default)]
    footers: Vec<TextLine>,
}

/// Raw connection inputs and auth requirements for one operation.
pub struct ConnectionInput<'a> {
    pub host: &'a str,
    pub port: &'a str,
    pub timeout_seconds: &'a str,
    pub password: &'a str,
    pub cashier: &'a str,
    pub pin: &'a str,
    pub needs_password: bool,
    pub needs_session: bool,
}

/// Validate connection fields and return strongly typed settings.
pub fn connection_settings(input: &ConnectionInput<'_>) -> Result<ConnectionSettings, String> {
    let host = required(input.host, "Host")?;
    validate_host(&host)?;
    let port = parse_port(input.port)?;
    let timeout = parse_timeout(input.timeout_seconds)?;

    let password = input.password.to_owned();
    if input.needs_password && password.is_empty() {
        return Err("Password is required for this operation.".to_owned());
    }

    let cashier = if input.needs_session {
        let cashier = parse_required_u32(input.cashier, "Cashier")?;
        if cashier == 0 {
            return Err("Cashier must be greater than zero.".to_owned());
        }
        cashier
    } else {
        0
    };

    let pin = input.pin.to_owned();
    if input.needs_session && pin.is_empty() {
        return Err("PIN is required for this operation.".to_owned());
    }

    Ok(ConnectionSettings {
        host,
        port,
        password,
        cashier,
        pin,
        timeout,
    })
}

/// Build op 4 request from GUI input.
pub fn build_receipt_request(inputs: &OperationInputs) -> Result<PrintReceiptRequest, String> {
    let mode = match inputs.receipt_mode.trim().to_ascii_lowercase().as_str() {
        "" | "simple" => PrintMode::Simple,
        "products" | "product" => PrintMode::Products,
        "prepayment" | "pre-payment" => PrintMode::Prepayment,
        other => {
            return Err(format!(
                "Receipt mode must be simple, products, or prepayment; got {other}."
            ));
        }
    };

    let items = match mode {
        PrintMode::Products => {
            let path = required(&inputs.json_path, "JSON path")?;
            let items: Vec<ReceiptItem> = read_json_file(Path::new(&path), "receipt items")?;
            validate_receipt_items(&items)?;
            items
        }
        PrintMode::Simple | PrintMode::Prepayment => Vec::new(),
    };

    let e_marks = split_emarks(&inputs.emarks)?;
    if !matches!(mode, PrintMode::Products) && !e_marks.is_empty() {
        return Err("eMarks are only valid for product receipts.".to_owned());
    }

    let dep = parse_optional_u32(&inputs.department, "Department")?;
    if matches!(mode, PrintMode::Simple | PrintMode::Prepayment) && dep.is_none() {
        return Err("Department is required for simple and prepayment receipts.".to_owned());
    }
    if let Some(dep) = dep {
        validate_positive_u32(dep, "Department")?;
    }

    let partner_tin = optional_string(&inputs.partner_tin);
    if let Some(tin) = &partner_tin {
        validate_digits_exact(tin, 8, "Partner TIN")?;
    }

    let (rrn, terminal_id) = if inputs.use_ext_pos {
        if optional_string(&inputs.payment_system).is_some() {
            return Err("Payment system cannot be used with External POS.".to_owned());
        }
        let rrn = required(&inputs.rrn, "RRN")?;
        validate_ascii_exact(&rrn, 12, "RRN")?;
        let terminal_id = required(&inputs.terminal_id, "Terminal ID")?;
        validate_ascii_exact(&terminal_id, 8, "Terminal ID")?;
        (Some(rrn), Some(terminal_id))
    } else {
        if optional_string(&inputs.rrn).is_some() || optional_string(&inputs.terminal_id).is_some()
        {
            return Err("RRN and Terminal ID require External POS.".to_owned());
        }
        (None, None)
    };

    let payment_system = parse_optional_u32(&inputs.payment_system, "Payment system")?;
    if let Some(code) = payment_system {
        validate_positive_u32(code, "Payment system")?;
    }

    let paid_amount = parse_decimal_or_zero(&inputs.amount, "Amount", 2)?;
    let paid_amount_card = parse_decimal_or_zero(&inputs.card_amount, "Card", 2)?;
    let partial_amount = parse_decimal_or_zero(&inputs.partial_amount, "Partial", 2)?;
    let pre_payment_amount = parse_decimal_or_zero(&inputs.prepayment_amount, "Prepayment", 2)?;
    if paid_amount + paid_amount_card + partial_amount + pre_payment_amount <= Decimal::ZERO {
        return Err("Receipt payment total must be greater than zero.".to_owned());
    }

    Ok(PrintReceiptRequest {
        mode,
        paid_amount,
        paid_amount_card,
        partial_amount,
        pre_payment_amount,
        dep,
        partner_tin,
        use_ext_pos: inputs.use_ext_pos,
        payment_system,
        rrn,
        terminal_id,
        e_marks,
        items,
    })
}

/// Build op 9 request from GUI input.
pub fn build_report_request(inputs: &OperationInputs) -> Result<FiscalReportRequest, String> {
    let kind = match inputs.report_kind.trim().to_ascii_lowercase().as_str() {
        "" | "x" => FiscalReportKind::X,
        "z" => FiscalReportKind::Z,
        other => return Err(format!("Report kind must be x or z; got {other}.")),
    };

    let filter = match inputs
        .report_filter_kind
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "none" => None,
        "dept" | "department" => Some(ReportFilter::Department(parse_required_positive_u32(
            &inputs.report_filter_value,
            "Filter value",
        )?)),
        "cashier" => Some(ReportFilter::Cashier(parse_required_positive_u32(
            &inputs.report_filter_value,
            "Filter value",
        )?)),
        "transaction" | "transaction-type" => Some(ReportFilter::TransactionType(
            parse_required_positive_u32(&inputs.report_filter_value, "Filter value")?,
        )),
        other => {
            return Err(format!(
                "Filter kind must be empty, dept, cashier, or transaction; got {other}."
            ));
        }
    };

    let start_date = parse_i64_or_zero(&inputs.report_start, "Start")?;
    let end_date = parse_i64_or_zero(&inputs.report_end, "End")?;
    if start_date < 0 || end_date < 0 {
        return Err("Report start and end must be zero or positive epoch values.".to_owned());
    }
    if start_date != 0 && end_date != 0 && end_date < start_date {
        return Err("Report end must be greater than or equal to start.".to_owned());
    }

    Ok(FiscalReportRequest {
        kind,
        filter,
        start_date,
        end_date,
    })
}

/// Build op 10 request from GUI input.
pub fn build_return_request(inputs: &OperationInputs) -> Result<PrintReturnReceiptRequest, String> {
    let return_item_list = match optional_string(&inputs.json_path) {
        Some(path) => {
            let items: Vec<ReturnItem> = read_json_file(Path::new(&path), "return items")?;
            validate_return_items(&items)?;
            items
        }
        None => Vec::new(),
    };

    let crn = required(&inputs.crn, "CRN")?;
    validate_digits_exact(&crn, 8, "CRN")?;

    let (rrn, terminal_id) = match (
        optional_string(&inputs.rrn),
        optional_string(&inputs.terminal_id),
    ) {
        (Some(rrn), Some(terminal_id)) => {
            validate_ascii_exact(&rrn, 12, "RRN")?;
            validate_ascii_exact(&terminal_id, 8, "Terminal ID")?;
            (Some(rrn), Some(terminal_id))
        }
        (None, None) => (None, None),
        (Some(_), None) => return Err("Terminal ID is required when RRN is set.".to_owned()),
        (None, Some(_)) => return Err("RRN is required when Terminal ID is set.".to_owned()),
    };

    Ok(PrintReturnReceiptRequest {
        crn,
        return_ticket_id: parse_required_u64(&inputs.ticket, "Ticket")?,
        cash_amount_for_return: parse_optional_decimal(&inputs.amount, "Amount", 2)?,
        card_amount_for_return: parse_optional_decimal(&inputs.card_amount, "Card", 2)?,
        pre_payment_amount_for_return: parse_optional_decimal(
            &inputs.prepayment_amount,
            "Prepayment",
            2,
        )?,
        rrn,
        terminal_id,
        e_marks: split_emarks(&inputs.emarks)?,
        return_item_list,
    })
}

/// Build op 11 request from GUI input.
pub fn build_cash_request(
    inputs: &OperationInputs,
    cashier: u32,
) -> Result<CashInOutRequest, String> {
    let amount = parse_required_decimal(&inputs.amount, "Amount", 2)?;
    if amount <= Decimal::ZERO {
        return Err("Amount must be greater than zero.".to_owned());
    }
    Ok(CashInOutRequest {
        amount,
        is_cash_in: inputs.cash_in,
        cashier_id: Some(cashier),
        description: inputs.description.clone(),
    })
}

/// Build op 7 request from a header/footer JSON file.
pub fn build_header_footer_request(
    inputs: &OperationInputs,
) -> Result<SetupHeaderFooterRequest, String> {
    let path = required(&inputs.json_path, "JSON path")?;
    let parsed: HeaderFooterFile = read_json_file(Path::new(&path), "header/footer JSON")?;
    validate_text_lines(&parsed.headers, "headers")?;
    validate_text_lines(&parsed.footers, "footers")?;
    Ok(SetupHeaderFooterRequest {
        headers: parsed.headers,
        footers: parsed.footers,
    })
}

/// Read and validate a logo BMP file, returning Base64-encoded bytes for op 8.
pub fn read_logo_base64(inputs: &OperationInputs) -> Result<String, String> {
    let path = required(&inputs.logo_path, "Logo path")?;
    let bytes = fs::read(&path).map_err(|err| format!("Reading logo file {path} failed: {err}"))?;
    validate_bmp_logo(&bytes)?;
    Ok(BASE64.encode(bytes))
}

/// Validate and return lookup arguments for op 6.
pub fn lookup_args(inputs: &OperationInputs) -> Result<(String, String), String> {
    let receipt_id = required(&inputs.receipt_id, "Receipt ID")?;
    let crn = required(&inputs.crn, "CRN")?;
    validate_digits_exact(&crn, 8, "CRN")?;
    Ok((receipt_id, crn))
}

/// Validate and return a single eMark for op 16.
pub fn single_emark(inputs: &OperationInputs) -> Result<String, String> {
    let value = required(&inputs.emarks, "eMarks")?;
    validate_emark(&value, "eMark")?;
    Ok(value)
}

fn validate_host(host: &str) -> Result<(), String> {
    if host.chars().any(char::is_whitespace) {
        return Err("Host must not contain whitespace.".to_owned());
    }
    Ok(())
}

fn parse_port(raw: &str) -> Result<u16, String> {
    let trimmed = raw.trim();
    trimmed
        .parse::<u16>()
        .map_err(|_| format!("Port must be an integer from 1 to {}.", u16::MAX))
        .and_then(|port| {
            if port == 0 {
                Err("Port must be greater than zero.".to_owned())
            } else {
                Ok(port)
            }
        })
}

fn parse_timeout(raw: &str) -> Result<Duration, String> {
    let seconds = raw
        .trim()
        .parse::<u64>()
        .map_err(|_| "Timeout must be a whole number of seconds.".to_owned())?;
    if seconds == 0 {
        return Err("Timeout must be greater than zero.".to_owned());
    }
    if seconds > 50 {
        return Err("Timeout must not exceed 50 seconds per the HDM protocol.".to_owned());
    }
    Ok(Duration::from_secs(seconds))
}

fn read_json_file<T>(path: &Path, label: &str) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let raw = fs::read_to_string(path).map_err(|err| {
        format!(
            "Reading {label} file {} failed: {err}",
            path.to_string_lossy()
        )
    })?;
    serde_json::from_str(&raw).map_err(|err| {
        format!(
            "Parsing {label} from {} failed: {err}",
            path.to_string_lossy()
        )
    })
}

fn required(value: &str, label: &str) -> Result<String, String> {
    optional_string(value).ok_or_else(|| format!("{label} is required."))
}

fn optional_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn parse_decimal_or_zero(raw: &str, label: &str, max_scale: u32) -> Result<Decimal, String> {
    if raw.trim().is_empty() {
        Ok(Decimal::ZERO)
    } else {
        parse_required_decimal(raw, label, max_scale)
    }
}

fn parse_required_decimal(raw: &str, label: &str, max_scale: u32) -> Result<Decimal, String> {
    let value =
        Decimal::from_str(raw.trim()).map_err(|_| format!("{label} must be a decimal number."))?;
    validate_decimal_scale(value, max_scale, label)?;
    Ok(value)
}

fn parse_optional_decimal(
    raw: &str,
    label: &str,
    max_scale: u32,
) -> Result<Option<Decimal>, String> {
    if raw.trim().is_empty() {
        Ok(None)
    } else {
        parse_required_decimal(raw, label, max_scale).map(Some)
    }
}

fn validate_decimal_scale(value: Decimal, max_scale: u32, label: &str) -> Result<(), String> {
    if value.normalize().scale() > max_scale {
        return Err(format!(
            "{label} may have at most {max_scale} decimal places."
        ));
    }
    Ok(())
}

fn parse_required_u32(raw: &str, label: &str) -> Result<u32, String> {
    raw.trim()
        .parse::<u32>()
        .map_err(|_| format!("{label} must be an unsigned integer."))
}

fn parse_required_positive_u32(raw: &str, label: &str) -> Result<u32, String> {
    let value = parse_required_u32(raw, label)?;
    validate_positive_u32(value, label)?;
    Ok(value)
}

fn parse_optional_u32(raw: &str, label: &str) -> Result<Option<u32>, String> {
    if raw.trim().is_empty() {
        Ok(None)
    } else {
        parse_required_u32(raw, label).map(Some)
    }
}

fn parse_required_u64(raw: &str, label: &str) -> Result<u64, String> {
    raw.trim()
        .parse::<u64>()
        .map_err(|_| format!("{label} must be an unsigned integer."))
}

fn parse_i64_or_zero(raw: &str, label: &str) -> Result<i64, String> {
    if raw.trim().is_empty() {
        Ok(0)
    } else {
        raw.trim()
            .parse::<i64>()
            .map_err(|_| format!("{label} must be an integer."))
    }
}

fn validate_positive_u32(value: u32, label: &str) -> Result<(), String> {
    if value == 0 {
        return Err(format!("{label} must be greater than zero."));
    }
    Ok(())
}

fn validate_digits_exact(value: &str, len: usize, label: &str) -> Result<(), String> {
    if value.chars().count() != len || !value.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(format!("{label} must be exactly {len} digits."));
    }
    Ok(())
}

fn validate_ascii_exact(value: &str, len: usize, label: &str) -> Result<(), String> {
    if value.chars().count() != len || !value.is_ascii() {
        return Err(format!("{label} must be exactly {len} ASCII characters."));
    }
    Ok(())
}

fn split_emarks(raw: &str) -> Result<Vec<String>, String> {
    raw.split([',', ';', '\n'])
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .enumerate()
        .map(|(idx, entry)| {
            validate_emark(entry, &format!("eMark {}", idx + 1))?;
            Ok(entry.to_owned())
        })
        .collect()
}

fn validate_emark(value: &str, label: &str) -> Result<(), String> {
    let len = value.chars().count();
    if !(29..=110).contains(&len) {
        return Err(format!("{label} must be 29-110 characters long."));
    }
    if !value.bytes().all(|b| b == 29 || (33..=126).contains(&b)) {
        return Err(format!(
            "{label} may contain only ASCII 33-126 and ASCII 29 group separator."
        ));
    }
    Ok(())
}

fn validate_receipt_items(items: &[ReceiptItem]) -> Result<(), String> {
    if items.is_empty() {
        return Err("Product receipts require at least one item.".to_owned());
    }
    for (idx, item) in items.iter().enumerate() {
        let label = format!("Item {}", idx + 1);
        validate_positive_u32(item.dep, &format!("{label} department"))?;
        validate_positive_decimal(item.qty, 3, &format!("{label} quantity"))?;
        validate_positive_decimal(item.price, 2, &format!("{label} price"))?;
        validate_non_empty_max(&item.product_code, 50, &format!("{label} productCode"))?;
        validate_non_empty_max(&item.product_name, 50, &format!("{label} productName"))?;
        validate_non_empty_max(&item.unit, 50, &format!("{label} unit"))?;
        validate_discount_pair(
            item.discount,
            item.discount_kind,
            &format!("{label} discount"),
        )?;
        validate_discount_pair(
            item.additional_discount,
            item.additional_discount_kind,
            &format!("{label} additionalDiscount"),
        )?;
        if item
            .adg_code
            .as_ref()
            .is_some_and(|adg| adg.trim().is_empty())
        {
            return Err(format!("{label} adgCode must not be empty when present."));
        }
    }
    Ok(())
}

fn validate_positive_decimal(value: Decimal, max_scale: u32, label: &str) -> Result<(), String> {
    if value <= Decimal::ZERO {
        return Err(format!("{label} must be greater than zero."));
    }
    validate_decimal_scale(value, max_scale, label)
}

fn validate_discount_pair(
    amount: Option<Decimal>,
    kind: Option<DiscountKind>,
    label: &str,
) -> Result<(), String> {
    match (amount, kind) {
        (Some(amount), Some(_)) => {
            validate_decimal_scale(amount, 2, label)?;
            if amount < Decimal::ZERO {
                return Err(format!("{label} must not be negative."));
            }
            Ok(())
        }
        (None, None) => Ok(()),
        (Some(_), None) => Err(format!("{label} type is required when amount is set.")),
        (None, Some(_)) => Err(format!("{label} amount is required when type is set.")),
    }
}

fn validate_non_empty_max(value: &str, max: usize, label: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{label} must not be empty."));
    }
    if value.chars().count() > max {
        return Err(format!("{label} must be at most {max} characters."));
    }
    Ok(())
}

fn validate_return_items(items: &[ReturnItem]) -> Result<(), String> {
    for (idx, item) in items.iter().enumerate() {
        let label = format!("Return item {}", idx + 1);
        if item.rpid <= 0 {
            return Err(format!("{label} rpid must be greater than zero."));
        }
        validate_positive_decimal(item.quantity, 3, &format!("{label} quantity"))?;
    }
    Ok(())
}

fn validate_text_lines(lines: &[TextLine], label: &str) -> Result<(), String> {
    for (idx, line) in lines.iter().enumerate() {
        let item = format!("{label}[{idx}]");
        if !(1..=5).contains(&line.fsize) {
            return Err(format!("{item} fsize must be in 1..=5."));
        }
        if line.text.chars().count() > 100 {
            return Err(format!("{item} text must be at most 100 characters."));
        }
    }
    Ok(())
}

fn validate_bmp_logo(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 30 || &bytes[0..2] != b"BM" {
        return Err("Logo must be a BMP file.".to_owned());
    }
    let bpp = u16::from_le_bytes([bytes[28], bytes[29]]);
    if bpp > 4 {
        return Err(format!(
            "Logo BMP colour depth must be 4 bits or less; got {bpp}."
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_inputs() -> OperationInputs {
        OperationInputs {
            receipt_mode: "simple".to_owned(),
            amount: "10.00".to_owned(),
            card_amount: String::new(),
            partial_amount: String::new(),
            prepayment_amount: String::new(),
            department: "1".to_owned(),
            partner_tin: String::new(),
            payment_system: String::new(),
            use_ext_pos: false,
            rrn: String::new(),
            terminal_id: String::new(),
            crn: "51815332".to_owned(),
            receipt_id: "abc".to_owned(),
            ticket: "1".to_owned(),
            emarks: String::new(),
            json_path: String::new(),
            logo_path: String::new(),
            description: String::new(),
            cash_in: true,
            report_kind: "x".to_owned(),
            report_filter_kind: String::new(),
            report_filter_value: String::new(),
            report_start: "0".to_owned(),
            report_end: "0".to_owned(),
            confirm_operation: false,
        }
    }

    #[test]
    fn connection_rejects_timeout_above_protocol_cap() {
        let Err(err) = connection_settings(&ConnectionInput {
            host: "10.0.0.5",
            port: "1025",
            timeout_seconds: "51",
            password: "pw",
            cashier: "1",
            pin: "1234",
            needs_password: true,
            needs_session: true,
        }) else {
            panic!("expected timeout validation error");
        };
        assert!(err.contains("50 seconds"));
    }

    #[test]
    fn receipt_rejects_bad_partner_tin() {
        let mut inputs = base_inputs();
        inputs.partner_tin = "123".to_owned();
        let Err(err) = build_receipt_request(&inputs) else {
            panic!("expected partner TIN validation error");
        };
        assert!(err.contains("Partner TIN"));
    }

    #[test]
    fn receipt_rejects_money_scale_over_two_places() {
        let mut inputs = base_inputs();
        inputs.amount = "1.001".to_owned();
        let Err(err) = build_receipt_request(&inputs) else {
            panic!("expected scale validation error");
        };
        assert!(err.contains("decimal places"));
    }

    #[test]
    fn emark_validation_checks_length() {
        let mut inputs = base_inputs();
        inputs.emarks = "short".to_owned();
        let Err(err) = single_emark(&inputs) else {
            panic!("expected eMark validation error");
        };
        assert!(err.contains("29-110"));
    }

    #[test]
    fn report_rejects_backwards_range() {
        let mut inputs = base_inputs();
        inputs.report_start = "20".to_owned();
        inputs.report_end = "10".to_owned();
        let Err(err) = build_report_request(&inputs) else {
            panic!("expected report range validation error");
        };
        assert!(err.contains("end"));
    }

    #[test]
    fn report_rejects_negative_range() {
        let mut inputs = base_inputs();
        inputs.report_start = "-1".to_owned();
        let Err(err) = build_report_request(&inputs) else {
            panic!("expected negative report range validation error");
        };
        assert!(err.contains("zero or positive"));
    }

    #[test]
    fn return_rejects_incomplete_terminal_pair() {
        let mut inputs = base_inputs();
        inputs.rrn = "123456789012".to_owned();
        let Err(err) = build_return_request(&inputs) else {
            panic!("expected missing terminal ID validation error");
        };
        assert!(err.contains("Terminal ID"));
    }
}
