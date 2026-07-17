//! `OpenAPI` 3.1 document generation for the bridge's HTTP surface.
//!
//! The document is assembled from the same `schemars`-derived schemas the handlers serialize, so it
//! cannot drift from the wire types. The `dump-openapi` example writes it to `docs/openapi.json` and
//! the CI `--check` gate keeps the committed copy current; the bridge serves that committed copy
//! verbatim at `GET /v1/openapi.json` (see the `routes` module).
//!
//! Two source-of-truth guarantees hold this together:
//! - The component schemas come straight from `JsonSchema` derives on the wire types, so request and
//!   response shapes match the implementation.
//! - The path list comes from `OPERATIONS`, and the `documents_every_route` test asserts that list
//!   and the live router expose exactly the same operation paths — so a new route can't be added
//!   without appearing in the document.

use schemars::{Schema, SchemaGenerator, generate::SchemaSettings};
use serde_json::{Map, Value, json};

use hdm_am::{
    CashInOutRequest, DateTimeResponse, EmptyResponse, FiscalReportRequest,
    GetReturnableReceiptRequest, ListOpsAndDepsResponse, PaymentSystemsListResponse,
    PrintReceiptRequest, PrintReturnReceiptRequest, ReceiptResponse, ReturnReceiptResponse,
    ReturnableReceiptResponse, SetupHeaderFooterRequest, SetupHeaderLogoRequest,
    SingleEmarkRequest,
};

use crate::config::PartialConn;
use crate::error::ErrorBody;
use crate::routes::{HealthOk, Info, StatusOk};

/// Connection-resolution tier an operation needs. Surfaced in the operation description so callers
/// know which `connection` fields must ultimately be present (via the bridge's configured default
/// or a per-request override).
#[derive(Clone, Copy)]
enum Conn {
    /// Endpoint plus the access password.
    Password,
    /// Full operator session: endpoint, password, `cashier`, and `pin`.
    Session,
}

impl Conn {
    const fn note(self) -> &'static str {
        match self {
            Self::Password => "Requires connection: host + password.",
            Self::Session => "Requires connection: host + password + cashier + pin.",
        }
    }
}

/// Produces a type's schema (a `$ref` into `components/schemas`) and registers it in the generator.
type SchemaFn = fn(&mut SchemaGenerator) -> Schema;

/// One documented `POST /v1/<op>` operation.
struct OperationDef {
    /// Route path.
    path: &'static str,
    /// Stable `operationId` (camelCase) used by code generators.
    op_id: &'static str,
    /// One-line summary.
    summary: &'static str,
    /// Fuller, user-facing description: what the operation does, key inputs, and common gotchas.
    /// The connection-tier note is appended automatically, so it must not be repeated here.
    description: &'static str,
    /// Connection tier required.
    conn: Conn,
    /// Params schema generator, or `None` for parameterless operations.
    params: Option<SchemaFn>,
    /// Success (`200`) response schema generator.
    response: SchemaFn,
    /// Example request body (the full `{ params: ... }` envelope) as a JSON string. `Some` iff the
    /// operation takes params; the `examples_match_params` test enforces that invariant.
    req_example: Option<&'static str>,
    /// Example `200` response body as a JSON string.
    resp_example: Option<&'static str>,
}

