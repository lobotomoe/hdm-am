//! Supervises the separate `hdm-bridge` binary as a managed process.
//!
//! The CLI deliberately does **not** depend on the bridge crate or its async stack. It locates the
//! `hdm-bridge` executable, runs it, and (on Unix) signals it. The only coupling is a runtime
//! contract: the binary name, the `HDM_*` / `HDM_BRIDGE_*` settings it reads, and the convention
//! that it shuts down gracefully on `SIGTERM`.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

use crate::{BridgeAction, BridgeArgs, BridgeRunArgs, Cli};

/// Name of the bridge executable to look for.
const BRIDGE_BIN: &str = if cfg!(windows) {
    "hdm-bridge.exe"
} else {
    "hdm-bridge"
};

/// Route a `bridge` subcommand.
pub fn dispatch(cli: &Cli, args: &BridgeArgs) -> Result<()> {
    match &args.action {
        BridgeAction::Run(run_args) => run_foreground(cli, run_args),
        BridgeAction::Start(run_args) => start(cli, run_args),
        BridgeAction::Stop => stop(),
        BridgeAction::Status => status(),
    }
}

/// Find the `hdm-bridge` executable: an explicit `HDM_BRIDGE_BIN` override, then a sibling of this
/// binary (the usual `cargo install` / release layout), then plain `PATH` resolution by name.
fn locate_bridge_bin() -> PathBuf {
    if let Some(path) = std::env::var_os("HDM_BRIDGE_BIN") {
        return PathBuf::from(path);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(sibling) = exe.parent().map(|dir| dir.join(BRIDGE_BIN)) {
            if sibling.is_file() {
                return sibling;
            }
        }
    }
    PathBuf::from(BRIDGE_BIN)
}

/// Build the child command, forwarding the device connection and bridge settings. Secrets go via
/// the environment (never argv, so they don't show up in `ps`); the child also inherits the parent
/// environment, so any unset value falls back to the caller's `HDM_*` env.
fn bridge_command(cli: &Cli, args: &BridgeRunArgs) -> Command {
    let mut cmd = Command::new(locate_bridge_bin());
    if let Some(host) = &cli.host {
        cmd.env("HDM_HOST", host);
    }
    cmd.env("HDM_PORT", cli.port.to_string());
    if let Some(password) = &cli.password {
        cmd.env("HDM_PASSWORD", password);
    }
    if let Some(cashier) = cli.cashier {
        cmd.env("HDM_CASHIER", cashier.to_string());
    }
    if let Some(pin) = &cli.pin {
        cmd.env("HDM_PIN", pin);
    }
    cmd.env("HDM_TIMEOUT", cli.timeout.to_string());
    if let Some(bind) = &args.bind {
        cmd.env("HDM_BRIDGE_BIND", bind);
    }
    if let Some(token) = &args.token {
        cmd.env("HDM_BRIDGE_TOKEN", token);
    }
    if !args.allow_origins.is_empty() {
        cmd.env("HDM_BRIDGE_ALLOW_ORIGIN", args.allow_origins.join(","));
    }
    if args.insecure_no_auth {
        cmd.arg("--insecure-no-auth");
    }
    cmd
}

/// `bridge run` — foreground until terminated. On Unix this *replaces* the CLI process with the
/// bridge (`exec`) so signals from a service manager reach the bridge directly; elsewhere it spawns
/// and waits.
fn run_foreground(cli: &Cli, args: &BridgeRunArgs) -> Result<()> {
    let mut cmd = bridge_command(cli, args);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        // `exec` only returns on failure.
        Err(cmd.exec()).context("exec hdm-bridge (is it installed / on PATH?)")
    }
    #[cfg(not(unix))]
    {
        let status = cmd.status().context("running hdm-bridge")?;
        if !status.success() {
            anyhow::bail!("hdm-bridge exited with status {status}");
        }
        Ok(())
    }
}

#[cfg(unix)]
fn start(cli: &Cli, args: &BridgeRunArgs) -> Result<()> {
    daemon::start(cli, args)
}
#[cfg(unix)]
fn stop() -> Result<()> {
    daemon::stop()
}
#[cfg(unix)]
fn status() -> Result<()> {
    daemon::status()
}

#[cfg(not(unix))]
const UNIX_ONLY: &str =
    "background bridge management is Unix-only for now; use `hdm bridge run` (foreground)";
#[cfg(not(unix))]
fn start(_cli: &Cli, _args: &BridgeRunArgs) -> Result<()> {
    anyhow::bail!(UNIX_ONLY)
}
#[cfg(not(unix))]
fn stop() -> Result<()> {
    anyhow::bail!(UNIX_ONLY)
}
#[cfg(not(unix))]
fn status() -> Result<()> {
    anyhow::bail!(UNIX_ONLY)
}

#[cfg(unix)]
mod daemon {
    use std::fs;
    use std::io::Read as _;
    use std::path::{Path, PathBuf};
    use std::process::Stdio;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use anyhow::{Context, Result};
    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;

    use super::bridge_command;
    use crate::{BridgeRunArgs, Cli};

    const DEFAULT_BIND: &str = "127.0.0.1:8077";
    const STOP_TIMEOUT: Duration = Duration::from_secs(10);
    const READY_TIMEOUT: Duration = Duration::from_secs(5);

    /// What `start` records so `stop`/`status` can find and report the running bridge. No secrets.
    #[derive(serde::Serialize, serde::Deserialize)]
    struct State {
        pid: i32,
        bind: String,
        log: String,
        started_unix: u64,
    }

