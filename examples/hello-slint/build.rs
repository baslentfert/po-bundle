fn main() {
    // Rust strings (tr!) and Slint strings (@tr) share the same folder.
    po_bundle::build::compile("translations");
    let config = slint_build::CompilerConfiguration::new().with_bundled_translations("translations");
    slint_build::compile_with_config("ui/app.slint", config).unwrap();
}
