//! `hdm` — command-line tool for talking to an Armenian fiscal cash register (HDM) over TCP.
//!
//! All protocol logic lives in the `hdm-am` core crate; this binary is a thin shell that maps
//! subcommands onto core operations, handles connection setup, and renders responses for a human.
//!
//! Connection parameters can be supplied via flags or environment variables
//! (`HDM_HOST`, `HDM_PORT`, `HDM_PASSWORD`, `HDM_CASHIER`, `HDM_PIN`). The device exposes these on
//! its "integration with external programs" settings screen.

#![warn(missing_docs)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod bridge;
mod conn;
mod format;
mod run;

use clap::{Parser, Subcommand};
use rust_decimal::Decimal;

/// Connection and authentication parameters shared by every subcommand.
#[derive(Debug, Parser)]
#[command(
    name = "hdm",
    about = "Talk to an Armenian fiscal cash register (HDM) over TCP",
    version,
    propagate_version = true
)]
pub struct Cli {
    /// HDM IP address (the "ՀԴՄ IP հասցե" field on the device).
    #[arg(long, env = "HDM_HOST", global = true)]
    pub host: Option<String>,

    /// HDM TCP port (the "Պորտ" field; commonly 1025).
    #[arg(long, env = "HDM_PORT", default_value_t = 1025, global = true)]
    pub port: u16,

    /// HDM access password (the "ՀԴՄ գաղտնաբառ" field).
    #[arg(long, env = "HDM_PASSWORD", global = true)]
    pub password: Option<String>,

    /// Operator (cashier) numeric ID — required by session operations.
    #[arg(long, env = "HDM_CASHIER", global = true)]
    pub cashier: Option<u32>,

    /// Operator PIN — required by session operations.
    #[arg(long, env = "HDM_PIN", global = true)]
    pub pin: Option<String>,

    /// Socket read/write/connect timeout, in seconds (1-50). The spec caps response wait at 50s.
    #[arg(
        long,
        default_value_t = 50,
        value_parser = clap::value_parser!(u64).range(1..=50),
        global = true
    )]
    pub timeout: u64,

    /// Emit machine-readable JSON instead of formatted text.
    #[arg(long, global = true)]
    pub json: bool,

    /// Increase log verbosity (`-v` = debug, `-vv` = trace). Logs go to stderr.
    ///
    /// `-vv` traces the decrypted payloads, which include your own fiscal data (taxpayer/partner
    /// TIN, eMark codes). The session key is always redacted. Use it for debugging, not in
    /// unattended/production logging.
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// The operation to perform.
    #[command(subcommand)]
    pub command: Command,
}

/// One subcommand per HDM operation (plus a transport-only connectivity check).
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Check whether an endpoint speaks the HDM protocol, without logging in. Sends one
    /// unauthenticated probe and reports the protocol/software version (or that it is not an HDM).
    Probe,

    /// List operators and departments registered on the device. No login required.
    Operators,

    /// Verify credentials: log in then immediately log out.
    Login,

    /// Query the device's current date and time.
    Datetime,

    /// List the payment systems configured on the device.
    PaymentSystems,

    /// Print a non-fiscal sample receipt for layout verification. Consumes paper.
    Sample,

    /// Print a fiscal receipt. IRREVERSIBLE — registers a sale with the tax authority.
    Receipt(ReceiptArgs),

    /// Reprint a copy of the operator's most recent receipt.
    PrintLast,

    /// Print an X (interim) or Z (end-of-day) fiscal report.
    Report(ReportArgs),

    /// Record a cash-drawer in/out adjustment. IRREVERSIBLE.
    Cash(CashArgs),

    /// Synchronise the device with the tax authority's clock/state.
    TimeSync,

    /// Submit a single eMark traceability code.
    Emark {
        /// The eMark code (29-110 ASCII-printable chars, see spec section 4.9).
        code: String,
    },

    /// Look up a prior receipt's contents by number. Read-only — registers nothing.
    /// Use this to inspect a receipt before returning it with `return`.
    LookupReceipt(LookupReceiptArgs),

    /// Print a return/refund receipt for a prior receipt: full, by-amount, or per-item.
    /// IRREVERSIBLE — registers a refund.
    Return(ReturnArgs),

    /// Configure the header/footer lines printed on every receipt. Persists on the device.
    HeaderFooter(HeaderFooterArgs),

    /// Upload the receipt header logo bitmap. Persists on the device.
    Logo(LogoArgs),

    /// Manage the local HTTP bridge that exposes this device to a browser: start/stop/status as a
    /// background process, or `run` in the foreground.
    Bridge(BridgeArgs),
}

