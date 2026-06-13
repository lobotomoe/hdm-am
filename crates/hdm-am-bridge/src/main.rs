//! `hdm-bridge` — the standalone binary entrypoint.
//!
//! Parses configuration from flags and `HDM_*` / `HDM_BRIDGE_*` environment variables, then runs
//! the server from [`hdm_am_bridge::serve`].

use std::net::SocketAddr;

use clap::Parser;
use hdm_am_bridge::{BridgeConfig, PartialConn, serve};

/// Default listen address: loopback only.
const DEFAULT_BIND: &str = "127.0.0.1:8077";

#[derive(Parser, Debug)]
#[command(
    name = "hdm-bridge",
    about = "Local HTTP bridge exposing an Armenian fiscal cash register (HDM) to the browser",
    version
)]
struct Cli {
    /// Address to listen on (loopback only in production).
    #[arg(long, env = "HDM_BRIDGE_BIND", default_value = DEFAULT_BIND)]
    bind: SocketAddr,

    /// Shared bearer token required on every route except `/v1/health`.
    #[arg(long, env = "HDM_BRIDGE_TOKEN")]
    token: Option<String>,

    /// Start without a token. Loopback development only — leaves the device unprotected.
    #[arg(long)]
    insecure_no_auth: bool,

    /// Allowed CORS origin (repeatable; env is comma-separated). Required for browser callers.
    #[arg(
        long = "allow-origin",
        env = "HDM_BRIDGE_ALLOW_ORIGIN",
        value_delimiter = ','
    )]
    allow_origins: Vec<String>,

    /// Default HDM host (IP or name). Overridable per request.
    #[arg(long, env = "HDM_HOST")]
    host: Option<String>,

    /// Default HDM TCP port.
    #[arg(long, env = "HDM_PORT")]
    port: Option<u16>,

    /// Default HDM access password.
    #[arg(long, env = "HDM_PASSWORD")]
    password: Option<String>,

    /// Default operator (cashier) id.
    #[arg(long, env = "HDM_CASHIER")]
    cashier: Option<u32>,

    /// Default operator PIN.
    #[arg(long, env = "HDM_PIN")]
    pin: Option<String>,

    /// Default socket timeout in seconds (clamped to the spec's 50s cap).
    #[arg(long, env = "HDM_TIMEOUT")]
    timeout: Option<u64>,
}

impl Cli {
    fn into_config(self) -> BridgeConfig {
        BridgeConfig {
            bind: self.bind,
            token: self.token,
            insecure_no_auth: self.insecure_no_auth,
            allow_origins: self.allow_origins,
            default_conn: PartialConn {
                host: self.host,
                port: self.port,
                password: self.password,
                cashier: self.cashier,
                pin: self.pin,
                timeout_secs: self.timeout,
            },
        }
    }
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let config = Cli::parse().into_config();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(serve(config))
}
