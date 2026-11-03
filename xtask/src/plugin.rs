//! `cargo xt plugin build <name> | pack <name>` (Phase 7 WP6): build a first-party plugin from `plugins/<name>` for
//! `wasm32-unknown-unknown` and pack it. A package is `plugin.toml`, `plugin.wasm` and, when present, `strings.tsv`, stored (not
//! compressed) in that order with a fixed timestamp and mode, so the same inputs always give the same bytes and the same SHA-256.
//! The manifest and the entry list are checked with `plugin-api` before anything is written.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime};

pub fn run(root: &Path, args: &[&str]) -> Result<String, String> {
    match args {
        ["build", name] => build(root, name).map(|p| p.display().to_string()),
        ["pack", name] => pack(root, name),
        _ => Err("usage: cargo xt plugin build <name> | pack <name>".into()),
    }
}

fn target_dir(root: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| root.join("target"))
}

/// Build the cdylib with the `plugin` profile; returns the path of the `.wasm`.
fn build(root: &Path, name: &str) -> Result<PathBuf, String> {
    let dir = root.join("plugins").join(name);
    let manifest = fs::read_to_string(dir.join("Cargo.toml")).map_err(|e| format!("plugins/{name}/Cargo.toml: {e}"))?;
    let krate = manifest
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| e.to_string())?
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(str::to_string)
        .ok_or("no package name")?;
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["build", "--locked", "--profile", "plugin", "--target", "wasm32-unknown-unknown", "-p", &krate])
        .current_dir(root)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("cargo build -p {krate} failed"));
    }
    Ok(target_dir(root).join("wasm32-unknown-unknown/plugin").join(format!("{}.wasm", krate.replace('-', "_"))))
}

fn pack(root: &Path, name: &str) -> Result<String, String> {
    let wasm = fs::read(build(root, name)?).map_err(|e| e.to_string())?;
    let dir = root.join("plugins").join(name);
    let toml = fs::read(dir.join("plugin.toml")).map_err(|e| format!("plugins/{name}/plugin.toml: {e}"))?;
    let m = plugin_api::manifest::parse(std::str::from_utf8(&toml).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let mut entries: Vec<(&str, Vec<u8>)> = vec![("plugin.toml", toml), ("plugin.wasm", wasm)];
    if let Ok(s) = fs::read(dir.join("strings.tsv")) {
        entries.push(("strings.tsv", s));
    }
    let sizes: Vec<(&str, u64)> = entries.iter().map(|(n, b)| (*n, b.len() as u64)).collect();
    plugin_api::manifest::check_package(&sizes).map_err(|e| e.to_string())?;
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored).last_modified_time(DateTime::default()).unix_permissions(0o644);
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (n, b) in &entries {
        zip.start_file(*n, opts).map_err(|e| e.to_string())?;
        zip.write_all(b).map_err(|e| e.to_string())?;
    }
    let bytes = zip.finish().map_err(|e| e.to_string())?.into_inner();
    let out = target_dir(root).join("plugins");
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let file = out.join(format!("{}-{}.zip", m.id, m.version));
    fs::write(&file, &bytes).map_err(|e| e.to_string())?;
    let sha: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!("{sha}  {} ({} bytes)", file.display(), bytes.len()))
}