/// Arguments for the `bridge` command.
#[derive(Debug, clap::Args)]
pub struct BridgeArgs {
    /// What to do with the bridge.
    #[command(subcommand)]
    pub action: BridgeAction,
}

/// Lifecycle actions for the local HTTP bridge.
#[derive(Debug, Subcommand)]
pub enum BridgeAction {
    /// Start the bridge as a background process and return immediately.
    Start(BridgeRunArgs),
    /// Run the bridge in the foreground until terminated (for debugging or a service manager).
    Run(BridgeRunArgs),
    /// Stop the background bridge.
    Stop,
    /// Report whether the background bridge is running.
    Status,
}

/// Bridge server settings forwarded to the `hdm-bridge` process. The device connection comes from
/// the global `--host`/`--password`/`--cashier`/`--pin` flags (or the `HDM_*` env vars).
#[derive(Debug, clap::Args)]
pub struct BridgeRunArgs {
    /// Address to listen on (loopback only in production; defaults to 127.0.0.1:8077).
    #[arg(long, env = "HDM_BRIDGE_BIND")]
    pub bind: Option<String>,
    /// Bearer token required on every route except `/v1/health`.
    #[arg(long, env = "HDM_BRIDGE_TOKEN")]
    pub token: Option<String>,
    /// Start without a token (loopback development only — leaves the device unprotected).
    #[arg(long)]
    pub insecure_no_auth: bool,
    /// Allowed CORS origin (repeatable; env is comma-separated).
    #[arg(
        long = "allow-origin",
        env = "HDM_BRIDGE_ALLOW_ORIGIN",
        value_delimiter = ','
    )]
    pub allow_origins: Vec<String>,
}

/// Arguments for the `lookup-receipt` command (op 6).
#[derive(Debug, clap::Args)]
pub struct LookupReceiptArgs {
    /// Number of the receipt to look up (the `receiptId` field).
    #[arg(long)]
    pub receipt_id: String,
    /// HDM registration number (`crn`) of the device that printed the original receipt.
    #[arg(long)]
    pub crn: String,
}

/// Arguments for the `return` command (op 10).
#[derive(Debug, clap::Args)]
pub struct ReturnArgs {
    /// HDM registration number (`crn`).
    #[arg(long)]
    pub crn: String,
    /// Number of the receipt to return (the `returnTicketId` field).
    #[arg(long)]
    pub ticket: u64,
    /// Cash amount to return (partial-payment return case only).
    #[arg(long)]
    pub cash: Option<Decimal>,
    /// Card amount to return (partial-payment return case only).
    #[arg(long)]
    pub card: Option<Decimal>,
    /// Prepayment amount to return (partial-payment return case only).
    #[arg(long)]
    pub prepayment: Option<Decimal>,
    /// Acquirer RRN (12 chars).
    #[arg(long)]
    pub rrn: Option<String>,
    /// Payment terminal ID (8 chars).
    #[arg(long)]
    pub terminal_id: Option<String>,
    /// eMark code for marked goods being returned. May be passed more than once.
    #[arg(long = "emark")]
    pub e_marks: Vec<String>,
    /// JSON file holding an array of per-item returns: `[{"rpid":123,"quantity":1}]`.
    #[arg(long)]
    pub return_items: Option<std::path::PathBuf>,
    /// Skip the confirmation prompt.
    #[arg(long)]
    pub yes: bool,
}

/// Arguments for the `header-footer` command (op 7).
#[derive(Debug, clap::Args)]
pub struct HeaderFooterArgs {
    /// JSON file: `{ "headers": [ {"align":2,"bold":true,"fsize":2,"text":"..."} ], "footers": [...] }`.
    #[arg(long)]
    pub file: std::path::PathBuf,
}

/// Arguments for the `logo` command (op 8).
#[derive(Debug, clap::Args)]
pub struct LogoArgs {
    /// Path to the logo image file; its raw bytes are Base64-encoded and uploaded. Must be a BMP
    /// with colour depth ≤4 bits (≤16 colours; 1-bit monochrome works).
    #[arg(long)]
    pub image: std::path::PathBuf,
}

/// Receipt mode, mapped to `hdm_am::PrintMode`.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ReceiptMode {
    /// Lump-sum receipt; `items` unused, `dep` required.
    Simple,
    /// Itemised receipt; requires `--items`.
    Products,
    /// Prepayment receipt; `items` must be empty.
    Prepayment,
}

