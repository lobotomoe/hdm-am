//! The simulated device's state and what each operation does to it.

use super::totals::receipt_total;
use super::{Fault, Reply, SimulatorConfig};
use crate::crypto::{Codec, KEY_LEN};
use crate::operations::{
    CashInOutRequest, GetReturnableReceiptRequest, PrintMode, PrintReceiptRequest,
    PrintReturnReceiptRequest, ReceiptItem,
};
use crate::wire::RESPONSE_CODE_OK;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive as _;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

/// A receipt the device registered: a sale, or the return of one.
pub(super) struct Registered {
    rseq: i64,
    time: i64,
    operator: u32,
    department: u32,
    total: Decimal,
    cash: Decimal,
    card: Decimal,
    items: Vec<ReceiptItem>,
    /// How much of this sale has been returned so far.
    returned: Decimal,
    /// The receipt this one returns; `None` for a sale.
    returns: Option<i64>,
}

pub(super) struct Device {
    config: SimulatorConfig,
    password: Codec,
    /// The session of the last login and the operator who logged in.
    session: Option<(Codec, u32)>,
    last_seq: i64,
    logins: u64,
    cash_in_drawer: Decimal,
    pub(super) receipts: Vec<Registered>,
    pub(super) fault: Option<Fault>,
}

fn now_millis() -> i64 {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis());
    i64::try_from(since_epoch).unwrap_or(i64::MAX)
}

/// An amount as the JSON number the protocol carries, not the string `Decimal` writes by default.
fn number(amount: Decimal) -> Value {
    amount
        .to_f64()
        .and_then(serde_json::Number::from_f64)
        .map_or(Value::Null, Value::Number)
}

fn parse<T: DeserializeOwned>(body: &Value) -> Result<T, u16> {
    // 105: wrong JSON format.
    serde_json::from_value(body.clone()).map_err(|_| 105)
}

impl Device {
    pub(super) fn new(config: SimulatorConfig) -> Self {
        Self {
            password: Codec::from_password(&config.password),
            config,
            session: None,
            last_seq: 0,
            logins: 0,
            cash_in_drawer: Decimal::ZERO,
            receipts: Vec::new(),
            fault: None,
        }
    }

    pub(super) fn handle(&mut self, op: u8, payload: &[u8]) -> Reply {
        let outcome = match op {
            1 | 2 => self.with_password(op, payload),
            3..=16 => return self.in_session(op, payload),
            // 404: wrong operation code.
            _ => Err(404),
        };
        match outcome {
            Ok(payload) => Reply {
                code: RESPONSE_CODE_OK,
                payload,
                lost: false,
            },
            Err(code) => Reply::refused(code),
        }
    }

    /// Ops 1 and 2: encrypted with the key derived from the device's password.
    fn with_password(&mut self, op: u8, payload: &[u8]) -> Result<Vec<u8>, u16> {
        // 101: a payload the password does not decrypt.
        let plain = self.password.decrypt(payload).map_err(|_| 101_u16)?;
        let body: Value = serde_json::from_slice(&plain).map_err(|_| 105_u16)?;
        if body["password"].as_str() != Some(self.config.password.as_str()) {
            return Err(101);
        }
        let answer = if op == 1 {
            self.directory()
        } else {
            self.login(&body)?
        };
        self.password
            .encrypt(answer.to_string().as_bytes())
            .map_err(|_| 500)
    }

    fn directory(&self) -> Value {
        let departments: Vec<u32> = self.config.departments.iter().map(|d| d.id).collect();
        log::info!("op 1: operators and departments listed");
        json!({
            "c": self.config.operators.iter().map(|operator| json!({
                "id": operator.id,
                "name": operator.name,
                "deps": departments,
            })).collect::<Vec<_>>(),
            "d": self.config.departments,
        })
    }

    fn login(&mut self, body: &Value) -> Result<Value, u16> {
        let cashier = body["cashier"].as_u64();
        let operator = self
            .config
            .operators
            .iter()
            .find(|operator| Some(u64::from(operator.id)) == cashier)
            // 112: no such operator.
            .ok_or(112_u16)?;
        // 111: wrong operator password.
        if body["pin"].as_str() != Some(operator.pin.as_str()) {
            return Err(111);
        }
        self.logins += 1;
        let mut seed = Sha256::new();
        seed.update(self.config.password.as_bytes());
        seed.update(self.logins.to_be_bytes());
        seed.update(now_millis().to_be_bytes());
        let mut key = [0u8; KEY_LEN];
        key.copy_from_slice(&seed.finalize()[..KEY_LEN]);
        self.session = Some((Codec::from_key(key), operator.id));
        // The sequence is per session (spec §4.4.5).
        self.last_seq = 0;
        log::info!("op 2: operator {} logged in", operator.id);
        Ok(json!({ "key": BASE64.encode(key) }))
    }

