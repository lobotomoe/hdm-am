//! Build-time compilation for the Slint UI markup.

fn main() {
    // Render std-widgets (LineEdit, Switch, ScrollView) with the native iOS look.
    let config = slint_build::CompilerConfiguration::new().with_style("cupertino".into());
    if let Err(err) = slint_build::compile_with_config("ui/main.slint", config) {
        panic!("failed to compile Slint UI: {err}");
    }
}
