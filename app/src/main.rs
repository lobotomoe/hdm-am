//! Desktop executable entrypoint for `hdm-am-app`.

fn main() -> Result<(), slint::PlatformError> {
    hdm_am_app::run()
}