    /// Ops 3 to 16: encrypted with the session key, and numbered.
    fn in_session(&mut self, op: u8, payload: &[u8]) -> Reply {
        // 102: no session, or a payload its key does not decrypt.
        let Some((codec, operator)) = self.session.clone() else {
            return Reply::refused(102);
        };
        let Ok(plain) = codec.decrypt(payload) else {
            return Reply::refused(102);
        };
        let Ok(body) = serde_json::from_slice::<Value>(&plain) else {
            return Reply::refused(105);
        };
        // 104: a sequence number that does not move forward.
        let Some(seq) = body["seq"].as_i64().filter(|seq| *seq > self.last_seq) else {
            return Reply::refused(104);
        };
        self.last_seq = seq;

        let fault = self.fault.take();
        if let Some(Fault::Refuse(code)) = fault {
            log::info!("op {op}: refused with {code}, as injected");
            return Reply::refused(code);
        }
        let lost = fault == Some(Fault::LoseAnswer);
        if lost {
            log::info!("op {op}: carried out, answer lost, as injected");
        }
        let encrypted = self.operate(op, operator, &body).and_then(|answer| {
            codec
                .encrypt(answer.to_string().as_bytes())
                .map_err(|_| 500)
        });
        match encrypted {
            Ok(payload) => Reply {
                code: RESPONSE_CODE_OK,
                payload,
                lost,
            },
            Err(code) => Reply {
                lost,
                ..Reply::refused(code)
            },
        }
    }

    fn operate(&mut self, op: u8, operator: u32, body: &Value) -> Result<Value, u16> {
        match op {
            3 => {
                self.session = None;
                log::info!("op 3: operator {operator} logged out");
                Ok(json!({}))
            }
            4 => self.print_receipt(operator, &parse(body)?),
            5 => {
                // 141: the archive holds no last receipt.
                let last = self.receipts.last().ok_or(141_u16)?;
                log::info!("op 5: copy of receipt {} printed", last.rseq);
                Ok(json!({}))
            }
            6 => self.print_return(operator, &parse(body)?),
            10 => self.returnable(&parse(body)?),
            11 => self.cash_in_out(&parse(body)?),
            12 => Ok(json!({ "dt": now_millis().to_string() })),
            15 => Ok(json!({ "PaymentSystems": [{ "code": 1, "name": "ArCa" }] })),
            _ => {
                log::info!("op {op}: accepted, nothing simulated");
                Ok(json!({}))
            }
        }
    }

    fn department_exists(&self, id: u32) -> bool {
        self.config.departments.iter().any(|d| d.id == id)
    }

    /// The response codes a real device answers for a receipt it will not take.
    fn check_receipt(&self, request: &PrintReceiptRequest) -> Result<u32, u16> {
        if request.mode != PrintMode::Products {
            // 151: no such department.
            return request
                .dep
                .filter(|dep| self.department_exists(*dep))
                .ok_or(151);
        }
        let first = request.items.first().ok_or(400_u16)?;
        for item in &request.items {
            if !self.department_exists(item.dep) {
                return Err(151);
            }
            // 159: a negative price. 165: a price of zero.
            if item.price < Decimal::ZERO {
                return Err(159);
            }
            if item.price.is_zero() {
                return Err(165);
            }
            // 161, 162, 163: an empty product code, name, unit.
            if item.product_code.trim().is_empty() {
                return Err(161);
            }
            if item.product_name.trim().is_empty() {
                return Err(162);
            }
            if item.unit.trim().is_empty() {
                return Err(163);
            }
            // 192: the goods classification code is required.
            if item
                .adg_code
                .as_deref()
                .is_none_or(|code| code.trim().is_empty())
            {
                return Err(192);
            }
        }
        Ok(first.dep)
    }

    fn print_receipt(
        &mut self,
        operator: u32,
        request: &PrintReceiptRequest,
    ) -> Result<Value, u16> {
        let department = self.check_receipt(request)?;
        let total = receipt_total(request)?;
        // 154: the amount must be positive.
        if total <= Decimal::ZERO {
            return Err(154);
        }
        let card = request.paid_amount_card;
        // 167: the card amount exceeds the total.
        if card > total {
            return Err(167);
        }
        let not_cash = card + request.partial_amount + request.pre_payment_amount;
        // 152: paid less than the total.
        if request.paid_amount + not_cash < total {
            return Err(152);
        }
        let cash = total - not_cash;
        let change = request.paid_amount - cash;
        let receipt = Registered {
            rseq: self.next_rseq(),
            time: now_millis(),
            operator,
            department,
            total,
            cash,
            card,
            items: request.items.clone(),
            returned: Decimal::ZERO,
            returns: None,
        };
        self.cash_in_drawer += cash;
        log::info!(
            "op 4: receipt {} registered: total {total}, cash {cash}, card {card}, rrn {:?}",
            receipt.rseq,
            request.rrn
        );
        let mut answer = self.printed(&receipt);
        answer["total"] = number(total);
        answer["change"] = number(change);
        self.receipts.push(receipt);
        Ok(answer)
    }

