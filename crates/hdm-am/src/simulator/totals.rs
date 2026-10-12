//! What a receipt comes to, the plain way: no tax device's rounding rules, only arithmetic.

use crate::operations::{DiscountKind, PrintMode, PrintReceiptRequest, ReceiptItem};
use rust_decimal::{Decimal, RoundingStrategy};

/// Money is kept to two decimals, halves away from zero.
fn money(amount: Decimal) -> Decimal {
    amount.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

/// One discount applied to a line. `None` when the kind does not fit the slot it was sent in.
fn discounted(
    line: Decimal,
    quantity: Decimal,
    discount: Option<Decimal>,
    kind: Option<DiscountKind>,
) -> Option<Decimal> {
    let (Some(discount), Some(kind)) = (discount, kind) else {
        return Some(line);
    };
    let hundred = Decimal::ONE_HUNDRED;
    match kind {
        DiscountKind::Percent | DiscountKind::AdditionalPercent => (discount >= Decimal::ZERO
            && discount <= hundred)
            .then(|| line - line * discount / hundred),
        DiscountKind::UnitPriceReduction => Some(line - discount * quantity),
        DiscountKind::LineTotalReduction | DiscountKind::AdditionalMonetary => {
            Some(line - discount)
        }
    }
}

/// A line's total after both of its discounts, or the response code that refuses it.
pub(super) fn line_total(item: &ReceiptItem) -> Result<Decimal, u16> {
    let gross = item.qty * item.price;
    // 160: discount percentage out of range.
    let once = discounted(gross, item.qty, item.discount, item.discount_kind).ok_or(160_u16)?;
    let twice = discounted(
        once,
        item.qty,
        item.additional_discount,
        item.additional_discount_kind,
    )
    .ok_or(160_u16)?;
    // 166: the final price cannot be calculated.
    if twice < Decimal::ZERO {
        return Err(166);
    }
    Ok(money(twice))
}

/// The receipt's total: its lines, or for a receipt without lines what was paid.
pub(super) fn receipt_total(request: &PrintReceiptRequest) -> Result<Decimal, u16> {
    if request.mode != PrintMode::Products {
        return Ok(money(
            request.paid_amount + request.paid_amount_card + request.partial_amount,
        ));
    }
    let mut total = Decimal::ZERO;
    for item in &request.items {
        total += line_total(item)?;
    }
    Ok(total)
}
