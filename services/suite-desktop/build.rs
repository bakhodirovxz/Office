//! Windows only: embed the product icon and version info into the host exe, so Explorer, the taskbar and Alt-Tab show it.
//! Every name comes from `brand/brand.toml` and the icon from the logo pack (`brand/logo`); nothing is written here.
//! A missing resource compiler is a warning, unless `SUITE_REQUIRE_WINRES` is set (release builds).
use std::path::Path;

fn main() {
    let root = Path::new(&std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets it")).join("../..");
    let brand = root.join("brand/brand.toml");
    let ico = root.join("brand/logo/windows/hoshiya.ico");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", brand.display());
    println!("cargo:rerun-if-changed={}", ico.display());
    println!("cargo:rerun-if-env-changed=SUITE_REQUIRE_WINRES");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let text = std::fs::read_to_string(&brand).unwrap_or_else(|e| panic!("{}: {e}", brand.display()));
    let doc: toml::Table = text.parse().unwrap_or_else(|e| panic!("brand.toml: {e}"));
    let p = &doc["product"];
    let get = |k: &str| p[k].as_str().unwrap_or_else(|| panic!("brand.toml: product.{k}")).to_string();
    let exe = format!("{}.exe", get("id"));
    let mut res = winresource::WindowsResource::new();
    res.set_icon(&ico.to_string_lossy())
        .set("ProductName", &get("name"))
        .set("FileDescription", &get("name"))
        .set("CompanyName", &get("vendor"))
        .set("LegalCopyright", &get("copyright"))
        .set("OriginalFilename", &exe)
        .set("InternalName", &get("id"));
    if let Err(e) = res.compile() {
        if std::env::var_os("SUITE_REQUIRE_WINRES").is_some() {
            eprintln!("embedding Windows resources failed: {e}");
            std::process::exit(1);
        }
        println!("cargo:warning=the host exe is built without icon/version resources: {e}");
    }
}
