// The module imports its memory from the host (`env.memory`, docs/plugins.md section 3) and carries a small stack.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        for a in ["--import-memory", "-zstack-size=262144", "--no-entry"] {
            println!("cargo:rustc-link-arg-cdylib={a}");
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
}
