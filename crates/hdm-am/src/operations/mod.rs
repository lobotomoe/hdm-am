//! Per-operation request and response shapes, serialised as JSON per spec §4.5.
//!
//! Every field name matches the wire-format JSON key exactly (`paidAmount`, `crn`, etc.) so
//! consumers can cross-reference the spec PDF without translation.
//!
//! Forward-compatibility invariants (see the crate-level docs): responses use `#[serde(default)]`
//! on fields added in later spec revisions so older firmware (which omits them) still round-trips,
//! and **no response type uses `#[serde(deny_unknown_fields)]`** so a newer firmware that adds
//! response fields does not break parsing. Preserve both when adding operations.

use crate::wire::OperationCode;
use serde::{Deserialize, Serialize};

/// Op 11 — cash drawer in/out (§4.5.8).
pub mod cash;
/// Op 7/8 — header, footer, and logo configuration (§4.6.3–4.6.4).
pub mod config;
/// Op 1 — operator and department directory (§4.5.1).
pub mod directory;
/// Op 12–16 — miscellaneous device operations (§4.6–4.9).
pub mod misc;
/// Op 4–6, 10 — receipt printing and returns (§4.5.4–4.5.7).
pub mod receipt;
/// Op 9 — fiscal reports (§4.6.2).
pub mod reports;
/// Op 2–3 — operator login and logout (§4.5.2–4.5.3).
pub mod session;

pub use cash::CashInOutRequest;
pub use config::{SetupHeaderFooterRequest, SetupHeaderLogoRequest, TextAlign, TextLine};
pub use directory::{
    DepartmentInfo, ListOpsAndDepsRequest, ListOpsAndDepsResponse, OperatorInfo, TaxationKind,
};
pub use misc::{
    DateTimeRequest, DateTimeResponse, HdmTimeSyncRequest, PaymentSystemEntry,
    PaymentSystemsListRequest, PaymentSystemsListResponse, ReceiptSampleRequest,
    SingleEmarkRequest,
};
pub use receipt::{
    DiscountKind, GetReturnableReceiptRequest, PrintLastReceiptRequest, PrintMode,
    PrintReceiptRequest, PrintReturnReceiptRequest, ReceiptItem, ReceiptResponse, ReturnItem,
    ReturnReceiptResponse, ReturnableReceiptItem, ReturnableReceiptResponse,
};
pub use reports::{FiscalReportKind, FiscalReportRequest, ReportFilter};
pub use session::{OperatorLoginRequest, OperatorLoginResponse, OperatorLogoutRequest};

/// Marker response for operations that do not return a meaningful payload.
///
/// The wire-level response framing is still parsed (header + zero-byte or `{}` payload), but the
/// caller gets no fields to consume.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct EmptyResponse {}

/// Internal trait coupling a request type to its operation code and response type. Implemented
/// for each operation so that the high-level client API can stay short.
pub trait Operation: Serialize {
    /// Wire-level operation code.
    const CODE: OperationCode;
    /// Response type this operation expects back from the HDM.
    type Response: for<'de> Deserialize<'de>;
    /// Whether this operation uses the password-derived key (`true`) or the session key
    /// (`false`). Per spec §4.4.3, only ops 1 and 2 use the password key.
    const USES_PASSWORD_KEY: bool = false;
    /// Whether this operation's *response* carries a secret (e.g. the session key from login) that
    /// must never reach a log. When `true`, the client redacts the decrypted payload from its trace
    /// output. Defaults to `false`.
    const RESPONSE_IS_SECRET: bool = false;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::OperationCode;
    use rust_decimal::Decimal;

    /// `PrintMode` serialises as its integer wire value (1/2/3).
    #[test]
    fn print_mode_wire_encoding() {
        assert_eq!(serde_json::to_string(&PrintMode::Simple).unwrap(), "1");
        assert_eq!(serde_json::to_string(&PrintMode::Products).unwrap(), "2");
        assert_eq!(serde_json::to_string(&PrintMode::Prepayment).unwrap(), "3");
    }

