//! Build-time compilation for the Slint UI markup.

fn main() {
    // Render std-widgets (LineEdit, Switch, ScrollView) with the native iOS look, and bundle the
    // .po translations (lang/<locale>/LC_MESSAGES/hdm-am-app.po) into the binary so the app can
    // switch language at runtime without a gettext runtime — the right model for iOS.
    // slint-build does not register the bundled .po files as build dependencies, so cargo would
    // not regenerate the UI when a translation changes. Track them explicitly.
    for locale in ["en", "ru", "hy"] {
        println!("cargo:rerun-if-changed=lang/{locale}/LC_MESSAGES/hdm-am-app.po");
    }

    let config = slint_build::CompilerConfiguration::new()
        .with_style("cupertino".into())
        .with_bundled_translations("lang");
    if let Err(err) = slint_build::compile_with_config("ui/main.slint", config) {
        panic!("failed to compile Slint UI: {err}");
    }
}
