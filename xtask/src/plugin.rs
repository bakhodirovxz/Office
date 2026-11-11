//! `cargo xt plugin build <name|folder> | pack <name|folder> | commands [--check]` (Phase 7 WP6, WP11): build a first-party plugin from `plugins/<name>` for
//! `wasm32-unknown-unknown` and pack it. A package is `plugin.toml`, `plugin.wasm` and, when present, `strings.tsv`, stored (not
//! compressed) in that order with a fixed timestamp and mode, so the same inputs always give the same bytes and the same SHA-256.
//! The manifest and the entry list are checked with `plugin-api` before anything is written.
//!
//! `keygen <seed file>`, `pubkey <seed file>`, `sign <package.zip|folder> <seed file>` (Phase 7 O4): make an Ed25519 key (its 32-byte
//! seed as hex, never overwritten), print its public key, add a `SIGNATURE` over `suite_core::plugins::signed_message` to a package.
//! For test keys (`ci/plugins/signing`) and an administrator's own key; the project key is made and kept offline by the owner.

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
        ["keygen", file] => keygen(Path::new(file)),
        ["pubkey", file] => read_seed(Path::new(file)).map(|s| public_key(&s)),
        ["sign", package, file] => sign(Path::new(package), &read_seed(Path::new(file))?),
        _ => Err("usage: cargo xt plugin build <name|folder> | pack <name|folder> | commands [--check] | \
                  keygen <seed file> | pubkey <seed file> | sign <package.zip|folder> <seed file>"
            .into()),
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

/// `docs/plugins-commands.md` (Docs) and `-sheets.md`, `-slides.md` (WP13), each written by a test of its domain, the only place that can see
/// the real command registry. `--check` runs the same tests without the write switch: they fail when a file is stale.
fn commands(root: &Path, check: bool) -> Result<String, String> {
    for (domain, krate) in [
        ("docs", "docs-office"),
        ("sheets", "sheets-plugins"),
        ("slides", "slides-plugins"),
    ] {
        let mut cmd = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
        cmd.args([
            "test",
            "--locked",
            "--manifest-path",
            &format!("domains/{domain}/Cargo.toml"),
            "-p",
            krate,
        ])
        .arg("--lib")
        .arg("the_generated_command_list_is_current")
        .current_dir(root);
        if !check {
            cmd.env("PLUGIN_COMMANDS_WRITE", "1");
        }
        let status = cmd.status().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(if check {
                format!("the {domain} command list is stale: run `cargo xt plugin commands`")
            } else {
                format!("generating the {domain} command list failed")
            });
        }
    }
    Ok(if check {
        "the command lists are current"
    } else {
        "wrote the command lists"
    }
    .into())
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
    let bytes = zip_of(&entries)?;
    let out = p.target(root).join("plugins");
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let file = out.join(format!("{}-{}.zip", m.id, m.version));
    fs::write(&file, &bytes).map_err(|e| e.to_string())?;
    Ok(format!(
        "{}  {} ({} bytes)",
        sha(&bytes),
        file.display(),
        bytes.len()
    ))
}

fn sha(b: &[u8]) -> String {
    Sha256::digest(b)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The entries as a stored zip, checked by `plugin-api` first: fixed order, time and mode (the same inputs give the same bytes).
fn zip_of(entries: &[(&str, Vec<u8>)]) -> Result<Vec<u8>, String> {
    let sizes: Vec<(&str, u64)> = entries.iter().map(|(n, b)| (*n, b.len() as u64)).collect();
    plugin_api::manifest::check_package(&sizes).map_err(|e| e.to_string())?;
    let opts = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644);
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (n, b) in entries {
        zip.start_file(*n, opts).map_err(|e| e.to_string())?;
        zip.write_all(b).map_err(|e| e.to_string())?;
    }
    Ok(zip.finish().map_err(|e| e.to_string())?.into_inner())
}

fn read_seed(file: &Path) -> Result<Vec<u8>, String> {
    let t = fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let t = t.trim();
    if t.len() != 64 || !t.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return Err(format!(
            "{}: not a seed (64 lower-case hex digits)",
            file.display()
        ));
    }
    Ok((0..32)
        .map(|i| u8::from_str_radix(&t[2 * i..2 * i + 2], 16).unwrap_or(0))
        .collect())
}

fn public_key(seed: &[u8]) -> String {
    suite_core::plugins::signature_file(seed, b"")
        .ok()
        .and_then(|f| f.split(' ').nth(1).map(str::to_string))
        .unwrap_or_default()
}