    /// The sale a return or a lookup names, or the response code for one that cannot be found.
    fn sale_position(&self, crn: &str, receipt_id: i64) -> Result<usize, u16> {
        // 175: the registration number is not this device's.
        if crn.trim() != self.config.crn {
            return Err(175);
        }
        self.receipts
            .iter()
            .position(|receipt| receipt.rseq == receipt_id)
            // 174: the receipt to return is not found.
            .ok_or(174)
    }

    fn print_return(
        &mut self,
        operator: u32,
        request: &PrintReturnReceiptRequest,
    ) -> Result<Value, u16> {
        let receipt_id = i64::try_from(request.return_ticket_id).map_err(|_| 157_u16)?;
        let position = self.sale_position(&request.crn, receipt_id)?;
        let sale = &self.receipts[position];
        // 182: the receipt to return is itself a return.
        if sale.returns.is_some() {
            return Err(182);
        }
        let remaining = sale.total - sale.returned;
        // 158: the receipt has already been returned.
        if remaining <= Decimal::ZERO {
            return Err(158);
        }
        let asked = [
            request.cash_amount_for_return,
            request.card_amount_for_return,
            request.pre_payment_amount_for_return,
        ];
        let full = asked.iter().all(Option::is_none);
        let amount = if full {
            remaining
        } else {
            asked.iter().flatten().copied().sum()
        };
        if amount <= Decimal::ZERO {
            return Err(154);
        }
        // 178: more than can be returned.
        if amount > remaining {
            return Err(178);
        }
        // A full return gives back in cash what the sale took in cash.
        let cash_back = request.cash_amount_for_return.unwrap_or_else(|| {
            if full {
                sale.cash.min(remaining)
            } else {
                Decimal::ZERO
            }
        });
        let card_back = amount - cash_back;
        let (department, sale_time) = (sale.department, sale.time);
        let returned = Registered {
            rseq: self.next_rseq(),
            time: now_millis(),
            operator,
            department,
            total: amount,
            cash: cash_back,
            card: card_back,
            items: Vec::new(),
            returned: Decimal::ZERO,
            returns: Some(receipt_id),
        };
        self.receipts[position].returned += amount;
        self.cash_in_drawer -= cash_back;
        log::info!(
            "op 6: receipt {} returns {amount} of receipt {receipt_id}, rrn {:?}",
            returned.rseq,
            request.rrn
        );
        let mut answer = self.printed(&returned);
        answer["rtime"] = json!(sale_time);
        answer["total"] = number(amount);
        self.receipts.push(returned);
        Ok(answer)
    }

    fn returnable(&self, request: &GetReturnableReceiptRequest) -> Result<Value, u16> {
        // 157: a receipt number that is not a number.
        let receipt_id: i64 = request.receipt_id.trim().parse().map_err(|_| 157_u16)?;
        let sale = &self.receipts[self.sale_position(&request.crn, receipt_id)?];
        log::info!("op 10: receipt {receipt_id} looked up");
        let totals: Vec<Value> = sale
            .items
            .iter()
            .zip(0_i64..)
            .map(|(item, rpid)| {
                json!({
                    "gc": item.product_code,
                    "gn": item.product_name,
                    "qty": number(item.qty),
                    "p": number(item.price),
                    "mu": item.unit,
                    "rpid": rpid,
                    "did": item.dep,
                })
            })
            .collect();
        Ok(json!({
            "rseq": sale.rseq,
            "cid": sale.operator,
            "time": sale.time,
            "type": if sale.returns.is_some() { 2 } else { 0 },
            "did": sale.department,
            "ta": number(sale.total),
            "cash": number(sale.cash),
            "card": number(sale.card),
            "ref": sale.returns,
            "totals": totals,
        }))
    }

    fn cash_in_out(&mut self, request: &CashInOutRequest) -> Result<Value, u16> {
        // 191: the cash amount must be positive.
        if request.amount <= Decimal::ZERO {
            return Err(191);
        }
        if request.is_cash_in {
            self.cash_in_drawer += request.amount;
        } else {
            self.cash_in_drawer -= request.amount;
        }
        log::info!(
            "op 11: {} {}, drawer now holds {}",
            if request.is_cash_in {
                "cash in"
            } else {
                "cash out"
            },
            request.amount,
            self.cash_in_drawer
        );
        Ok(json!({}))
    }

    fn next_rseq(&self) -> i64 {
        i64::try_from(self.receipts.len()).map_or(i64::MAX, |count| count + 1)
    }

    /// What every printed receipt carries, whatever kind it is.
    fn printed(&self, receipt: &Registered) -> Value {
        json!({
            "rseq": receipt.rseq,
            "crn": self.config.crn,
            "sn": self.config.serial_number,
            "tin": self.config.tin,
            "taxpayer": self.config.taxpayer,
            "address": self.config.address,
            "time": receipt.time,
            "fiscal": format!("{:08}", receipt.rseq),
            "lottery": "",
            "prize": 0,
            "change": 0,
            "qr": format!("simulated receipt {} of {}", receipt.rseq, self.config.crn),
            "verificationNumber": format!("SIM{:010}", receipt.rseq),
        })
    }
}
