//! Native GUI application shell for `hdm-am`.
//!
//! The crate is intentionally separate from the protocol library and CLI: the root crate remains a
//! reusable HDM client, `cli/` stays scriptable, and this crate owns presentation, desktop/mobile
//! entrypoints, and UI-specific orchestration.

mod bridge;
mod format;
mod i18n;
mod secrets;
mod storage;
#[allow(
    clippy::all,
    clippy::expect_used,
    clippy::nursery,
    clippy::pedantic,
    clippy::unwrap_used
)]
mod generated {
    slint::include_modules!();
}
mod validation;

/// Run the GUI application on the current platform.
///
/// # Errors
/// Returns a Slint platform error if the native windowing backend cannot be initialised.
pub fn run() -> Result<(), slint::PlatformError> {
    #[cfg(target_os = "ios")]
    configure_platform_backend()?;
    bridge::run()
}

#[cfg(target_os = "ios")]
fn configure_platform_backend() -> Result<(), slint::PlatformError> {
    slint::BackendSelector::new()
        .backend_name("winit".to_owned())
        .renderer_name("skia".to_owned())
        .select()
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    if let Err(err) = slint::android::init(app) {
        eprintln!("hdm-am-app failed to initialise Android backend: {err}");
        return;
    }

    if let Err(err) = run() {
        eprintln!("hdm-am-app failed: {err}");
    }
}
