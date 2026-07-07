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
    /// Connection tier required.
    conn: Conn,
    /// Params schema generator, or `None` for parameterless operations.
    params: Option<SchemaFn>,
    /// Success (`200`) response schema generator.
    response: SchemaFn,
}

/// Every operation the protected router exposes, in route order. Mirrors `routes::app`; the
/// `documents_every_route` test asserts this list and the live router stay in lockstep.
const OPERATIONS: &[OperationDef] = &[
    OperationDef {
        path: "/v1/operators",
        op_id: "operators",
        summary: "List the HDM's operators and departments.",
        conn: Conn::Password,
        params: None,
        response: SchemaGenerator::subschema_for::<ListOpsAndDepsResponse>,
    },
    OperationDef {
        path: "/v1/login",
        op_id: "login",
        summary: "Verify operator login credentials.",
        conn: Conn::Session,
        params: None,
        response: SchemaGenerator::subschema_for::<StatusOk>,
    },
    OperationDef {
        path: "/v1/receipt",
        op_id: "printReceipt",
        summary: "Print a fiscal receipt.",
        conn: Conn::Session,
        params: Some(SchemaGenerator::subschema_for::<PrintReceiptRequest>),
        response: SchemaGenerator::subschema_for::<ReceiptResponse>,
    },
    OperationDef {
        path: "/v1/receipt/last",
        op_id: "printLastReceipt",
        summary: "Print a copy of the last receipt.",
        conn: Conn::Session,
        params: None,
        response: SchemaGenerator::subschema_for::<EmptyResponse>,
    },
    OperationDef {
        path: "/v1/receipt/lookup",
        op_id: "lookupReceipt",
        summary: "Look up a returnable receipt's contents (read-only).",
        conn: Conn::Session,
        params: Some(SchemaGenerator::subschema_for::<GetReturnableReceiptRequest>),
        response: SchemaGenerator::subschema_for::<ReturnableReceiptResponse>,
    },
    OperationDef {
        path: "/v1/return",
        op_id: "printReturn",
        summary: "Print a return receipt.",
        conn: Conn::Session,
        params: Some(SchemaGenerator::subschema_for::<PrintReturnReceiptRequest>),
        response: SchemaGenerator::subschema_for::<ReturnReceiptResponse>,
    },
    OperationDef {
        path: "/v1/report",
        op_id: "report",
        summary: "Print an X or Z fiscal report.",
        conn: Conn::Session,
        params: Some(SchemaGenerator::subschema_for::<FiscalReportRequest>),
        response: SchemaGenerator::subschema_for::<EmptyResponse>,
    },
    OperationDef {
        path: "/v1/cash",
        op_id: "cashInOut",
        summary: "Register a cash-drawer in or out.",
        conn: Conn::Session,
        params: Some(SchemaGenerator::subschema_for::<CashInOutRequest>),
        response: SchemaGenerator::subschema_for::<EmptyResponse>,
    },
    OperationDef {
        path: "/v1/datetime",
        op_id: "dateTime",
        summary: "Get the device date and time.",
        conn: Conn::Session,
        params: None,
        response: SchemaGenerator::subschema_for::<DateTimeResponse>,
    },
    OperationDef {
        path: "/v1/time-sync",
        op_id: "timeSync",
        summary: "Synchronize the device clock.",
        conn: Conn::Session,
        params: None,
        response: SchemaGenerator::subschema_for::<EmptyResponse>,
    },
    OperationDef {
        path: "/v1/payment-systems",
        op_id: "paymentSystems",
        summary: "List payment systems configured on the device.",
        conn: Conn::Session,
        params: None,
        response: SchemaGenerator::subschema_for::<PaymentSystemsListResponse>,
    },
    OperationDef {
        path: "/v1/emark",
        op_id: "emark",
        summary: "Validate a single eMark code.",
        conn: Conn::Session,
        params: Some(SchemaGenerator::subschema_for::<SingleEmarkRequest>),
        response: SchemaGenerator::subschema_for::<EmptyResponse>,
    },
    OperationDef {
        path: "/v1/sample",
        op_id: "receiptSample",
        summary: "Print a sample receipt.",
        conn: Conn::Session,
        params: None,
        response: SchemaGenerator::subschema_for::<EmptyResponse>,
    },
    OperationDef {
        path: "/v1/header-footer",
        op_id: "headerFooter",
        summary: "Configure receipt header and footer lines.",
        conn: Conn::Session,
        params: Some(SchemaGenerator::subschema_for::<SetupHeaderFooterRequest>),
        response: SchemaGenerator::subschema_for::<EmptyResponse>,
    },
    OperationDef {
        path: "/v1/logo",
        op_id: "headerLogo",
        summary: "Configure the receipt header logo.",
        conn: Conn::Session,
        params: Some(SchemaGenerator::subschema_for::<SetupHeaderLogoRequest>),
        response: SchemaGenerator::subschema_for::<EmptyResponse>,
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
        meta_get("health", "Liveness probe (public, no auth).", &health_ref),
    );
    paths.insert(
        "/v1/info".to_owned(),
        meta_get(
            "info",
            "Bridge metadata and the operation list (public, no auth).",
            &info_ref,
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
    let description = format!("{} {}", op.summary, op.conn.note());

    json!({
        "post": {
            "operationId": op.op_id,
            "summary": op.summary,
            "description": description,
            "security": [{ "bearerAuth": [] }],
            "requestBody": {
                "required": body_required,
                "content": { "application/json": { "schema": body_schema } },
            },
            "responses": {
                "200": {
                    "description": "Operation succeeded.",
                    "content": { "application/json": { "schema": response_ref.clone() } },
                },
                "default": {
                    "description": "Error envelope (4xx/5xx).",
                    "content": { "application/json": { "schema": error_ref.clone() } },
                },
            },
        }
    })
}

/// Build a public `GET` meta endpoint returning a single typed body.
fn meta_get(op_id: &str, summary: &str, response_ref: &Value) -> Value {
    json!({
        "get": {
            "operationId": op_id,
            "summary": summary,
            "responses": {
                "200": {
                    "description": "Operation succeeded.",
                    "content": { "application/json": { "schema": response_ref.clone() } },
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
