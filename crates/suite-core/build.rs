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
    // the trusted plugin publisher keys (Phase 7 O4): optional, a list of 64 lower-case hex digits each
    let keys: Vec<String> = doc
        .get("plugins")
        .and_then(|p| p.get("publisher_keys"))
        .and_then(|k| k.as_array())
        .map(|a| {
            a.iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();
    for k in &keys {
        assert!(
            k.len() == 64 && k.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
            "brand.toml: plugins.publisher_keys: \"{k}\" is not 64 lower-case hex digits"
        );
    }
    println!("cargo:rustc-env=SUITE_PLUGIN_KEYS={}", keys.join(","));
    println!(
        "cargo:rustc-env=SUITE_FONT_SANS={}",
        get(&["fonts", "fallback_sans"])
    );
    println!(
        "cargo:rustc-env=SUITE_FONT_MONO={}",
        get(&["fonts", "fallback_mono"])
    );
}
