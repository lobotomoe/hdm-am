//! Local HTTP bridge for the Armenian fiscal cash register (HDM) protocol.
//!
//! A browser cannot open a raw TCP socket, but the physical HDM protocol is raw 3DES-over-TCP. This
//! crate runs a small localhost HTTP server that speaks HTTP/JSON to the browser and the HDM TCP
//! protocol (via [`hdm_am::Client`]) to the device, with one `POST /v1/<op>` per protocol operation.
//!
//! The server is exposed as [`serve`] so it can run standalone (the `hdm-bridge` binary) or be
//! embedded in another process (e.g. the GUI app).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod config;
mod device;
mod error;
mod routes;

#[cfg(test)]
mod tests;

pub use config::{BridgeConfig, PartialConn};

use anyhow::Context as _;

use crate::routes::AppState;

/// Run the bridge until the process receives a shutdown signal (Ctrl-C).
///
/// # Errors
/// Fails if authentication is misconfigured (no token and not explicitly insecure), if the bind
/// address is unavailable, or if the server loop errors.
pub async fn serve(config: BridgeConfig) -> anyhow::Result<()> {
    if config.token.is_none() && !config.insecure_no_auth {
        anyhow::bail!(
            "refusing to start without a bearer token: set HDM_BRIDGE_TOKEN or pass --insecure-no-auth (loopback dev only)"
        );
    }
    if config.token.is_none() {
        log::warn!(
            "hdm-bridge: running WITHOUT authentication (--insecure-no-auth); use loopback only"
        );
    }
    if config.allow_origins.is_empty() {
        log::warn!(
            "hdm-bridge: no --allow-origin configured; browsers will be blocked by CORS (curl/native clients still work)"
        );
    }

    let bind = config.bind;
    let app = routes::app(AppState::new(config));

    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .with_context(|| format!("binding {bind}"))?;
    log::info!("hdm-bridge listening on http://{bind}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("http server error")?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            log::error!("hdm-bridge: failed to listen for Ctrl-C: {err}");
        }
    };

    // A managed `hdm bridge stop` (and most service managers) terminate with SIGTERM, not SIGINT —
    // handle both so shutdown is always graceful.
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut term) => {
                term.recv().await;
            }
            Err(err) => log::error!("hdm-bridge: failed to listen for SIGTERM: {err}"),
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
    log::info!("hdm-bridge: shutting down");
}
