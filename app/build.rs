//! Build-time compilation for the Slint UI markup.

fn main() {
    if let Err(err) = slint_build::compile("ui/main.slint") {
        panic!("failed to compile Slint UI: {err}");
    }
}