    fn runtime_dir() -> PathBuf {
        std::env::var_os("XDG_RUNTIME_DIR")
            .map_or_else(std::env::temp_dir, PathBuf::from)
            .join("hdm-am")
    }
    fn state_path() -> PathBuf {
        runtime_dir().join("bridge.json")
    }
    fn log_path() -> PathBuf {
        runtime_dir().join("bridge.log")
    }

    fn read_state() -> Result<Option<State>> {
        let path = state_path();
        match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .with_context(|| format!("parsing {}", path.display())),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err).with_context(|| format!("reading {}", path.display())),
        }
    }

    /// Whether `pid` is a live process — signal 0 probes existence without delivering a signal.
    fn alive(pid: i32) -> bool {
        kill(Pid::from_raw(pid), None).is_ok()
    }

    fn now_unix() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    }

    pub fn start(cli: &Cli, args: &BridgeRunArgs) -> Result<()> {
        if let Some(existing) = read_state()? {
            if alive(existing.pid) {
                anyhow::bail!(
                    "bridge already running (pid {}) on {}; stop it first with `hdm bridge stop`",
                    existing.pid,
                    existing.bind
                );
            }
        }

        let dir = runtime_dir();
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        let log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path())
            .with_context(|| format!("opening {}", log_path().display()))?;

        let mut cmd = bridge_command(cli, args);
        cmd.stdin(Stdio::null())
            .stdout(log.try_clone().context("duplicating log handle")?)
            .stderr(log);
        let child = cmd
            .spawn()
            .context("spawning hdm-bridge (is it installed?)")?;
        // Dropping a `Child` neither waits nor kills it; the CLI exits and init reaps the daemon.
        let pid = i32::try_from(child.id()).context("child PID out of range")?;

        let bind = args.bind.clone().unwrap_or_else(|| DEFAULT_BIND.to_owned());
        let state = State {
            pid,
            bind: bind.clone(),
            log: log_path().display().to_string(),
            started_unix: now_unix(),
        };
        fs::write(state_path(), serde_json::to_vec_pretty(&state)?)
            .with_context(|| format!("writing {}", state_path().display()))?;

        // Report "started" only once the port actually accepts connections, so an immediate
        // follow-up request doesn't race the listener. Surface the log if it dies on startup
        // (missing token, port in use, ...).
        match wait_ready(&bind, pid) {
            Readiness::Listening => {
                println!("bridge started (pid {pid}) on {bind}");
                println!("logs:  {}", log_path().display());
                println!("stop:  hdm bridge stop");
                Ok(())
            }
            Readiness::Died => {
                let _ = fs::remove_file(state_path());
                anyhow::bail!(
                    "hdm-bridge exited during startup. Recent log:\n{}",
                    log_tail(&log_path(), 2000)
                )
            }
            Readiness::TimedOut => {
                println!(
                    "bridge started (pid {pid}) but is not yet accepting connections on {bind}; check the log:"
                );
                println!("  {}", log_path().display());
                Ok(())
            }
        }
    }

    enum Readiness {
        Listening,
        Died,
        TimedOut,
    }

    /// Poll until the bridge accepts a TCP connection on `bind`, the process dies, or we time out.
    fn wait_ready(bind: &str, pid: i32) -> Readiness {
        let deadline = Instant::now() + READY_TIMEOUT;
        loop {
            if !alive(pid) {
                return Readiness::Died;
            }
            if std::net::TcpStream::connect(bind).is_ok() {
                return Readiness::Listening;
            }
            if Instant::now() >= deadline {
                return Readiness::TimedOut;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    pub fn stop() -> Result<()> {
        let Some(state) = read_state()? else {
            println!("bridge is not running");
            return Ok(());
        };
        if !alive(state.pid) {
            let _ = fs::remove_file(state_path());
            println!("bridge is not running (cleaned up stale state)");
            return Ok(());
        }

        let pid = Pid::from_raw(state.pid);
        kill(pid, Signal::SIGTERM)
            .with_context(|| format!("sending SIGTERM to pid {}", state.pid))?;

        let deadline = Instant::now() + STOP_TIMEOUT;
        while alive(state.pid) {
            if Instant::now() >= deadline {
                let _ = kill(pid, Signal::SIGKILL);
                println!("bridge did not exit in time; sent SIGKILL");
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = fs::remove_file(state_path());
        println!("bridge stopped (pid {})", state.pid);
        Ok(())
    }

    pub fn status() -> Result<()> {
        match read_state()? {
            None => println!("bridge is not running"),
            Some(state) if alive(state.pid) => {
                let uptime = now_unix().saturating_sub(state.started_unix);
                println!("bridge is running");
                println!("  pid:    {}", state.pid);
                println!("  bind:   {}", state.bind);
                println!("  uptime: {uptime}s");
                println!("  logs:   {}", state.log);
            }
            Some(state) => {
                let _ = fs::remove_file(state_path());
                println!(
                    "bridge is not running (pid {} is gone; cleaned up stale state)",
                    state.pid
                );
            }
        }
        Ok(())
    }

    /// Last `max_bytes` of a (UTF-8) log file, for surfacing a startup failure.
    fn log_tail(path: &Path, max_bytes: usize) -> String {
        let Ok(mut file) = fs::File::open(path) else {
            return String::new();
        };
        let mut buf = String::new();
        let _ = file.read_to_string(&mut buf);
        if buf.len() > max_bytes {
            buf.split_off(buf.len() - max_bytes)
        } else {
            buf
        }
    }
}
