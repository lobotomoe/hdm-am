//! `hdm update` — self-update via the cargo-dist install receipt.
//!
//! The release installer script writes an install receipt (recording where and how `hdm` was
//! installed); [`axoupdater`] reads it, checks GitHub for a newer release, and replaces the binary
//! in place. This only works for installs done through that script — a `cargo install` build or a
//! manually-placed binary has no receipt, and the command says so instead of guessing.

use anyhow::{Context, Result};
use axoupdater::AxoUpdater;

/// The cargo-dist "app name" (the package name, not the `hdm` binary name) whose install receipt we
/// read. Must match the package in `Cargo.toml`.
const APP_NAME: &str = "hdm-am-cli";

/// Update the running `hdm` binary in place to the latest GitHub release.
pub fn run() -> Result<()> {
    let mut updater = AxoUpdater::new_for(APP_NAME);
    updater.load_receipt().context(
        "could not read the install receipt — `hdm update` only works for installs done via the \
         release installer script (see the README). Re-run the installer, or update your package \
         manager / source checkout instead.",
    )?;

    match updater.run_sync().context("installing the update")? {
        Some(_) => println!("hdm updated to the latest release. Run `hdm --version` to confirm."),
        None => println!("hdm is already up to date."),
    }
    Ok(())
}
