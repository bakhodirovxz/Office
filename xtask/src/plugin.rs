//! `cargo xt plugin build <name|folder> | pack <name|folder> | commands [--check]` (Phase 7 WP6, WP11): build a first-party plugin from `plugins/<name>` for
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
        ["build", name] => build(root, &Plugin::new(root, name)).map(|p| p.display().to_string()),
        ["pack", name] => pack(root, name),
        ["commands"] => commands(root, false),
        ["commands", "--check"] => commands(root, true),
        _ => Err(
            "usage: cargo xt plugin build <name|folder> | pack <name|folder> | commands [--check]"
                .into(),
        ),
    }
}

/// Where a plugin lives. `plugins/<name>` is a member of the root workspace (built with the `plugin` profile). A name with a path
/// separator is a folder of its own, outside the workspace (`cargo xt plugin build ../hello`; the author's Cargo.toml has its own
/// profile): it is built with `--release` by its own manifest.
struct Plugin {
    dir: PathBuf,
    external: bool,
}

impl Plugin {
    fn new(root: &Path, name: &str) -> Plugin {
        let external = name.contains(['/', '\\']);
        let dir = if external {
            root.join(name)
        } else {
            root.join("plugins").join(name)
        };
        Plugin { dir, external }
    }

    fn target(&self, root: &Path) -> PathBuf {
        match std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from) {
            Some(t) => t,
            None if self.external => self.dir.join("target"),
            None => root.join("target"),
        }
    }
}

/// Build the cdylib; returns the path of the `.wasm`.
fn build(root: &Path, p: &Plugin) -> Result<PathBuf, String> {
    let manifest = fs::read_to_string(p.dir.join("Cargo.toml"))
        .map_err(|e| format!("{}: {e}", p.dir.join("Cargo.toml").display()))?;
    let krate = manifest
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| e.to_string())?
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(str::to_string)
        .ok_or("no package name")?;
    let mut cmd = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cmd.arg("build");
    let profile = if p.external {
        cmd.args(["--release", "--manifest-path"])
            .arg(p.dir.join("Cargo.toml"))
            .current_dir(&p.dir);
        "release"
    } else {
        cmd.args(["--locked", "--profile", "plugin", "-p", &krate])
            .current_dir(root);
        "plugin"
    };
    let status = cmd
        .args(["--target", "wasm32-unknown-unknown"])
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("cargo build of {krate} failed"));
    }
    Ok(p.target(root)
        .join("wasm32-unknown-unknown")
        .join(profile)
        .join(format!("{}.wasm", krate.replace('-', "_"))))
}

/// `docs/plugins-commands.md`, written by a test of the Docs domain, the only place that can see the real command registry.
/// `--check` runs the same test without the write switch: it fails when the file is stale.
fn commands(root: &Path, check: bool) -> Result<String, String> {
    let mut cmd = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cmd.args([
        "test",
        "--locked",
        "--manifest-path",
        "domains/docs/Cargo.toml",
        "-p",
        "docs-office",
        "--lib",
        "the_generated_command_list_is_current",
    ])
    .current_dir(root);
    if !check {
        cmd.env("PLUGIN_COMMANDS_WRITE", "1");
    }
    let status = cmd.status().map_err(|e| e.to_string())?;
    match (status.success(), check) {
        (true, true) => Ok("docs/plugins-commands.md is current".into()),
        (true, false) => Ok("wrote docs/plugins-commands.md".into()),
        (false, true) => {
            Err("docs/plugins-commands.md is stale: run `cargo xt plugin commands`".into())
        }
        (false, false) => Err("generating docs/plugins-commands.md failed".into()),
    }
}

fn pack(root: &Path, name: &str) -> Result<String, String> {
    let p = Plugin::new(root, name);
    let wasm = fs::read(build(root, &p)?).map_err(|e| e.to_string())?;
    let dir = &p.dir;
    let toml = fs::read(dir.join("plugin.toml"))
        .map_err(|e| format!("{}: {e}", dir.join("plugin.toml").display()))?;
    let m = plugin_api::manifest::parse(std::str::from_utf8(&toml).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let mut entries: Vec<(&str, Vec<u8>)> = vec![("plugin.toml", toml), ("plugin.wasm", wasm)];
    if let Ok(s) = fs::read(dir.join("strings.tsv")) {
        entries.push(("strings.tsv", s));
    }
    let sizes: Vec<(&str, u64)> = entries.iter().map(|(n, b)| (*n, b.len() as u64)).collect();
    plugin_api::manifest::check_package(&sizes).map_err(|e| e.to_string())?;
    let opts = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644);
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (n, b) in &entries {
        zip.start_file(*n, opts).map_err(|e| e.to_string())?;
        zip.write_all(b).map_err(|e| e.to_string())?;
    }
    let bytes = zip.finish().map_err(|e| e.to_string())?.into_inner();
    let out = p.target(root).join("plugins");
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let file = out.join(format!("{}-{}.zip", m.id, m.version));
    fs::write(&file, &bytes).map_err(|e| e.to_string())?;
    let sha: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok(format!("{sha}  {} ({} bytes)", file.display(), bytes.len()))
}
