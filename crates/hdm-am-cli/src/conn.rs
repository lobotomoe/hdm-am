//! Connection setup, the operator session lifecycle, and the output helper shared by handlers.

use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use hdm_am::{Client, InMemorySeq};

use crate::Cli;

/// A client whose transport is a real TCP socket and whose sequence counter lives in memory
/// (a fresh login establishes a fresh server-side session, so cross-run persistence is moot).
pub type TcpClient = Client<TcpStream, InMemorySeq>;

/// Resolve the host/port and open a TCP connection with the configured timeout applied to the
/// connect, read, and write phases.
pub fn connect(cli: &Cli) -> Result<TcpStream> {
    let host = require(cli.host.as_deref(), "host", "HDM_HOST")?;
    let timeout = Duration::from_secs(cli.timeout);

    let addr = (host, cli.port)
        .to_socket_addrs()
        .with_context(|| format!("resolving {host}:{}", cli.port))?
        .next()
        .ok_or_else(|| anyhow!("{host}:{} resolved to no addresses", cli.port))?;

    let stream = TcpStream::connect_timeout(&addr, timeout)
        .with_context(|| format!("connecting to {addr} (timeout {}s)", cli.timeout))?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    Ok(stream)
}

/// Build a client over a fresh connection, using the password from the CLI.
pub fn client(cli: &Cli) -> Result<TcpClient> {
    let password = require(cli.password.as_deref(), "password", "HDM_PASSWORD")?;
    let stream = connect(cli)?;
    Ok(Client::new(stream, password, InMemorySeq::default()))
}

/// Connect, log in, run `op`, then log out — even if `op` fails. Session operations go through
/// here so the device session is always released on a clean exit.
pub fn with_session<T>(cli: &Cli, op: impl FnOnce(&mut TcpClient) -> Result<T>) -> Result<T> {
    let cashier = cli.cashier.ok_or_else(|| {
        anyhow!("this operation needs an operator: pass --cashier or set HDM_CASHIER")
    })?;
    let pin = require(cli.pin.as_deref(), "pin", "HDM_PIN")?;

    let mut c = client(cli)?;
    c.login(cashier, pin)
        .with_context(|| format!("logging in as cashier {cashier}"))?;

    let result = op(&mut c);

    if let Err(err) = c.logout() {
        // Logout failure must not mask the operation's own outcome; surface it as a warning.
        log::warn!("logout failed: {err}");
    }
    result
}

/// Fetch a required string parameter, with a message naming both the flag and the env var.
pub fn require<'a>(value: Option<&'a str>, flag: &str, env: &str) -> Result<&'a str> {
    value.ok_or_else(|| anyhow!("missing --{flag}: pass it or set {env}"))
}

/// Render a result to stdout: pretty JSON when `--json` is set, otherwise via `render`.
///
/// `render` is only invoked in text mode, so text-only formatting never runs needlessly.
pub fn emit<T: serde::Serialize>(cli: &Cli, value: &T, render: impl FnOnce(&T)) -> Result<()> {
    if cli.json {
        let json = serde_json::to_string_pretty(value).context("serialising response to JSON")?;
        println!("{json}");
    } else {
        render(value);
    }
    Ok(())
}

/// Ask a yes/no question on the terminal. Returns `true` only on an explicit `y`/`yes`.
///
/// The prompt goes to stderr so it never contaminates `--json` output on stdout.
pub fn confirm(question: &str) -> Result<bool> {
    use std::io::Write;
    eprint!("{question} [y/N] ");
    std::io::stderr().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

#[cfg(test)]
mod tests {
    use super::require;

    #[test]
    fn require_error_names_flag_and_env() {
        let err = require(None, "host", "HDM_HOST").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("--host"), "{msg}");
        assert!(msg.contains("HDM_HOST"), "{msg}");
    }
}
