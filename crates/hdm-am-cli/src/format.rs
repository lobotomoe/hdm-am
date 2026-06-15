//! Human-readable rendering of HDM responses. Kept deliberately plain — no colour, no unicode
//! decoration — so output is easy to pipe and grep.

use hdm_am::{
    ListOpsAndDepsResponse, PaymentSystemsListResponse, ReturnableReceiptResponse, TaxationKind,
};

/// Render the operators-and-departments listing (op 1).
pub fn operators(response: &ListOpsAndDepsResponse) {
    if response.operators.is_empty() {
        println!("Operators: none registered");
    } else {
        println!("Operators:");
        for op in &response.operators {
            let deps = op
                .deps
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            println!("  [{}] {:<24} departments: [{}]", op.id, op.name, deps);
        }
    }

    if response.departments.is_empty() {
        println!("Departments: none registered");
    } else {
        println!("Departments:");
        for dep in &response.departments {
            println!(
                "  [{}] {:<24} taxation: {}",
                dep.id,
                dep.name,
                taxation(dep.kind)
            );
        }
    }
}

/// Render the payment-systems listing (op 15).
pub fn payment_systems(response: &PaymentSystemsListResponse) {
    if response.payment_systems.is_empty() {
        println!("Payment systems: none configured");
        return;
    }
    println!("Payment systems:");
    for ps in &response.payment_systems {
        println!("  [{}] {}", ps.code, ps.name);
    }
}

/// Render the result of a returnable-receipt lookup (op 10, read-only).
pub fn returnable_receipt(response: &ReturnableReceiptResponse) {
    println!("Receipt lookup:");
    if let Some(rseq) = response.rseq {
        println!("  receipt seq:  {rseq}");
    }
    if let Some(cid) = response.cid {
        println!("  cashier id:   {cid}");
    }
    if let Some(st) = response.sale_type {
        println!("  sale type:    {}", sale_type(st));
    }
    if let Some(ta) = response.ta {
        println!("  total:        {ta}");
    }
    if let Some(cash) = response.cash {
        println!("  cash:         {cash}");
    }
    if let Some(card) = response.card {
        println!("  card:         {card}");
    }
    if let Some(tin) = &response.partner_tin {
        println!("  partner tin:  {tin}");
    }
    if !response.e_marks.is_empty() {
        println!("  eMark codes:  {}", response.e_marks.len());
    }
    if response.totals.is_empty() {
        println!("  items:        none (simple/prepayment receipt)");
    } else {
        println!("  items:");
        for item in &response.totals {
            let rpid = item.rpid.map_or_else(|| "?".to_owned(), |r| r.to_string());
            let name = item.product_name.as_deref().unwrap_or("?");
            let qty = item.qty.map(|q| q.to_string()).unwrap_or_default();
            let price = item.price.map(|p| p.to_string()).unwrap_or_default();
            println!("    [{rpid}] {name}  qty {qty} x {price}");
        }
    }
}

/// Spell out a sale-type code from a returnable-receipt lookup (`saleType`: 0/2/3).
fn sale_type(code: i64) -> String {
    match code {
        0 => "sale".to_owned(),
        2 => "return".to_owned(),
        3 => "prepayment".to_owned(),
        other => format!("unknown ({other})"),
    }
}

/// Spell out a taxation kind, falling back to the raw code for forward-compat values.
fn taxation(kind: TaxationKind) -> String {
    match kind {
        TaxationKind::VatTaxable => "VAT-taxable".to_owned(),
        TaxationKind::NotVatTaxable => "not VAT-taxable".to_owned(),
        TaxationKind::TurnoverTax => "turnover tax".to_owned(),
        TaxationKind::ProductionLicensee => "production licensee".to_owned(),
        TaxationKind::Patented => "patented".to_owned(),
        TaxationKind::FamilyBusiness => "family business".to_owned(),
        TaxationKind::MicroBusiness => "micro-business".to_owned(),
        TaxationKind::Unknown(code) => format!("unknown (code {code})"),
        // `TaxationKind` is `#[non_exhaustive]`; cover spec revisions that add new variants.
        _ => "unrecognised".to_owned(),
    }
}