    /// `DiscountKind` covers the full {1, 2, 4, 8, 16} set.
    #[test]
    fn discount_kind_wire_encoding() {
        assert_eq!(serde_json::to_string(&DiscountKind::Percent).unwrap(), "1");
        assert_eq!(
            serde_json::to_string(&DiscountKind::UnitPriceReduction).unwrap(),
            "2"
        );
        assert_eq!(
            serde_json::to_string(&DiscountKind::LineTotalReduction).unwrap(),
            "4"
        );
        assert_eq!(
            serde_json::to_string(&DiscountKind::AdditionalPercent).unwrap(),
            "8"
        );
        assert_eq!(
            serde_json::to_string(&DiscountKind::AdditionalMonetary).unwrap(),
            "16"
        );
    }

    /// `TextAlign` matches spec values 1/2/3.
    #[test]
    fn text_align_wire_encoding() {
        assert_eq!(serde_json::to_string(&TextAlign::Left).unwrap(), "1");
        assert_eq!(serde_json::to_string(&TextAlign::Centered).unwrap(), "2");
        assert_eq!(serde_json::to_string(&TextAlign::Right).unwrap(), "3");
    }

    /// `FiscalReportKind` matches spec values 1 (X) / 2 (Z).
    #[test]
    fn fiscal_report_kind_wire_encoding() {
        assert_eq!(serde_json::to_string(&FiscalReportKind::X).unwrap(), "1");
        assert_eq!(serde_json::to_string(&FiscalReportKind::Z).unwrap(), "2");
    }