/// Every operation the protected router exposes, in route order. Mirrors `routes::app`; the
/// `documents_every_route` test asserts this list and the live router stay in lockstep.
const OPERATIONS: &[OperationDef] = &[
    OperationDef {
        path: "/v1/operators",
        op_id: "operators",
        summary: "List the device's operators and departments.",
        description: "Returns every operator (cashier) and department registered on the device, \
            including which departments each operator may use. Call it first to discover the valid \
            `cashier` ids and department numbers you will pass to other operations. Read-only.",
        conn: Conn::Password,
        params: None,
        response: SchemaGenerator::subschema_for::<ListOpsAndDepsResponse>,
        req_example: None,
        resp_example: Some(
            r#"{"c":[{"id":3,"name":"Cashier 1","deps":[1,2]}],"d":[{"id":1,"name":"Main","type":1}]}"#,
        ),
    },
    OperationDef {
        path: "/v1/login",
        op_id: "login",
        summary: "Check operator login credentials.",
        description: "Verifies that the supplied cashier + PIN can open an operator session, \
            without printing anything or changing device state. Use it to validate credentials \
            (e.g. at the start of a shift). Every other operation opens its own session, so this \
            call is not a prerequisite for them.",
        conn: Conn::Session,
        params: None,
        response: SchemaGenerator::subschema_for::<StatusOk>,
        req_example: None,
        resp_example: Some(r#"{"ok":true}"#),
    },
    OperationDef {
        path: "/v1/receipt",
        op_id: "printReceipt",
        summary: "Print a fiscal sale receipt.",
        description: "Prints and fiscalises a sale. Set `mode` to 1 (simple lump-sum, uses `dep`), \
            2 (itemised — supply `items`), or 3 (prepayment). Split the total across `paidAmount` \
            (cash), `paidAmountCard` (card), `partialAmount`, and `prePaymentAmount`. Keep the \
            response's `rseq` — it is the sale's sequence number, and returns key on it, not on the \
            printed `fiscal` number.",
        conn: Conn::Session,
        params: Some(SchemaGenerator::subschema_for::<PrintReceiptRequest>),
        response: SchemaGenerator::subschema_for::<ReceiptResponse>,
        req_example: Some(
            r#"{"params":{"mode":2,"paidAmount":40,"paidAmountCard":0,"partialAmount":0,"prePaymentAmount":0,"useExtPOS":false,"items":[{"dep":1,"adgCode":"56.10","productCode":"0001","productName":"Coffee","qty":1,"unit":"pcs","price":40}]}}"#,
        ),
        resp_example: Some(
            r#"{"rseq":232,"crn":"51815332","sn":"NLS12345678","tin":"02601234","taxpayer":"Example Trade LLC","address":"12 Abovyan St, Yerevan","time":1710000000000,"fiscal":"20000123","total":40,"change":0,"lottery":"","prize":0,"verificationNumber":"A1B2C3D4"}"#,
        ),
    },
    OperationDef {
        path: "/v1/receipt/last",
        op_id: "printLastReceipt",
        summary: "Reprint a copy of the last receipt.",
        description: "Reprints a copy of the operator's most recent receipt. The copy is marked as \
            a duplicate and carries no new fiscal value.",
        conn: Conn::Session,
        params: None,
        response: SchemaGenerator::subschema_for::<EmptyResponse>,
        req_example: None,
        resp_example: Some("{}"),
    },
    OperationDef {
        path: "/v1/receipt/lookup",
        op_id: "lookupReceipt",
        summary: "Look up a receipt before returning it (read-only).",
        description: "Fetches an earlier sale's contents so you can confirm it is returnable before \
            calling `printReturn`. `receiptId` is the sale's sequence number (`rseq` from the print \
            response); `crn` is the registration number of the device that printed it. Success means \
            the receipt can be returned; a device error (155/156/174/185) means it is not yet \
            returnable — usually the post-sale sync with the tax authority is still pending, so run \
            `timeSync` and retry.",
        conn: Conn::Session,
        params: Some(SchemaGenerator::subschema_for::<GetReturnableReceiptRequest>),
        response: SchemaGenerator::subschema_for::<ReturnableReceiptResponse>,
        req_example: Some(r#"{"params":{"crn":"51815332","receiptId":"232"}}"#),
        resp_example: Some(
            r#"{"rseq":232,"cid":3,"saleType":0,"subType":2,"ta":40,"cash":0,"card":40,"ppa":0,"ppu":0,"pTin":"","eMarks":[],"totals":[{"gc":"0001","gn":"Coffee","qty":1,"p":40,"mu":"pcs","rpid":0,"t":33.33,"tt":40}]}"#,
        ),
    },
    OperationDef {
        path: "/v1/return",
        op_id: "printReturn",
        summary: "Print a return (refund) receipt.",
        description: "Registers a refund against an earlier sale. `returnTicketId` is the original \
            sale's sequence number (`rseq`), NOT the printed fiscal number — passing the fiscal \
            number yields device error 174. Omit the `*ForReturn` amounts and `returnItemList` for a \
            full return; set them for a partial one. If the device reports 174/185, the sale has not \
            finished syncing with the tax authority (or the terminal is showing a modal): run \
            `timeSync`, clear the terminal, and retry.",
        conn: Conn::Session,
        params: Some(SchemaGenerator::subschema_for::<PrintReturnReceiptRequest>),
        response: SchemaGenerator::subschema_for::<ReturnReceiptResponse>,
        req_example: Some(r#"{"params":{"crn":"51815332","returnTicketId":232}}"#),
        resp_example: Some(
            r#"{"rseq":233,"crn":"51815332","sn":"NLS12345678","tin":"02601234","taxpayer":"Example Trade LLC","address":"12 Abovyan St, Yerevan","time":1710000500000,"rtime":1710000500000,"fiscal":"20000124","total":40,"change":0,"lottery":"","prize":0,"verificationNumber":"E5F6G7H8"}"#,
        ),
    },
    OperationDef {
        path: "/v1/report",
        op_id: "report",
        summary: "Print an X or Z fiscal report.",
        description: "Prints a fiscal report over a time range. `reportType` 1 is an X-report (an \
            interim summary that leaves counters untouched); 2 is a Z-report (end-of-day close that \
            zeros counters and finalises the fiscal day). Optionally restrict to a single \
            department, cashier, or transaction type. `startDate`/`endDate` are epoch-style integers \
            as in the spec.",
        conn: Conn::Session,
        params: Some(SchemaGenerator::subschema_for::<FiscalReportRequest>),
        response: SchemaGenerator::subschema_for::<EmptyResponse>,
        req_example: Some(r#"{"params":{"reportType":2,"startDate":123231324,"endDate":123271324}}"#),
        resp_example: Some("{}"),
    },
    OperationDef {
        path: "/v1/cash",
        op_id: "cashInOut",
        summary: "Register a cash-drawer deposit or withdrawal.",
        description: "Records money moving in or out of the cash drawer outside of a sale (e.g. a \
            starting float or a payout). Set `isCashIn` true for a deposit, false for a withdrawal; \
            `amount` must be greater than zero.",
        conn: Conn::Session,
        params: Some(SchemaGenerator::subschema_for::<CashInOutRequest>),
        response: SchemaGenerator::subschema_for::<EmptyResponse>,
        req_example: Some(
            r#"{"params":{"isCashIn":true,"amount":5000,"cashierId":3,"description":"Opening float"}}"#,
        ),
        resp_example: Some("{}"),
    },
    OperationDef {
        path: "/v1/datetime",
        op_id: "dateTime",
        summary: "Read the device date and time.",
        description: "Returns the device's current date and time as an opaque string (the spec does \
            not pin a format). Handy as a quick clock/liveness check against a live session.",
        conn: Conn::Session,
        params: None,
        response: SchemaGenerator::subschema_for::<DateTimeResponse>,
        req_example: None,
        resp_example: Some(r#"{"dt":"2026-07-17 20:14:03"}"#),
    },
    OperationDef {
        path: "/v1/time-sync",
        op_id: "timeSync",
        summary: "Synchronize the device with the tax authority.",
        description: "Runs the HDM's synchronisation with the tax authority (spec op 14 — clock plus \
            pending fiscal state). Run it when the device reports a sync-required error (155/156) or \
            rejects a return as not-yet-returnable (174/185): it uploads outstanding data so those \
            operations can proceed. Harmless to call at any time.",
        conn: Conn::Session,
        params: None,
        response: SchemaGenerator::subschema_for::<EmptyResponse>,
        req_example: None,
        resp_example: Some("{}"),
    },
    OperationDef {
        path: "/v1/payment-systems",
        op_id: "paymentSystems",
        summary: "List the payment systems configured on the device.",
        description: "Returns the payment-system code-to-name mapping configured on the device \
            (1 = card, 10-18 = various Armenian wallets). Call it once at startup to learn which \
            `PaymentSystem` codes are valid for `printReceipt`.",
        conn: Conn::Session,
        params: None,
        response: SchemaGenerator::subschema_for::<PaymentSystemsListResponse>,
        req_example: None,
        resp_example: Some(
            r#"{"PaymentSystems":[{"code":1,"name":"Card"},{"code":11,"name":"Telcell"}]}"#,
        ),
    },
    OperationDef {
        path: "/v1/emark",
        op_id: "emark",
        summary: "Validate a single eMark traceability code.",
        description: "Checks one eMark (product traceability) code with the device without printing \
            anything. The code is 29-110 ASCII-printable characters. Use it to pre-validate a \
            scanned mark before adding it to a receipt.",
        conn: Conn::Session,
        params: Some(SchemaGenerator::subschema_for::<SingleEmarkRequest>),
        response: SchemaGenerator::subschema_for::<EmptyResponse>,
        req_example: Some(r#"{"params":{"eMark":"0104680000000000215abcDEfgHij12"}}"#),
        resp_example: Some("{}"),
    },
    OperationDef {
        path: "/v1/sample",
        op_id: "receiptSample",
        summary: "Print a sample (test) receipt.",
        description: "Prints a non-fiscal sample receipt for checking paper, layout, and print \
            quality. It carries no fiscal value.",
        conn: Conn::Session,
        params: None,
        response: SchemaGenerator::subschema_for::<EmptyResponse>,
        req_example: None,
        resp_example: Some("{}"),
    },
    OperationDef {
        path: "/v1/header-footer",
        op_id: "headerFooter",
        summary: "Configure receipt header and footer lines.",
        description: "Sets the free-text header and footer lines printed on every receipt (e.g. shop \
            name, address, a thank-you). Lines print top-to-bottom in array order; send empty arrays \
            to clear them.",
        conn: Conn::Session,
        params: Some(SchemaGenerator::subschema_for::<SetupHeaderFooterRequest>),
        response: SchemaGenerator::subschema_for::<EmptyResponse>,
        req_example: Some(
            r#"{"params":{"headers":["Example Trade LLC","12 Abovyan St, Yerevan"],"footers":["Thank you for your purchase!"]}}"#,
        ),
        resp_example: Some("{}"),
    },
    OperationDef {
        path: "/v1/logo",
        op_id: "headerLogo",
        summary: "Upload the receipt header logo.",
        description: "Uploads the logo image printed at the top of receipts. `headerLogo` is the \
            image bytes as Base64; the device expects a BMP with colour depth <=4 bits. (The Base64 \
            is truncated in the example.)",
        conn: Conn::Session,
        params: Some(SchemaGenerator::subschema_for::<SetupHeaderLogoRequest>),
        response: SchemaGenerator::subschema_for::<EmptyResponse>,
        req_example: Some(r#"{"params":{"headerLogo":"Qk1GAAAAAAAAADYAAAAoAAAA..."}}"#),
        resp_example: Some("{}"),
    },
];

/// The paths of every protected operation. Used by the route-coverage test to assert the document
/// and the live router stay in lockstep.
#[cfg(test)]
pub(crate) fn operation_paths() -> Vec<&'static str> {
    OPERATIONS.iter().map(|op| op.path).collect()
}

const API_DESCRIPTION: &str = "\
Local HTTP bridge for the Armenian fiscal cash register (HDM) protocol. Every operation is a \
`POST /v1/<op>` carrying a uniform JSON envelope `{ connection?, params? }`: `connection` overrides \
the bridge's configured default device field-by-field, and `params` carries the operation input. \
On failure every route returns the same error envelope (see `ErrorBody`): `kind` is a stable \
machine tag and `code` carries the device/spec response code when the device rejected the request. \
All routes except `/v1/health`, `/v1/info`, and `/v1/openapi.json` require \
`Authorization: Bearer <token>`.";

/// Build the `OpenAPI` 3.1 document describing the bridge's HTTP surface for the given bridge version.
#[must_use]
pub fn document(version: &str) -> Value {
    let settings = SchemaSettings::draft2020_12().with(|s| {
        s.definitions_path = "/components/schemas".into();
        // Component schemas don't carry their own `$schema`; the document declares the dialect once.
        s.meta_schema = None;
    });
    let mut generator = SchemaGenerator::new(settings);

    // Register the shared component types up front and capture their `$ref` nodes.
    let partial_conn = generator.subschema_for::<PartialConn>().to_value();
    let error_ref = generator.subschema_for::<ErrorBody>().to_value();

    let mut paths = Map::new();
    for op in OPERATIONS {
        let params_ref = op.params.map(|f| f(&mut generator).to_value());
        let response_ref = (op.response)(&mut generator).to_value();
        paths.insert(
            op.path.to_owned(),
            operation_item(
                op,
                &partial_conn,
                params_ref.as_ref(),
                &response_ref,
                &error_ref,
            ),
        );
    }

    // Public meta endpoints.
    let info_ref = generator.subschema_for::<Info>().to_value();
    let health_ref = generator.subschema_for::<HealthOk>().to_value();
    paths.insert(
        "/v1/health".to_owned(),
        meta_get(
            "health",
            "Liveness probe (public, no auth).",
            &health_ref,
            Some(r#"{"status":"ok"}"#),
        ),
    );
    paths.insert(
        "/v1/info".to_owned(),
        meta_get(
            "info",
            "Bridge metadata and the operation list (public, no auth).",
            &info_ref,
            None,
        ),
    );
    paths.insert("/v1/openapi.json".to_owned(), openapi_self_item());

    let schemas = Value::Object(generator.take_definitions(true));

    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "HDM Bridge API",
            "description": API_DESCRIPTION,
            "version": version,
            "license": { "name": "MIT OR Apache-2.0" },
        },
        "servers": [
            { "url": "http://127.0.0.1:8077", "description": "Default local bridge bind address" }
        ],
        "paths": Value::Object(paths),
        "components": {
            "securitySchemes": {
                "bearerAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "description": "The shared bridge token (HDM_BRIDGE_TOKEN)."
                }
            },
            "schemas": schemas,
        },
    })
}

/// Build the `{ "post": { ... } }` path item for one protected operation.
fn operation_item(
    op: &OperationDef,
    partial_conn: &Value,
    params_ref: Option<&Value>,
    response_ref: &Value,
    error_ref: &Value,
) -> Value {
    // Request body: `{ connection?: PartialConn, params?: <typed> }`, no extra keys (mirrors the
    // handler's `deny_unknown_fields` envelope).
    let mut props = Map::new();
    props.insert("connection".to_owned(), partial_conn.clone());
    let mut body_schema = json!({ "type": "object", "additionalProperties": false });
    if let Some(params) = params_ref {
        props.insert("params".to_owned(), params.clone());
        body_schema["required"] = json!(["params"]);
    } else {
        // Parameterless operations ignore any `params` value but still accept the key.
        props.insert(
            "params".to_owned(),
            json!({ "description": "Ignored for this operation." }),
        );
    }
    body_schema["properties"] = Value::Object(props);
    let body_required = op.params.is_some();
    let description = format!("{}\n\n{}", op.description, op.conn.note());

    let mut request_media = json!({ "schema": body_schema });
    if let Some(example) = op.req_example {
        request_media["example"] = parse_example(example);
    }
    let mut response_media = json!({ "schema": response_ref.clone() });
    if let Some(example) = op.resp_example {
        response_media["example"] = parse_example(example);
    }

    json!({
        "post": {
            "operationId": op.op_id,
            "summary": op.summary,
            "description": description,
            "security": [{ "bearerAuth": [] }],
            "requestBody": {
                "required": body_required,
                "content": { "application/json": request_media },
            },
            "responses": {
                "200": {
                    "description": "Operation succeeded.",
                    "content": { "application/json": response_media },
                },
                "default": {
                    "description": "Error envelope (4xx/5xx).",
                    "content": { "application/json": { "schema": error_ref.clone() } },
                },
            },
        }
    })
}

/// Parse an authored example JSON string into a `Value`. The examples are compile-time constants in
/// `OPERATIONS`; the `examples_are_valid_json` test proves every one parses, so this never fails at
/// runtime.
fn parse_example(raw: &str) -> Value {
    serde_json::from_str(raw).expect("authored OpenAPI example must be valid JSON")
}

/// Build a public `GET` meta endpoint returning a single typed body.
fn meta_get(op_id: &str, summary: &str, response_ref: &Value, example: Option<&str>) -> Value {
    let mut response_media = json!({ "schema": response_ref.clone() });
    if let Some(example) = example {
        response_media["example"] = parse_example(example);
    }
    json!({
        "get": {
            "operationId": op_id,
            "summary": summary,
            "responses": {
                "200": {
                    "description": "Operation succeeded.",
                    "content": { "application/json": response_media },
                }
            }
        }
    })
}

/// The `GET /v1/openapi.json` path item — the document describes its own discovery endpoint.
fn openapi_self_item() -> Value {
    json!({
        "get": {
            "operationId": "openapiDocument",
            "summary": "This OpenAPI 3.1 document (public, no auth).",
            "responses": {
                "200": {
                    "description": "The OpenAPI document.",
                    "content": { "application/json": { "schema": { "type": "object" } } },
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guards `parse_example`'s `expect`: every authored example must be valid JSON, and each request
    /// example must carry the `{ params: ... }` envelope the handlers accept.
    #[test]
    fn examples_are_valid_json() {
        for op in OPERATIONS {
            if let Some(raw) = op.req_example {
                let value = parse_example(raw);
                assert!(
                    value.get("params").is_some(),
                    "{} request example must wrap its input in `params`",
                    op.op_id
                );
            }
            if let Some(raw) = op.resp_example {
                parse_example(raw);
            }
        }
    }

    /// A request example exists exactly when the operation takes params — otherwise the example and
    /// the schema would disagree about whether a body is meaningful.
    #[test]
    fn request_examples_track_params() {
        for op in OPERATIONS {
            assert_eq!(
                op.req_example.is_some(),
                op.params.is_some(),
                "{}: req_example must be Some iff the operation takes params",
                op.op_id
            );
        }
    }
}
