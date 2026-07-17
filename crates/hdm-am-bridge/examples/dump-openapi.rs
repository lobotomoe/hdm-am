//! Generate the bridge's `OpenAPI` 3.1 document into `docs/openapi.json`.
//!
//! The document is assembled from the bridge's own route table and the wire types'
//! `schemars`-derived schemas, so it cannot drift from the implementation as long as this is run.
//!
//! ```text
//! cargo run -p hdm-am-bridge --example dump-openapi --features schema            # (re)write docs/openapi.json
//! cargo run -p hdm-am-bridge --example dump-openapi --features schema -- --check # verify it is up to date
//! ```
//!
//! `--check` exits non-zero if the committed document differs from what the current code produces —
//! wire it into CI to guarantee the document stays in sync.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let check = std::env::args().any(|arg| arg == "--check");
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/openapi.json");

    let document = match hdm_am_bridge::openapi::document(env!("CARGO_PKG_VERSION")) {
        Ok(document) => document,
        Err(err) => {
            eprintln!("error building OpenAPI document: {err}");
            return ExitCode::FAILURE;
        }
    };
    let mut json = match serde_json::to_string_pretty(&document) {
        Ok(json) => json,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };
    json.push('\n');

    if check {
        let current = std::fs::read_to_string(&path).unwrap_or_default();
        if current == json {
            println!("openapi.json up to date");
            ExitCode::SUCCESS
        } else {
            eprintln!(
                "drift: docs/openapi.json is OUT OF DATE — run `cargo run -p hdm-am-bridge --example dump-openapi --features schema`"
            );
            ExitCode::FAILURE
        }
    } else {
        match std::fs::write(&path, json) {
            Ok(()) => {
                println!("wrote {}", path.display());
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("error: {err}");
                ExitCode::FAILURE
            }
        }
    }
}
