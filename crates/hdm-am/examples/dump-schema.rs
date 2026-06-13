//! Generate JSON Schema for every HDM wire request/response type into `docs/schema/`.
//!
//! The schemas are derived from this crate's own serde-annotated types, so they cannot drift from
//! the implementation as long as this is run.
//!
//! ```text
//! cargo run -p hdm-am --example dump-schema --features schema            # (re)write docs/schema/*.json
//! cargo run -p hdm-am --example dump-schema --features schema -- --check # verify they are up to date
//! ```
//!
//! `--check` exits non-zero if any committed schema differs from what the current types produce —
//! wire it into CI to guarantee the schemas stay in sync with the code.

use std::error::Error;
use std::path::PathBuf;
use std::process::ExitCode;

use hdm_am::{
    CashInOutRequest, DateTimeRequest, DateTimeResponse, EmptyResponse, FiscalReportRequest,
    GetReturnableReceiptRequest, HdmTimeSyncRequest, ListOpsAndDepsRequest, ListOpsAndDepsResponse,
    OperatorLoginRequest, OperatorLoginResponse, OperatorLogoutRequest, PaymentSystemsListRequest,
    PaymentSystemsListResponse, PrintLastReceiptRequest, PrintReceiptRequest,
    PrintReturnReceiptRequest, ReceiptResponse, ReceiptSampleRequest, ReturnReceiptResponse,
    ReturnableReceiptResponse, SetupHeaderFooterRequest, SetupHeaderLogoRequest,
    SingleEmarkRequest,
};

/// Build `(type name, generated schema)` pairs for every root request/response type.
macro_rules! schemas {
    ($($t:ty),+ $(,)?) => {
        vec![ $( (stringify!($t), schemars::schema_for!($t)) ),+ ]
    };
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE, // --check found drift
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Returns `Ok(true)` on success, `Ok(false)` if `--check` found a stale file.
fn run() -> Result<bool, Box<dyn Error>> {
    let check = std::env::args().any(|arg| arg == "--check");
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema");
    if !check {
        std::fs::create_dir_all(&dir)?;
    }

    let schemas = schemas![
        ListOpsAndDepsRequest,
        ListOpsAndDepsResponse,
        OperatorLoginRequest,
        OperatorLoginResponse,
        OperatorLogoutRequest,
        PrintReceiptRequest,
        ReceiptResponse,
        PrintLastReceiptRequest,
        GetReturnableReceiptRequest,
        ReturnableReceiptResponse,
        SetupHeaderFooterRequest,
        SetupHeaderLogoRequest,
        FiscalReportRequest,
        PrintReturnReceiptRequest,
        ReturnReceiptResponse,
        CashInOutRequest,
        DateTimeRequest,
        DateTimeResponse,
        ReceiptSampleRequest,
        HdmTimeSyncRequest,
        PaymentSystemsListRequest,
        PaymentSystemsListResponse,
        SingleEmarkRequest,
        EmptyResponse,
    ];

    let mut up_to_date = true;
    for (name, schema) in schemas {
        let mut json = serde_json::to_string_pretty(&schema)?;
        json.push('\n');
        let path = dir.join(format!("{name}.json"));
        if check {
            let current = std::fs::read_to_string(&path).unwrap_or_default();
            if current != json {
                eprintln!("drift: {name}.json");
                up_to_date = false;
            }
        } else {
            std::fs::write(&path, json)?;
            println!("wrote {name}.json");
        }
    }

    if check {
        println!(
            "{}",
            if up_to_date {
                "schemas up to date"
            } else {
                "schemas OUT OF DATE — run `cargo run -p hdm-am --example dump-schema --features schema`"
            }
        );
    }
    Ok(up_to_date)
}