/// A new seed from the OS's random source, written to `file` (never over an existing one); prints the public key.
fn keygen(file: &Path) -> Result<String, String> {
    use ring::rand::SecureRandom;
    let mut seed = [0u8; 32];
    ring::rand::SystemRandom::new()
        .fill(&mut seed)
        .map_err(|_| "no random source")?;
    let hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(file)
        .map_err(|e| format!("{}: {e}", file.display()))?;
    writeln!(f, "{hex}").map_err(|e| e.to_string())?;
    Ok(format!(
        "public key {} (the seed in {} is the private key: keep it offline)",
        public_key(&seed),
        file.display()
    ))
}

/// Signs a package folder (writes `SIGNATURE` next to `plugin.toml`) or a zip (rewritten in the `pack` layout with `SIGNATURE` last).
fn sign(package: &Path, seed: &[u8]) -> Result<String, String> {
    const SIGNED: [&str; 3] = ["plugin.toml", "plugin.wasm", "strings.tsv"];
    let mut entries: Vec<(&str, Vec<u8>)> = vec![];
    if package.is_dir() {
        for n in SIGNED {
            if let Ok(b) = fs::read(package.join(n)) {
                entries.push((n, b));
            }
        }
    } else {
        let f = fs::File::open(package).map_err(|e| format!("{}: {e}", package.display()))?;
        let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
        for n in SIGNED {
            if let Ok(mut e) = z.by_name(n) {
                let mut b = vec![];
                std::io::Read::read_to_end(&mut e, &mut b).map_err(|e| e.to_string())?;
                entries.push((n, b));
            }
        }
    }
    let get = |n: &str| {
        entries
            .iter()
            .find(|(e, _)| *e == n)
            .map(|(_, b)| b.as_slice())
    };
    let (Some(toml), Some(wasm)) = (get("plugin.toml"), get("plugin.wasm")) else {
        return Err(format!(
            "{}: no plugin.toml and plugin.wasm",
            package.display()
        ));
    };
    let msg = suite_core::plugins::signed_message(toml, wasm, get("strings.tsv"));
    let sig = suite_core::plugins::signature_file(seed, &msg)?;
    let by = public_key(seed);
    if package.is_dir() {
        fs::write(package.join("SIGNATURE"), &sig).map_err(|e| e.to_string())?;
        return Ok(format!("{} signed by {by}", package.display()));
    }
    entries.push(("SIGNATURE", sig.into_bytes()));
    let bytes = zip_of(&entries)?;
    fs::write(package, &bytes).map_err(|e| e.to_string())?;
    Ok(format!(
        "{}  {} signed by {by}",
        sha(&bytes),
        package.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What this tool signs, the store reads as signed by that key; the zip and the folder forms carry the same SIGNATURE.
    #[test]
    fn a_signed_package_reads_back_as_signed_by_the_key() {
        let d = std::env::temp_dir().join(format!("xt-sign-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("dir")).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let seed = read_seed(&root.join("ci/plugins/signing/test-a.seed")).unwrap();
        let toml = "id = \"demo.plugin\"\nversion = \"1.0.0\"\napi = 1\napps = [\"docs\"]\npublisher = \"T\"\n[name]\nen = \"D\"\n";
        let files: Vec<(&str, Vec<u8>)> = vec![
            ("plugin.toml", toml.as_bytes().to_vec()),
            ("plugin.wasm", b"\x00asm\x01\x00\x00\x00".to_vec()),
            ("strings.tsv", b"id\ten\n".to_vec()),
        ];
        for (n, b) in &files {
            fs::write(d.join("dir").join(n), b).unwrap();
        }
        sign(&d.join("dir"), &seed).unwrap();
        let p = suite_core::plugins::read_package(&d.join("dir")).unwrap();
        assert_eq!(p.signer.as_deref(), Some(public_key(&seed).as_str()));
        fs::write(d.join("p.zip"), zip_of(&files).unwrap()).unwrap();
        sign(&d.join("p.zip"), &seed).unwrap();
        let mut z = zip::ZipArchive::new(fs::File::open(d.join("p.zip")).unwrap()).unwrap();
        let mut s = String::new();
        std::io::Read::read_to_string(&mut z.by_name("SIGNATURE").unwrap(), &mut s).unwrap();
        assert_eq!(s, fs::read_to_string(d.join("dir/SIGNATURE")).unwrap());
        // keygen never overwrites
        assert!(keygen(&d.join("dir/SIGNATURE")).is_err());
        assert!(
            keygen(&d.join("new.seed"))
                .unwrap()
                .starts_with("public key ")
        );
        let _ = fs::remove_dir_all(&d);
    }
}
