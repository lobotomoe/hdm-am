//! Human-facing formatting for GUI results and HDM errors.

use std::collections::HashMap;

use hdm_am::{
    DepartmentInfo, Error as HdmError, ListOpsAndDepsResponse, PaymentSystemsListResponse,
    ReceiptResponse, ReturnReceiptResponse, ReturnableReceiptResponse, ServerErrorKind,
    TaxationKind, VendorErrorKind,
};

/// Format operators/departments response.
#[must_use]
pub fn operators(response: &ListOpsAndDepsResponse) -> String {
    let departments = response
        .departments
        .iter()
        .map(|department| (department.id, department))
        .collect::<HashMap<_, _>>();
    let mut lines = vec![format!(
        "Operators: {}\nDepartments: {}",
        response.operators.len(),
        response.departments.len()
    )];

    if !response.operators.is_empty() {
        lines.push(String::new());
        lines.push("Operators".to_owned());
        for operator in &response.operators {
            lines.push(format!(
                "  [{}] {}",
                operator.id,
                display_name(&operator.name, "[operator name not provided]")
            ));
            lines.push(format!(
                "      departments: {}",
                department_list(&operator.deps, &departments)
            ));
        }
    }

    if !response.departments.is_empty() {
        lines.push(String::new());
        lines.push("Departments".to_owned());
        for department in &response.departments {
            lines.push(format!(
                "  [{}] {}  taxation: {}",
                department.id,
                display_name(&department.name, "[department name not provided]"),
                taxation_label(department.kind)
            ));
        }
    }

    lines.join("\n")
}

