//! Reads the names this crate must not hard-code from `brand/brand.toml` (the one brand config) and hands them to the compiler.
use std::path::Path;

fn main() {
    let brand = Path::new(&std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets it"))
        .join("../../brand/brand.toml");
    println!("cargo:rerun-if-changed={}", brand.display());
    let text =
        std::fs::read_to_string(&brand).unwrap_or_else(|e| panic!("{}: {e}", brand.display()));
    let doc: toml_edit::DocumentMut = text
        .parse()
        .unwrap_or_else(|e| panic!("{}: {e}", brand.display()));
    let get = |path: &[&str]| -> String {
        let mut item = doc.as_item();
        for k in path {
            item = item
                .get(k)
                .unwrap_or_else(|| panic!("brand.toml: missing {}", path.join(".")));
        }
        item.as_str()
            .unwrap_or_else(|| panic!("brand.toml: {} is not a string", path.join(".")))
            .to_string()
    };
    println!(
        "cargo:rustc-env=SUITE_PRODUCT={}",
        get(&["product", "name"])
    );
    println!(
        "cargo:rustc-env=SUITE_PRODUCT_ID={}",
        get(&["product", "id"])
    );
    for app in ["docs", "sheets", "slides", "pdf"] {
        println!(
            "cargo:rustc-env=SUITE_DATA_DIR_{}={}",
            app.to_uppercase(),
            get(&["apps", app, "data_dir"])
        );
    }
    println!(
        "cargo:rustc-env=SUITE_FONT_SANS={}",
        get(&["fonts", "fallback_sans"])
    );
    println!(
        "cargo:rustc-env=SUITE_FONT_MONO={}",
        get(&["fonts", "fallback_mono"])
    );
}