    /// `TaxationKind::Unknown(N)` round-trips through JSON without loss.
    #[test]
    fn taxation_kind_preserves_unknown_codes() {
        let unknown = TaxationKind::Unknown(99);
        let json = serde_json::to_string(&unknown).unwrap();
        assert_eq!(json, "99");
        let parsed: TaxationKind = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, TaxationKind::Unknown(99));
    }

    /// `TaxationKind` named variants round-trip cleanly.
    #[test]
    fn taxation_kind_named_variants_round_trip() {
        for &kind in &[
            TaxationKind::VatTaxable,
            TaxationKind::NotVatTaxable,
            TaxationKind::TurnoverTax,
            TaxationKind::ProductionLicensee,
            TaxationKind::Patented,
            TaxationKind::FamilyBusiness,
            TaxationKind::MicroBusiness,
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            let parsed: TaxationKind = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, kind);
        }
    }

    /// `partnerTin: null` is emitted explicitly rather than being skipped.
    #[test]
    fn partner_tin_serialises_as_null_when_absent() {
        let req = PrintReceiptRequest {
            mode: PrintMode::Simple,
            paid_amount: Decimal::ZERO,
            paid_amount_card: Decimal::ZERO,
            partial_amount: Decimal::ZERO,
            pre_payment_amount: Decimal::ZERO,
            dep: Some(1),
            partner_tin: None,
            use_ext_pos: false,
            payment_system: None,
            rrn: None,
            terminal_id: None,
            e_marks: vec![],
            items: vec![],
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(
            json.contains(r#""partnerTin":null"#),
            "partnerTin should serialise as explicit null, got {json}"
        );
    }

    /// All wire-renamed fields use the spec's exact JSON keys.
    #[test]
    fn print_receipt_request_uses_spec_wire_keys() {
        let req = PrintReceiptRequest {
            mode: PrintMode::Products,
            paid_amount: Decimal::from(2200),
            paid_amount_card: Decimal::from(10),
            partial_amount: Decimal::ZERO,
            pre_payment_amount: Decimal::ZERO,
            dep: None,
            partner_tin: None,
            use_ext_pos: true,
            payment_system: None,
            rrn: Some("123456789012".to_owned()),
            terminal_id: Some("12345678".to_owned()),
            e_marks: vec!["xxx".to_owned()],
            items: vec![ReceiptItem {
                dep: 1,
                qty: Decimal::from(3),
                price: Decimal::from(1000),
                product_code: "001".to_owned(),
                product_name: "Pepsi".to_owned(),
                adg_code: Some("0104".to_owned()),
                unit: "litr".to_owned(),
                discount: Some(Decimal::from(10)),
                discount_kind: Some(DiscountKind::Percent),
                additional_discount: Some(Decimal::from(500)),
                additional_discount_kind: Some(DiscountKind::AdditionalMonetary),
            }],
        };
        let json = serde_json::to_string(&req).unwrap();
        for key in &[
            r#""paidAmount":2200.0"#,
            r#""paidAmountCard":10.0"#,
            r#""partialAmount":0.0"#,
            r#""prePaymentAmount":0.0"#,
            r#""useExtPOS":true"#,
            r#""terminalId":"12345678""#,
            r#""eMarks":["xxx"]"#,
            r#""productCode":"001""#,
            r#""productName":"Pepsi""#,
            r#""adgCode":"0104""#,
            r#""discountType":1"#,
            r#""additionalDiscount":500.0"#,
            r#""additionalDiscountType":16"#,
        ] {
            assert!(json.contains(key), "missing key {key} in {json}");
        }
        // Rust-cased names must not appear.
        for forbidden in &["paid_amount", "use_ext_pos", "terminal_id", "product_code"] {
            assert!(!json.contains(forbidden), "unexpected snake_case in {json}");
        }
    }

    /// Sample spec response (§4.5.4 Code Block 4) deserialises into `ReceiptResponse` cleanly.
    #[test]
    fn receipt_response_parses_spec_example() {
        let json = r#"{
            "rseq": 179,
            "crn": "31008940",
            "sn": "Q80414503833",
            "tin": "00000019",
            "taxpayer": "LUSARD",
            "address": "Yerevan",
            "time": 1490190340000,
            "fiscal": "68287355",
            "lottery": "00000002",
            "prize": 0,
            "total": 3000.0,
            "change": 0.0,
            "emarksCount": "1",
            "verificationNumber": "128503",
            "qr": "TIN:00000019..."
        }"#;
        let parsed: ReceiptResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.rseq, 179);
        assert_eq!(parsed.fiscal, "68287355");
        assert_eq!(parsed.qr.as_deref(), Some("TIN:00000019..."));
        assert_eq!(parsed.verification_number.as_deref(), Some("128503"));
    }

    /// Older HDM firmware that omits `qr`/`verificationNumber`/`emarksCount` must still parse.
    #[test]
    fn receipt_response_parses_v05_response_without_post_v05_fields() {
        let json = r#"{
            "rseq": 100,
            "crn": "0001",
            "sn": "SN",
            "tin": "TIN",
            "taxpayer": "TP",
            "address": "Addr",
            "time": 1,
            "fiscal": "FN",
            "lottery": "L",
            "prize": 0,
            "total": 50.0,
            "change": 0.0
        }"#;
        let parsed: ReceiptResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.rseq, 100);
        assert!(parsed.qr.is_none());
        assert!(parsed.verification_number.is_none());
        assert!(parsed.emarks_count.is_none());
    }

    /// `Operation` trait correctly couples request type → op code → response type.
    ///
    /// The `USES_PASSWORD_KEY` constants are checked at compile time via `const { assert!(..) }`
    /// blocks; any future drift would be a build failure, not a test failure.
    #[test]
    fn operation_couples_request_to_code_and_response() {
        const { assert!(OperatorLoginRequest::USES_PASSWORD_KEY) };
        const { assert!(!PrintReceiptRequest::USES_PASSWORD_KEY) };
        const { assert!(ListOpsAndDepsRequest::USES_PASSWORD_KEY) };
        const { assert!(!PaymentSystemsListRequest::USES_PASSWORD_KEY) };

        assert_eq!(OperatorLoginRequest::CODE, OperationCode::OperatorLogin);
        assert_eq!(PrintReceiptRequest::CODE, OperationCode::PrintReceipt);
        assert_eq!(ListOpsAndDepsRequest::CODE, OperationCode::ListOpsAndDeps);
        assert_eq!(
            PaymentSystemsListRequest::CODE,
            OperationCode::PaymentSystemsList
        );
    }

    /// Pins the two return-flow operations to their exact wire codes from the spec's
    /// operation-codes table (§4.4.1): print return = 6, get returnable (read-only lookup) = 10.
    /// These were historically swapped because the spec describes them in section order
    /// (§4.5.6 get, §4.5.7 print) which is the reverse of the code assignment.
    #[test]
    fn return_flow_codes_match_spec_table() {
        assert_eq!(PrintReturnReceiptRequest::CODE as u8, 6);
        assert_eq!(GetReturnableReceiptRequest::CODE as u8, 10);
    }

    /// Empty response deserialises from both `{}` and arbitrary objects (ignores unknown).
    #[test]
    fn empty_response_accepts_empty_and_unknown_fields() {
        let _: EmptyResponse = serde_json::from_str("{}").unwrap();
        let _: EmptyResponse = serde_json::from_str(r#"{"x":1,"y":"z"}"#).unwrap();
    }

    /// Op 15 payment-systems response handles the spec's wire shape with capitalised key.
    #[test]
    fn payment_systems_list_parses_spec_example() {
        let json = r#"{
            "PaymentSystems": [
                { "code": 1, "name": "Card Payment" },
                { "code": 13, "name": "IDRAM" }
            ]
        }"#;
        let parsed: PaymentSystemsListResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.payment_systems.len(), 2);
        assert_eq!(parsed.payment_systems[0].code, 1);
        assert_eq!(parsed.payment_systems[1].code, 13);
        assert_eq!(parsed.payment_systems[1].name, "IDRAM");
    }

    /// Op 10 `ReturnableReceiptResponse` deserialises the spec's §4.5.6 Code Block 7 example.
    /// This is the only coverage of that struct's `#[serde(default, with = "dec_opt")]` fields,
    /// because the operation is unverifiable on the available test stand (always returns 503).
    #[test]
    fn returnable_receipt_parses_spec_code_block_7() {
        let json = r#"{
            "time": 1450260000, "type": 0, "ref": 0, "cid": 3,
            "ta": 3000, "cash": 1000, "card": 1000, "ppu": 0, "ppa": 1000,
            "saleType": 0, "pTin": "12345678",
            "eMarks": ["aaa", "bbb"],
            "totals": [
                { "gc": "001", "gn": "Product1", "qty": 1, "p": 1000, "mu": "x",
                  "rpid": 0, "dsc": null, "adsc": null, "dsct": null,
                  "did": 1, "dt": 16.67, "dtm": 1, "t": 833.33, "tt": 1000.00 }
            ]
        }"#;
        let r: ReturnableReceiptResponse = serde_json::from_str(json).unwrap();
        // Numeric-as-number fields (cid/saleType/type/ref) deserialise via i64.
        assert_eq!(r.cid, Some(3));
        assert_eq!(r.sale_type, Some(0));
        assert_eq!(r.kind, Some(0));
        assert_eq!(r.returned_receipt, Some(0));
        // Money fields go through dec_opt (JSON number -> Decimal).
        assert_eq!(r.ta, Some(Decimal::from(3000)));
        assert_eq!(r.partner_tin.as_deref(), Some("12345678"));
        assert_eq!(r.e_marks.len(), 2);
        // Fields absent from the example fall back to None via `default` (not a parse error).
        assert_eq!(r.rseq, None);
        assert_eq!(r.sub_type, None);
        assert_eq!(r.returned_crn, None);
        // Line item: present number, present null, and a fractional value all parse.
        assert_eq!(r.totals.len(), 1);
        let [item] = r.totals.as_slice() else {
            panic!("expected one item")
        };
        assert_eq!(item.rpid, Some(0));
        assert_eq!(item.qty, Some(Decimal::from(1)));
        assert_eq!(item.discount, None); // dsc: null
        assert_eq!(item.did, Some(1));
        assert_eq!(item.vat_amount, Some(Decimal::new(1667, 2)));
    }

    /// An empty object must deserialise into an all-`None` `ReturnableReceiptResponse` — proves
    /// every field is genuinely optional (a partial firmware response won't hard-error).
    #[test]
    fn returnable_receipt_parses_empty_object() {
        let r: ReturnableReceiptResponse = serde_json::from_str("{}").unwrap();
        assert!(r.rseq.is_none() && r.cid.is_none() && r.ta.is_none());
        assert!(r.e_marks.is_empty() && r.totals.is_empty());
    }

    // --- Request deserialization (the input side the HTTP bridge relies on). ---

    /// `PrintMode` round-trips through its integer wire value and rejects unknown codes.
    #[test]
    fn print_mode_deserialises_from_wire_value() {
        assert_eq!(
            serde_json::from_str::<PrintMode>("1").unwrap(),
            PrintMode::Simple
        );
        assert_eq!(
            serde_json::from_str::<PrintMode>("3").unwrap(),
            PrintMode::Prepayment
        );
        assert!(serde_json::from_str::<PrintMode>("9").is_err());
    }

    /// `FiscalReportKind` round-trips through 1 (X) / 2 (Z) and rejects unknown codes.
    #[test]
    fn fiscal_report_kind_deserialises_from_wire_value() {
        assert_eq!(
            serde_json::from_str::<FiscalReportKind>("1").unwrap(),
            FiscalReportKind::X
        );
        assert_eq!(
            serde_json::from_str::<FiscalReportKind>("2").unwrap(),
            FiscalReportKind::Z
        );
        assert!(serde_json::from_str::<FiscalReportKind>("0").is_err());
    }

    /// A minimal receipt JSON parses, with `eMarks`/`items` defaulting to empty when omitted.
    #[test]
    fn print_receipt_request_deserialises_with_defaults() {
        let json = r#"{
            "mode": 1, "paidAmount": 1000.0, "paidAmountCard": 0.0,
            "partialAmount": 0.0, "prePaymentAmount": 0.0,
            "useExtPOS": false, "dep": 1
        }"#;
        let req: PrintReceiptRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.mode, PrintMode::Simple);
        assert_eq!(req.paid_amount, Decimal::from(1000));
        assert_eq!(req.dep, Some(1));
        assert!(req.partner_tin.is_none());
        assert!(req.e_marks.is_empty());
        assert!(req.items.is_empty());
    }

    /// The `#[serde(flatten)] Option<ReportFilter>` round-trips: a filter key parses into the
    /// right variant, and its absence yields `None` (the serde flatten+enum gotcha this guards).
    #[test]
    fn fiscal_report_request_filter_round_trips() {
        let with_filter: FiscalReportRequest =
            serde_json::from_str(r#"{"reportType":1,"deptId":5,"startDate":0,"endDate":0}"#)
                .unwrap();
        assert_eq!(with_filter.kind, FiscalReportKind::X);
        assert_eq!(with_filter.filter, Some(ReportFilter::Department(5)));

        let no_filter: FiscalReportRequest =
            serde_json::from_str(r#"{"reportType":2,"startDate":0,"endDate":0}"#).unwrap();
        assert_eq!(no_filter.kind, FiscalReportKind::Z);
        assert_eq!(no_filter.filter, None);

        // Build -> serialize -> deserialize preserves the chosen filter.
        let original = FiscalReportRequest {
            kind: FiscalReportKind::X,
            filter: Some(ReportFilter::Cashier(7)),
            start_date: 0,
            end_date: 0,
        };
        let json = serde_json::to_string(&original).unwrap();
        let back: FiscalReportRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.filter, Some(ReportFilter::Cashier(7)));
    }
}