/// Fiscal-report kind, mapped to `hdm_am::FiscalReportKind`.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ReportKind {
    /// Interim summary; does not zero counters.
    X,
    /// End-of-day; zeros counters and finalises the fiscal day.
    Z,
}

/// Cash-drawer adjustment direction.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum CashDirection {
    /// Deposit into the drawer.
    In,
    /// Withdrawal from the drawer.
    Out,
}

/// Arguments for the `receipt` command (op 4).
#[derive(Debug, clap::Args)]
pub struct ReceiptArgs {
    /// Receipt mode.
    #[arg(long, value_enum, default_value_t = ReceiptMode::Simple)]
    pub mode: ReceiptMode,
    /// Cash amount paid.
    #[arg(long, default_value_t = Decimal::ZERO)]
    pub cash: Decimal,
    /// Card (cashless) amount paid.
    #[arg(long, default_value_t = Decimal::ZERO)]
    pub card: Decimal,
    /// Partial-payment amount.
    #[arg(long, default_value_t = Decimal::ZERO)]
    pub partial: Decimal,
    /// Prepayment-used amount.
    #[arg(long, default_value_t = Decimal::ZERO)]
    pub prepayment: Decimal,
    /// Department (required for simple/prepayment receipts).
    #[arg(long)]
    pub dep: Option<u32>,
    /// Partner TIN (8 digits) for a B2B receipt.
    #[arg(long)]
    pub partner_tin: Option<String>,
    /// Payment-system code (from `payment-systems`) when not using an external POS.
    #[arg(long)]
    pub payment_system: Option<u32>,
    /// Use an external POS terminal; requires `--rrn` and `--terminal-id`.
    #[arg(long)]
    pub use_ext_pos: bool,
    /// Acquirer RRN (12 chars) when `--use-ext-pos` is set.
    #[arg(long)]
    pub rrn: Option<String>,
    /// Payment terminal ID (8 chars) when `--use-ext-pos` is set.
    #[arg(long)]
    pub terminal_id: Option<String>,
    /// eMark traceability code for marked goods. May be passed more than once.
    #[arg(long = "emark")]
    pub e_marks: Vec<String>,
    /// Path to a JSON file holding an array of receipt items (required for `--mode products`).
    #[arg(long)]
    pub items: Option<std::path::PathBuf>,
    /// Skip the confirmation prompt.
    #[arg(long)]
    pub yes: bool,
}

/// Arguments for the `report` command (op 9).
#[derive(Debug, clap::Args)]
pub struct ReportArgs {
    /// Report kind: `x` (interim) or `z` (end-of-day).
    #[arg(long, value_enum)]
    pub kind: ReportKind,
    /// Department filter (at most one of --dept / --cashier-id may be set).
    #[arg(long)]
    pub dept: Option<u32>,
    /// Cashier filter (at most one of --dept / --cashier-id may be set).
    #[arg(long)]
    pub cashier_id: Option<u32>,
    /// Transaction-type filter (at most one report filter may be set).
    #[arg(long)]
    pub transaction_type: Option<u32>,
    /// Start of the report range (epoch-style integer per spec).
    #[arg(long, default_value_t = 0)]
    pub start: i64,
    /// End of the report range (epoch-style integer per spec).
    #[arg(long, default_value_t = 0)]
    pub end: i64,
    /// Skip the confirmation prompt (Z-report only).
    #[arg(long)]
    pub yes: bool,
}

/// Arguments for the `cash` command (op 11).
#[derive(Debug, clap::Args)]
pub struct CashArgs {
    /// Adjustment direction.
    #[arg(long, value_enum)]
    pub direction: CashDirection,
    /// Amount; must be greater than zero.
    #[arg(long)]
    pub amount: Decimal,
    /// Free-text description.
    #[arg(long, default_value = "")]
    pub description: String,
    /// Cashier number to send in the cash operation; defaults to global `--cashier`.
    #[arg(long)]
    pub cashier_id: Option<u32>,
    /// Skip the confirmation prompt.
    #[arg(long)]
    pub yes: bool,
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    match run::dispatch(&cli) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            // `{:#}` prints the full anyhow context chain on one line.
            eprintln!("error: {err:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// Map `-v` count to a `RUST_LOG` filter unless the user already set one explicitly.
fn init_logging(verbose: u8) {
    let default_level = match verbose {
        0 => "warn",
        1 => "info,hdm_am=debug",
        _ => "debug,hdm_am=trace",
    };
    let env = env_logger::Env::default().default_filter_or(default_level);
    env_logger::Builder::from_env(env)
        .format_timestamp_millis()
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// clap's own consistency check — catches duplicate flags, bad defaults, etc. at test time.
    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