fn department_list(deps: &[u32], departments: &HashMap<u32, &DepartmentInfo>) -> String {
    if deps.is_empty() {
        return "none".to_owned();
    }

    deps.iter()
        .map(|id| {
            departments.get(id).map_or_else(
                || format!("[{id}] unknown department"),
                |department| department_summary(department),
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn department_summary(department: &DepartmentInfo) -> String {
    format!(
        "[{}] {} / {}",
        department.id,
        display_name(&department.name, "[department name not provided]"),
        taxation_label(department.kind)
    )
}

/// Format payment systems response.
#[must_use]
pub fn payment_systems(response: &PaymentSystemsListResponse) -> String {
    if response.payment_systems.is_empty() {
        return "No payment systems configured.".to_owned();
    }
    let lines = response
        .payment_systems
        .iter()
        .map(|entry| format!("  [{}] {}", entry.code, entry.name))
        .collect::<Vec<_>>()
        .join("\n");
    format!("Payment systems:\n{lines}")
}

/// Format fiscal receipt response.
#[must_use]
pub fn receipt(response: &ReceiptResponse) -> String {
    let mut lines = vec![
        "Fiscal receipt printed".to_owned(),
        format!("  fiscal number: {}", fallback(&response.fiscal)),
        format!("  receipt seq:   {}", response.rseq),
        format!("  reg number:    {}", fallback(&response.crn)),
        format!("  serial:        {}", fallback(&response.sn)),
        format!("  total:         {:.2}", response.total),
        format!("  change:        {:.2}", response.change),
    ];
    if !response.lottery.is_empty() {
        lines.push(format!("  lottery:       {}", response.lottery));
    }
    if let Some(qr) = &response.qr {
        lines.push(format!("  qr:            {qr}"));
    }
    if let Some(verification) = &response.verification_number {
        lines.push(format!("  verification:  {verification}"));
    }
    if let Some(count) = &response.emarks_count {
        lines.push(format!("  eMarks count:  {count}"));
    }
    lines.join("\n")
}

/// Format return receipt response.
#[must_use]
pub fn return_receipt(response: &ReturnReceiptResponse) -> String {
    let mut lines = vec![
        "Return receipt printed".to_owned(),
        format!("  return seq: {}", response.rseq),
        format!("  fiscal:     {}", fallback(&response.fiscal)),
        format!("  reg number: {}", fallback(&response.crn)),
        format!("  total:      {:.2}", response.total),
        format!("  change:     {:.2}", response.change),
    ];
    if let Some(verification) = &response.verification_number {
        lines.push(format!("  verification: {verification}"));
    }
    if let Some(count) = &response.emarks_count {
        lines.push(format!("  eMarks count: {count}"));
    }
    lines.join("\n")
}

/// Format returnable receipt lookup response.
#[must_use]
pub fn returnable_receipt(response: &ReturnableReceiptResponse) -> String {
    let mut lines = vec!["Receipt lookup".to_owned()];
    if let Some(rseq) = response.rseq {
        lines.push(format!("  receipt seq: {rseq}"));
    }
    if let Some(cid) = response.cid {
        lines.push(format!("  cashier id:  {cid}"));
    }
    if let Some(sale_type) = response.sale_type {
        lines.push(format!("  sale type:   {}", sale_type_label(sale_type)));
    }
    if let Some(total) = response.ta {
        lines.push(format!("  total:       {total}"));
    }
    if let Some(cash) = response.cash {
        lines.push(format!("  cash:        {cash}"));
    }
    if let Some(card) = response.card {
        lines.push(format!("  card:        {card}"));
    }
    if let Some(tin) = &response.partner_tin {
        lines.push(format!("  partner tin: {tin}"));
    }
    lines.push(format!("  eMarks:      {}", response.e_marks.len()));
    lines.push(format!("  items:       {}", response.totals.len()));
    if !response.totals.is_empty() {
        lines.push(String::new());
        lines.push("Items".to_owned());
        for item in &response.totals {
            let rpid = item
                .rpid
                .map_or_else(|| "?".to_owned(), |value| value.to_string());
            let name = item.product_name.as_deref().unwrap_or("?");
            let qty = item
                .qty
                .map_or_else(|| "?".to_owned(), |value| value.to_string());
            let price = item
                .price
                .map_or_else(|| "?".to_owned(), |value| value.to_string());
            lines.push(format!("  [{rpid}] {name}  qty {qty} x {price}"));
        }
    }
    lines.join("\n")
}

/// Format an HDM error with a user-action hint.
#[must_use]
pub fn hdm_error(context: &str, err: &HdmError) -> String {
    let mut lines = vec![format!("{context}: {}", error_title(err))];
    if let HdmError::Server { code, kind } = err {
        lines.push(format!("Code: {code}"));
        lines.push(format!("Meaning: {}", server_error_meaning(*kind)));
        lines.push(format!("Suggested action: {}", server_error_hint(*kind)));
    } else {
        lines.push(format!("Details: {err}"));
    }

    let mut recovery = Vec::new();
    if err.requires_relogin() {
        recovery.push("log in again");
    }
    if err.requires_reconnect() {
        recovery.push("reconnect");
    }
    if err.is_retryable() {
        recovery.push("retry after fixing the transient condition");
    }
    if !recovery.is_empty() {
        lines.push(format!("Recovery: {}.", recovery.join(", ")));
    }
    lines.join("\n")
}

const fn fallback(value: &str) -> &str {
    if value.is_empty() { "n/a" } else { value }
}

fn sale_type_label(code: i64) -> String {
    match code {
        0 => "sale".to_owned(),
        2 => "return".to_owned(),
        3 => "prepayment".to_owned(),
        other => format!("unknown ({other})"),
    }
}

fn taxation_label(kind: TaxationKind) -> String {
    match kind {
        TaxationKind::VatTaxable => "VAT-taxable".to_owned(),
        TaxationKind::NotVatTaxable => "not VAT-taxable".to_owned(),
        TaxationKind::TurnoverTax => "turnover tax".to_owned(),
        TaxationKind::ProductionLicensee => "production licensee".to_owned(),
        TaxationKind::Patented => "patented".to_owned(),
        TaxationKind::FamilyBusiness => "family business".to_owned(),
        TaxationKind::MicroBusiness => "micro-business".to_owned(),
        TaxationKind::Unknown(code) => format!("unknown (code {code})"),
        _ => "unrecognised".to_owned(),
    }
}

const fn display_name<'a>(name: &'a str, fallback: &'static str) -> &'a str {
    if name.is_empty() { fallback } else { name }
}

const fn error_title(err: &HdmError) -> &'static str {
    match err {
        HdmError::Transport(_) => "Connection or network error",
        HdmError::Server { .. } => "Device rejected the request",
        HdmError::Crypto(_) => "Session encryption error",
        HdmError::Decode(_) => "Could not decode device response",
        HdmError::Encode(_) => "Could not encode request",
        HdmError::NotLoggedIn => "Not logged in",
        HdmError::PayloadTooLarge { .. } => "Request is too large",
        _ => "Unexpected HDM client error",
    }
}

const fn server_error_meaning(kind: ServerErrorKind) -> &'static str {
    match kind {
        ServerErrorKind::InternalHdmError => "Internal HDM error.",
        ServerErrorKind::BadRequest => "The HDM could not process the request.",
        ServerErrorKind::BadProtocolVersion => "Protocol version mismatch.",
        ServerErrorKind::UnauthorizedConnection => "This computer is not authorised by the HDM.",
        ServerErrorKind::BadOperationCode => "Unsupported operation code.",
        ServerErrorKind::CryptographicError | ServerErrorKind::SessionEncryptionError => {
            "The request could not be decrypted with the expected key."
        }
        ServerErrorKind::HeaderFormatError => "The request header is malformed.",
        ServerErrorKind::BadSequenceNumber => "The request sequence number was rejected.",
        ServerErrorKind::BadJsonFormat => "The JSON payload is malformed.",
        ServerErrorKind::LastReceiptArchiveEmpty => "The last-receipt archive is empty.",
        ServerErrorKind::LastReceiptDifferentUser => {
            "The last receipt belongs to another operator."
        }
        ServerErrorKind::GenericPrintError => "The printer reported an error.",
        ServerErrorKind::PrinterInitError => "The printer could not initialise.",
        ServerErrorKind::PrinterOutOfPaper => "The printer is out of paper.",
        ServerErrorKind::BadOperatorPassword => "The operator PIN/password is incorrect.",
        ServerErrorKind::NoSuchOperator => "The operator does not exist or is not allowed.",
        ServerErrorKind::InactiveOperator => "The operator is inactive.",
        ServerErrorKind::GenericLoginPrintError => "The login flow failed during printing.",
        ServerErrorKind::NoSuchDepartment => {
            "Department is missing or unavailable to this operator."
        }
        ServerErrorKind::PaidLessThanTotal => "Paid amount is less than receipt total.",
        ServerErrorKind::AmountExceedsLimit => "Receipt amount exceeds the device limit.",
        ServerErrorKind::AmountMustBePositive => "Receipt amount must be positive.",
        ServerErrorKind::HdmSyncRequired | ServerErrorKind::SyncIncomplete => {
            "The HDM needs synchronisation."
        }
        ServerErrorKind::BadReturnReceiptNumber => "Return receipt number is invalid.",
        ServerErrorKind::ReceiptAlreadyReturned => "The receipt was already returned.",
        ServerErrorKind::NonPositiveProductPrice | ServerErrorKind::ZeroProductPrice => {
            "Product price or quantity is not positive."
        }
        ServerErrorKind::DiscountPercentOutOfRange => "Discount percent is outside 0..100.",
        ServerErrorKind::BadProductCode => "Product code is invalid.",
        ServerErrorKind::BadProductName => "Product name is invalid.",
        ServerErrorKind::EmptyProductUnit => "Product unit is empty.",
        ServerErrorKind::CashlessPaymentFailure => "Cashless payment failed.",
        ServerErrorKind::FinalPriceCalculationError => "Final price calculation failed.",
        ServerErrorKind::CardAmountExceedsTotal => "Card amount is greater than total.",
        ServerErrorKind::CardAmountCoversAllCashRedundant => {
            "Card amount covers the full total; cash amount is redundant."
        }
        ServerErrorKind::ReportFiltersError => "More than one report filter was sent.",
        ServerErrorKind::ReportTimeRangeError => "Report time range is too large.",
        ServerErrorKind::InvalidItemPrice => "Item price value is invalid.",
        ServerErrorKind::WrongReceiptType => "Receipt type is invalid for this operation.",
        ServerErrorKind::InvalidDiscountType => "Discount type is invalid.",
        ServerErrorKind::ReturnReceiptNotFound => "Receipt to return was not found.",
        ServerErrorKind::BadReturnReceiptRegNum => "Return receipt CRN is invalid.",
        ServerErrorKind::LastReceiptNotFound => "Last receipt does not exist.",
        ServerErrorKind::ReturnNotSupportedForType => "This receipt type cannot be returned.",
        ServerErrorKind::AmountCannotBeReturned => "Requested return amount cannot be processed.",
        ServerErrorKind::PartialMustBeReturnedInFull => {
            "Partial-payment receipt must be returned in full."
        }
        ServerErrorKind::FullReturnExceedsAmount => "Full return exceeds available amount.",
        ServerErrorKind::BadReturnProductQuantity => "Return product quantity is invalid.",
        ServerErrorKind::ReturnReceiptIsReturn => "The selected receipt is already a return.",
        ServerErrorKind::BadAtgCode => "ATG/ADG tax code is invalid.",
        ServerErrorKind::InvalidPrepaymentReturn => "Prepayment return request is invalid.",
        ServerErrorKind::PartialReturnSyncRequired => "Partial return requires HDM software sync.",
        ServerErrorKind::BadPrepaymentAmount => "Prepayment amount is invalid.",
        ServerErrorKind::BadPrepaymentList => "Prepayment item list is invalid.",
        ServerErrorKind::BadAmounts => "One or more amounts are invalid.",
        ServerErrorKind::BadRounding => "Amount rounding is invalid.",
        ServerErrorKind::PaymentUnavailable => "Payment method is unavailable.",
        ServerErrorKind::NonPositiveCashAmount => "Cash in/out amount must be positive.",
        ServerErrorKind::AtgCodeRequired => "ATG/ADG tax code is required.",
        ServerErrorKind::BadPartnerTinFormat => "Partner TIN format is invalid.",
        ServerErrorKind::EmarksNotAllowedInPrepayment => "eMarks are not allowed for prepayment.",
        ServerErrorKind::BadEmarkFormat => "eMark format is invalid.",
        ServerErrorKind::ForeignCountryEmark => "The eMark code belongs to another country.",
        ServerErrorKind::Vendor(VendorErrorKind::ExternalProgramBlocked) => {
            "The HDM screen is being used; external access is blocked."
        }
        ServerErrorKind::Vendor(VendorErrorKind::ServerDataMismatch) => {
            "The device and tax-authority server disagree about the requested data."
        }
        ServerErrorKind::Unknown(_) => "The HDM returned an undocumented response code.",
        _ => "The HDM returned a response code unknown to this GUI build.",
    }
}

const fn server_error_hint(kind: ServerErrorKind) -> &'static str {
    match kind {
        ServerErrorKind::UnauthorizedConnection => {
            "Add this computer's IP to the HDM external-program settings."
        }
        ServerErrorKind::BadOperatorPassword => "Check the cashier ID and PIN.",
        ServerErrorKind::NoSuchOperator | ServerErrorKind::InactiveOperator => {
            "Choose an active operator registered on the HDM."
        }
        ServerErrorKind::PrinterOutOfPaper => "Load paper and retry the operation.",
        ServerErrorKind::HdmSyncRequired
        | ServerErrorKind::SyncIncomplete
        | ServerErrorKind::PartialReturnSyncRequired => "Run time sync, then retry.",
        ServerErrorKind::NoSuchDepartment => {
            "Use Operators to check department IDs and operator access."
        }
        ServerErrorKind::PaidLessThanTotal
        | ServerErrorKind::BadAmounts
        | ServerErrorKind::BadRounding
        | ServerErrorKind::CardAmountExceedsTotal
        | ServerErrorKind::CardAmountCoversAllCashRedundant => {
            "Check cash/card/partial/prepayment amounts and rounding."
        }
        ServerErrorKind::BadProductCode
        | ServerErrorKind::BadProductName
        | ServerErrorKind::EmptyProductUnit
        | ServerErrorKind::NonPositiveProductPrice
        | ServerErrorKind::ZeroProductPrice
        | ServerErrorKind::InvalidItemPrice
        | ServerErrorKind::BadAtgCode
        | ServerErrorKind::AtgCodeRequired => "Fix the receipt item JSON and retry.",
        ServerErrorKind::ReturnReceiptNotFound
        | ServerErrorKind::BadReturnReceiptRegNum
        | ServerErrorKind::BadReturnReceiptNumber => {
            "Check CRN and receipt/ticket identifiers; lookup may need the Receipt_ID from QR."
        }
        ServerErrorKind::BadEmarkFormat | ServerErrorKind::EmarksNotAllowedInPrepayment => {
            "Check eMark length and allowed ASCII characters."
        }
        ServerErrorKind::Vendor(VendorErrorKind::ExternalProgramBlocked) => {
            "Close the active screen/action on the HDM and retry."
        }
        ServerErrorKind::Vendor(VendorErrorKind::ServerDataMismatch) => {
            "Verify the receipt identifier with the tax authority or device records."
        }
        _ => "Review the input fields and device state, then retry if appropriate.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_error_includes_hint() {
        let err = HdmError::Server {
            code: 145,
            kind: ServerErrorKind::PrinterOutOfPaper,
        };
        let text = hdm_error("printing", &err);
        assert!(text.contains("out of paper"));
        assert!(text.contains("Load paper"));
    }
}
