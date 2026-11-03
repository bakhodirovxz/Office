//! The plugin store (PLAN phase 7, section 8): where installed packages live, which are enabled and granted, the machine policy,
//! the integrity check and the descriptors the page host gets. The desktop host, `suite-serve` and the docserver all use it.
//!
//! - **Layout.** A package is a folder with `plugin.toml`, `plugin.wasm` (and optionally `strings.tsv`, `SIGNATURE`), checked by
//!   `plugin_api::manifest::check_package`. The user's copy is `<settings dir>/plugins/<id>/<version>/`, plus `install.toml`, the
//!   install record. IT's copies are in the machine folder (`%PROGRAMDATA%\<product>\plugins`).
//! - **Integrity (O4).** The record holds the SHA-256 of `plugin.wasm` and of `plugin.toml`; a package whose files no longer
//!   match is refused (not listed for the page, not served). A package in a machine folder may come without a record (IT filled
//!   it; only an administrator can write there); one in the user folder may not.
//! - **State (O3).** `settings.toml` `[plugins] enabled = [ids]` and `[plugins.grants] <id> = [origins]`. `<machine folder>/policy.toml`
//!   `[plugins] user_install = false` / `user_grants = false` switches user install and user network grants off; a policy file
//!   that cannot be read switches both off.
//! - **Grants.** Per origin, and only origins the manifest declares (`permissions.network`). Never on Android (that app has no store).
use crate::settings::{self, SetError};
use plugin_api::manifest::{self, Manifest};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// The install record inside a package folder (not part of the package itself).
pub const RECORD: &str = "install.toml";
const POLICY: &str = "policy.toml";

fn sha_hex(b: &[u8]) -> String {
    use std::fmt::Write;
    Sha256::digest(b)
        .iter()
        .fold(String::with_capacity(64), |mut s, x| {
            let _ = write!(s, "{x:02x}");
            s
        })
}

/// A package read from a folder, validated by `plugin-api`. `sha256` is that of `plugin.wasm` (what the page host checks).
#[derive(Clone, Debug)]
pub struct Package {
    pub dir: PathBuf,
    pub manifest: Manifest,
    /// `plugin.toml` as JSON, same key names
    pub json: Value,
    pub sha256: String,
    pub manifest_sha256: String,
}

fn package_from(dir: &Path, toml: &[u8], wasm: &[u8]) -> Result<Package, String> {
    if wasm.len() as u64 > manifest::MAX_WASM {
        return Err("plugin.wasm is over 4 MiB".into());
    }
    let text = std::str::from_utf8(toml).map_err(|_| "plugin.toml is not UTF-8")?;
    let (manifest, json) = manifest::parse_json(text).map_err(|e| e.to_string())?;
    if !plugin_api::supports(manifest.api) {
        return Err(format!("API {} is not supported", manifest.api));
    }
    Ok(Package {
        dir: dir.to_path_buf(),
        manifest,
        json,
        sha256: sha_hex(wasm),
        manifest_sha256: sha_hex(toml),
    })
}

/// Reads and validates the package in `dir`. Any other file than the four known names (the record excepted) refuses it.
pub fn read_package(dir: &Path) -> Result<Package, String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    let mut entries: Vec<(String, u64)> = vec![];
    for e in std::fs::read_dir(dir).map_err(io)? {
        let e = e.map_err(io)?;
        let name = e.file_name().to_string_lossy().into_owned();
        if name == RECORD {
            continue;
        }
        let md = e.metadata().map_err(io)?;
        if !md.is_file() {
            return Err(format!("entry `{}` is not allowed", name.escape_debug()));
        }
        entries.push((name, md.len()));
    }
    let refs: Vec<(&str, u64)> = entries.iter().map(|(n, s)| (n.as_str(), *s)).collect();
    manifest::check_package(&refs).map_err(|e| e.to_string())?;
    let toml = std::fs::read(dir.join("plugin.toml")).map_err(io)?;
    let wasm = std::fs::read(dir.join("plugin.wasm")).map_err(io)?;
    package_from(dir, &toml, &wasm)
}

/// The packages under `root`: every folder `root/X` or `root/X/Y` that holds a `plugin.toml`, with the reason when one is refused.
/// Folders whose name starts with `.` (an install in progress) are skipped.
pub fn scan(root: &Path) -> Vec<(PathBuf, Result<Package, String>)> {
    let subdirs = |d: &Path| -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = std::fs::read_dir(d)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.') && e.path().is_dir())
            .map(|e| e.path())
            .collect();
        v.sort();
        v
    };
    let mut out = vec![];
    for d in subdirs(root) {
        if d.join("plugin.toml").is_file() {
            out.push((d.clone(), read_package(&d)));
        } else {
            for d2 in subdirs(&d)
                .into_iter()
                .filter(|d| d.join("plugin.toml").is_file())
            {
                out.push((d2.clone(), read_package(&d2)));
            }
        }
    }
    out
}

/// The wasm bytes of `p`, read again and compared with the digest (a file that changed since the scan is refused).
pub fn read_wasm(p: &Package) -> Result<Vec<u8>, String> {
    let wasm = std::fs::read(p.dir.join("plugin.wasm")).map_err(|e| e.to_string())?;
    if sha_hex(&wasm) != p.sha256 {
        return Err("the package changed on disk".into());
    }
    Ok(wasm)
}

/// The install record (`install.toml`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub sha256: String,
    pub manifest_sha256: String,
    /// Reserved for the publisher signature (O4). shortcut: never read or checked in v1, so a signed package installs like an
    /// unsigned one; the check comes with the project signing key.
    pub signature: Option<String>,
}

fn read_record(dir: &Path) -> Option<Record> {
    let doc: toml_edit::DocumentMut = std::fs::read_to_string(dir.join(RECORD))
        .ok()?
        .parse()
        .ok()?;
    let s = |k: &str| doc.get(k)?.as_str().map(str::to_string);
    Some(Record {
        sha256: s("sha256")?,
        manifest_sha256: s("manifest_sha256")?,
        signature: s("signature"),
    })
}

fn write_record(dir: &Path, r: &Record) -> std::io::Result<()> {
    let mut t = format!(
        "sha256 = \"{}\"\nmanifest_sha256 = \"{}\"\n",
        r.sha256, r.manifest_sha256
    );
    if let Some(s) = &r.signature {
        t.push_str(&format!(
            "signature = {}\n",
            toml_edit::Value::from(s.as_str())
        ));
    }
    std::fs::write(dir.join(RECORD), t)
}

/// Does the package still match its install record? `machine`: a package in a machine folder may have none.
pub fn verify(p: &Package, machine: bool) -> Result<(), String> {
    match read_record(&p.dir) {
        Some(r) if r.sha256 == p.sha256 && r.manifest_sha256 == p.manifest_sha256 => Ok(()),
        Some(_) => Err("the package changed since it was installed".into()),
        None if machine => Ok(()),
        None => Err("the package has no install record".into()),
    }
}

// ---- state and policy ----

/// `settings.toml` `[plugins]`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub enabled: Vec<String>,
    pub grants: BTreeMap<String, Vec<String>>,
}

fn strings(item: Option<&toml_edit::Item>) -> Vec<String> {
    item.and_then(|i| i.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

pub fn load_state(settings_dir: &Path) -> State {
    let doc = settings::read_doc(settings_dir);
    let Some(p) = doc.get("plugins").and_then(|p| p.as_table()) else {
        return State::default();
    };
    State {
        enabled: strings(p.get("enabled")),
        grants: p
            .get("grants")
            .and_then(|g| g.as_table())
            .map(|g| {
                g.iter()
                    .map(|(k, v)| (k.to_string(), strings(Some(v))))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn set_list(table: &mut toml_edit::Table, key: &str, f: impl FnOnce(&mut Vec<String>)) {
    let mut v = strings(table.get(key));
    f(&mut v);
    v.sort();
    v.dedup();
    let arr: toml_edit::Array = v.iter().map(String::as_str).collect();
    table.insert(key, toml_edit::value(arr));
}

pub fn set_enabled(settings_dir: &Path, id: &str, on: bool) -> Result<(), SetError> {
    settings::edit(settings_dir, |doc| {
        set_list(settings::table_at(doc, &["plugins"])?, "enabled", |v| {
            v.retain(|x| x != id);
            if on {
                v.push(id.into());
            }
        });
        Ok(())
    })
}

/// Withdraws every grant of `id`.
pub fn clear_grants(settings_dir: &Path, id: &str) -> Result<(), SetError> {
    settings::edit(settings_dir, |doc| {
        settings::table_at(doc, &["plugins", "grants"])?.remove(id);
        Ok(())
    })
}

pub fn set_grant(settings_dir: &Path, id: &str, origin: &str, on: bool) -> Result<(), SetError> {
    settings::edit(settings_dir, |doc| {
        set_list(settings::table_at(doc, &["plugins", "grants"])?, id, |v| {
            v.retain(|x| x != origin);
            if on {
                v.push(origin.into());
            }
        });
        Ok(())
    })
}

/// What a machine policy forbids (O3). Both are allowed unless the file says otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    pub user_install: bool,
    pub user_grants: bool,
}

impl Default for Policy {
    fn default() -> Policy {
        Policy {
            user_install: true,
            user_grants: true,
        }
    }
}

/// `<machine dir>/policy.toml`. No file: the defaults. A file that cannot be read or parsed: everything off (fail closed).
pub fn load_policy(machine_dir: Option<&Path>) -> Policy {
    let Some(path) = machine_dir.map(|d| d.join(POLICY)) else {
        return Policy::default();
    };
    let off = Policy {
        user_install: false,
        user_grants: false,
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Policy::default(),
        Err(_) => return off,
    };
    let Ok(doc) = text.parse::<toml_edit::DocumentMut>() else {
        return off;
    };
    let flag = |k: &str| {
        doc.get("plugins")
            .and_then(|p| p.get(k))
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    };
    Policy {
        user_install: flag("user_install"),
        user_grants: flag("user_grants"),
    }
}

/// The machine folder: `%PROGRAMDATA%\<product>` (`/etc/<product>` elsewhere). Debug builds honour `SUITE_MACHINE_DIR`.
pub fn machine_dir() -> Option<PathBuf> {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if cfg!(debug_assertions)
        && let Some(p) = env("SUITE_MACHINE_DIR")
    {
        return Some(p);
    }
    let base = if cfg!(windows) {
        env("PROGRAMDATA")
    } else {
        Some("/etc".into())
    };
    base.map(|b| b.join(crate::PRODUCT))
}

// ---- what is enabled ----

/// A folder of packages. `machine`: filled by IT or the operator, so a package without a record is accepted.
#[derive(Clone, Debug)]
pub struct Root {
    pub dir: PathBuf,
    pub machine: bool,
}

/// An enabled, verified package and the origins it may reach.
#[derive(Clone, Debug)]
pub struct Enabled {
    pub package: Package,
    pub grant: Vec<String>,
}

fn effective_grant(p: &Package, st: &State, pol: &Policy) -> Vec<String> {
    if !pol.user_grants {
        return vec![];
    }
    st.grants
        .get(&p.manifest.id)
        .map(|g| {
            g.iter()
                .filter(|o| p.manifest.permissions.network.contains(o))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// Every enabled package that passes the integrity check, once per id (the first root wins: list the machine folder first).
/// A refused one is said on stderr.
pub fn enabled(roots: &[Root], st: &State, pol: &Policy) -> Vec<Enabled> {
    let mut out: Vec<Enabled> = vec![];
    for r in roots {
        for (dir, p) in scan(&r.dir) {
            let p = match p.and_then(|p| verify(&p, r.machine).map(|()| p)) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("plugins: {}: {e}", dir.display());
                    continue;
                }
            };
            if st.enabled.contains(&p.manifest.id)
                && !out.iter().any(|e| e.package.manifest.id == p.manifest.id)
            {
                let grant = effective_grant(&p, st, pol);
                out.push(Enabled { package: p, grant });
            }
        }
    }
    out
}

/// The descriptor the page host gets (`docs/plugins.md` section 9a). `grant` present: only those origins, nobody is asked.
pub fn descriptor(
    p: &Package,
    wasm_url: &str,
    grant: Option<&[String]>,
    settings: Option<&Value>,
) -> Value {
    let mut d = json!({"id": p.manifest.id, "version": p.manifest.version, "sha256": p.sha256, "wasm": wasm_url, "manifest": p.json});
    if let Some(g) = grant {
        d["grant"] = json!(g);
    }
    if let Some(s) = settings {
        d["settings"] = s.clone();
    }
    d
}

/// The descriptors of the enabled packages for the local web host: `wasm` is `/plugins/<sha256>`, `grant` always present.
pub fn local_descriptors(list: &[Enabled]) -> Vec<Value> {
    list.iter()
        .map(|e| {
            descriptor(
                &e.package,
                &format!("/plugins/{}", e.package.sha256),
                Some(&e.grant),
                None,
            )
        })
        .collect()
}

/// The origins any listed plugin may reach: the page's `connect-src` is these and nothing else.
pub fn granted_origins(list: &[Enabled]) -> Vec<String> {
    let mut v: Vec<String> = list.iter().flat_map(|e| e.grant.iter().cloned()).collect();
    v.sort();
    v.dedup();
    v
}

// ---- the store of one user ----

/// One row of the Plugins page.
#[derive(Clone, Debug)]
pub struct Item {
    /// the folder's name, for a package that did not load
    pub name: String,
    pub package: Option<Package>,
    pub machine: bool,
    pub enabled: bool,
    /// why it is not usable (refused, changed, no record); `None` when it is
    pub problem: Option<String>,
    pub grant: Vec<String>,
}

pub struct Store {
    pub user: PathBuf,
    pub machine: Option<PathBuf>,
    pub settings_dir: PathBuf,
}

impl Store {
    /// The real folders of this user. `None` when the platform gives no settings folder.
    pub fn open() -> Option<Store> {
        settings::dir().map(|d| {
            Store::new(
                d.join("plugins"),
                machine_dir().map(|m| m.join("plugins")),
                d,
            )
        })
    }
    pub fn new(user: PathBuf, machine: Option<PathBuf>, settings_dir: PathBuf) -> Store {
        Store {
            user,
            machine,
            settings_dir,
        }
    }
    pub fn policy(&self) -> Policy {
        load_policy(self.machine.as_deref().and_then(Path::parent))
    }
    /// The machine folder first, then the user folder unless the policy forbids user installs (what is in it stays unused).
    pub fn roots(&self) -> Vec<Root> {
        self.machine
            .iter()
            .map(|d| Root {
                dir: d.clone(),
                machine: true,
            })
            .chain(self.policy().user_install.then(|| Root {
                dir: self.user.clone(),
                machine: false,
            }))
            .collect()
    }
    pub fn state(&self) -> State {
        load_state(&self.settings_dir)
    }
    pub fn list(&self) -> Vec<Item> {
        let (st, pol) = (self.state(), self.policy());
        let mut out: Vec<Item> = vec![];
        for r in self.roots() {
            for (dir, p) in scan(&r.dir) {
                let name = dir
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                out.push(match p {
                    Err(e) => Item {
                        name,
                        package: None,
                        machine: r.machine,
                        enabled: false,
                        problem: Some(e),
                        grant: vec![],
                    },
                    Ok(p) => Item {
                        problem: verify(&p, r.machine).err(),
                        enabled: st.enabled.contains(&p.manifest.id),
                        grant: effective_grant(&p, &st, &pol),
                        name,
                        machine: r.machine,
                        package: Some(p),
                    },
                });
            }
        }
        out
    }
    /// The enabled, verified packages (see [`enabled`]).
    pub fn enabled(&self) -> Vec<Enabled> {
        enabled(&self.roots(), &self.state(), &self.policy())
    }
    pub fn set_enabled(&self, id: &str, on: bool) -> Result<(), String> {
        if !self
            .list()
            .iter()
            .any(|i| i.package.as_ref().is_some_and(|p| p.manifest.id == id))
        {
            return Err("no such plugin".into());
        }
        set_enabled(&self.settings_dir, id, on).map_err(|e| e.to_string())
    }
    /// Grants or withdraws one origin; it must be one the plugin's manifest declares. Withdrawing is always allowed.
    pub fn set_grant(&self, id: &str, origin: &str, on: bool) -> Result<(), String> {
        let items = self.list();
        let p = items
            .iter()
            .filter_map(|i| i.package.as_ref())
            .find(|p| p.manifest.id == id)
            .ok_or("no such plugin")?;
        if on && !self.policy().user_grants {
            return Err("network grants are switched off by the administrator".into());
        }
        if !p.manifest.permissions.network.iter().any(|o| o == origin) {
            return Err("the plugin does not declare this address".into());
        }
        set_grant(&self.settings_dir, id, origin, on).map_err(|e| e.to_string())
    }

    /// "Install from file": a zip with the package's flat entries. Refused when the policy forbids user install, when the zip breaks
    /// the package rules, or when the manifest is refused. An older copy of the same id and version is replaced.
    pub fn install_zip(&self, zip_path: &Path) -> Result<Package, String> {
        if !self.policy().user_install {
            return Err("installing plugins is switched off by the administrator".into());
        }
        let f = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
        if f.metadata().map_err(|e| e.to_string())?.len() > manifest::MAX_PACKAGE {
            return Err("package is over 8 MiB".into());
        }
        let mut z = zip::ZipArchive::new(f).map_err(|_| "this is not a zip file".to_string())?;
        let mut declared = vec![];
        for i in 0..z.len() {
            let e = z.by_index_raw(i).map_err(|e| e.to_string())?;
            declared.push((e.name().map_err(|e| e.to_string())?.to_string(), e.size()));
        }
        let refs: Vec<(&str, u64)> = declared.iter().map(|(n, s)| (n.as_str(), *s)).collect();
        manifest::check_package(&refs).map_err(|e| e.to_string())?;
        let mut files: Vec<(String, Vec<u8>)> = vec![];
        for (i, (name, size)) in declared.iter().enumerate() {
            let mut b = vec![];
            z.by_index(i)
                .map_err(|e| e.to_string())?
                .take(size + 1)
                .read_to_end(&mut b)
                .map_err(|e| e.to_string())?;
            if b.len() as u64 != *size {
                return Err(format!("entry `{name}` is not the size it declares"));
            }
            files.push((name.clone(), b));
        }
        let get = |n: &str| {
            files
                .iter()
                .find(|(f, _)| f == n)
                .map(|(_, b)| b.as_slice())
                .unwrap_or_default()
        };
        let pkg = package_from(&self.user, get("plugin.toml"), get("plugin.wasm"))?;
        let (id, ver) = (&pkg.manifest.id, &pkg.manifest.version);
        // installed copies of this id: a different manifest is a new thing to consent to
        let old: Vec<PathBuf> = std::fs::read_dir(self.user.join(id))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        let changed = old
            .iter()
            .any(|d| read_package(d).map_or(true, |p| p.manifest_sha256 != pkg.manifest_sha256));
        let tmp = self.user.join(format!(".install-{}", std::process::id()));
        let target = self.user.join(id).join(ver);
        let _ = std::fs::remove_dir_all(&tmp);
        let write = || -> std::io::Result<()> {
            std::fs::create_dir_all(&tmp)?;
            for (n, b) in &files {
                std::fs::write(tmp.join(n), b)?;
            }
            write_record(
                &tmp,
                &Record {
                    sha256: pkg.sha256.clone(),
                    manifest_sha256: pkg.manifest_sha256.clone(),
                    signature: None,
                },
            )?;
            std::fs::create_dir_all(target.parent().unwrap_or(&self.user))?;
            let _ = std::fs::remove_dir_all(&target);
            std::fs::rename(&tmp, &target)
        };
        write().map_err(|e| {
            let _ = std::fs::remove_dir_all(&tmp);
            format!("cannot store the package: {e}")
        })?;
        for d in old.iter().filter(|d| **d != target) {
            let _ = std::fs::remove_dir_all(d);
        }
        if changed {
            set_enabled(&self.settings_dir, id, false).map_err(|e| e.to_string())?;
            clear_grants(&self.settings_dir, id).map_err(|e| e.to_string())?;
        }
        read_package(&target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::tests::tmp;
    use std::io::Write;

    const TOML: &str = "id = \"demo.plugin\"\nversion = \"1.0.0\"\napi = 1\napps = [\"docs\"]\npublisher = \"Test\"\n\
        [name]\nen = \"Demo\"\n[permissions]\ndocument = \"read\"\nnetwork = [\"https://api.example.org\"]\n";

    pub(crate) fn zip_of(path: &Path, entries: &[(&str, &[u8])]) {
        let mut w = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (n, b) in entries {
            w.start_file(*n, zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(b).unwrap();
        }
        w.finish().unwrap();
    }

    fn store(name: &str) -> (Store, PathBuf) {
        let d = tmp(name);
        let machine = d.join("machine");
        (
            Store::new(
                d.join("user"),
                Some(machine.join("plugins")),
                d.join("settings"),
            ),
            d,
        )
    }

    #[test]
    fn install_enable_grant_and_list() {
        let (s, d) = store("plg-install");
        let z = d.join("p.zip");
        zip_of(
            &z,
            &[
                ("plugin.toml", TOML.as_bytes()),
                ("plugin.wasm", b"\0asm\x01\0\0\0"),
            ],
        );
        let p = s.install_zip(&z).unwrap();
        assert!(p.dir.ends_with("demo.plugin/1.0.0") || p.dir.ends_with("demo.plugin\\1.0.0"));
        assert!(s.enabled().is_empty(), "installed is not enabled");
        s.set_enabled("demo.plugin", true).unwrap();
        assert_eq!(s.enabled().len(), 1);
        // a grant must be an address the manifest declares
        assert!(
            s.set_grant("demo.plugin", "https://evil.example.org", true)
                .is_err()
        );
        s.set_grant("demo.plugin", "https://api.example.org", true)
            .unwrap();
        let e = s.enabled();
        assert_eq!(e[0].grant, ["https://api.example.org"]);
        assert_eq!(granted_origins(&e), ["https://api.example.org"]);
        let desc = &local_descriptors(&e)[0];
        assert_eq!(desc["grant"][0], "https://api.example.org");
        assert_eq!(desc["wasm"], format!("/plugins/{}", p.sha256));
        assert_eq!(
            desc["manifest"]["permissions"]["network"][0],
            "https://api.example.org"
        );
        s.set_enabled("demo.plugin", false).unwrap();
        assert!(s.enabled().is_empty());
        assert!(s.list()[0].problem.is_none());
        assert!(s.set_enabled("nope.nope", true).is_err());
    }

    #[test]
    fn a_changed_package_is_refused() {
        let (s, d) = store("plg-changed");
        let z = d.join("p.zip");
        zip_of(
            &z,
            &[
                ("plugin.toml", TOML.as_bytes()),
                ("plugin.wasm", b"\0asm\x01\0\0\0"),
            ],
        );
        let p = s.install_zip(&z).unwrap();
        s.set_enabled("demo.plugin", true).unwrap();
        std::fs::write(p.dir.join("plugin.wasm"), b"\0asm\x01\0\0\0\0").unwrap();
        assert!(s.enabled().is_empty());
        let item = &s.list()[0];
        assert!(
            item.problem.as_deref().unwrap().contains("changed"),
            "{item:?}"
        );
        // the manifest counts too (a widened permission)
        let p = s.install_zip(&z).unwrap();
        std::fs::write(p.dir.join("plugin.toml"), TOML.replace("Demo", "Demo2")).unwrap();
        assert!(s.enabled().is_empty());
        // and a user package with no record
        std::fs::remove_file(p.dir.join(RECORD)).unwrap();
        assert!(s.list()[0].problem.as_deref().unwrap().contains("record"));
    }

    #[test]
    fn the_machine_policy_switches_install_and_grants_off() {
        let (s, d) = store("plg-policy");
        let z = d.join("p.zip");
        zip_of(
            &z,
            &[
                ("plugin.toml", TOML.as_bytes()),
                ("plugin.wasm", b"\0asm\x01\0\0\0"),
            ],
        );
        s.install_zip(&z).unwrap();
        s.set_enabled("demo.plugin", true).unwrap();
        s.set_grant("demo.plugin", "https://api.example.org", true)
            .unwrap();
        std::fs::create_dir_all(d.join("machine")).unwrap();
        let pol = d.join("machine/policy.toml");
        std::fs::write(&pol, "[plugins]\nuser_grants = false\n").unwrap();
        assert_eq!(
            s.policy(),
            Policy {
                user_install: true,
                user_grants: false
            }
        );
        assert!(
            s.enabled()[0].grant.is_empty(),
            "an existing user grant stops counting"
        );
        assert!(
            s.set_grant("demo.plugin", "https://api.example.org", true)
                .is_err()
        );
        s.set_grant("demo.plugin", "https://api.example.org", false)
            .unwrap();
        std::fs::write(&pol, "[plugins]\nuser_install = false\n").unwrap();
        let e = s.install_zip(&z).unwrap_err();
        assert!(e.contains("administrator"), "{e}");
        // a damaged policy file fails closed
        std::fs::write(&pol, "[plugins\n").unwrap();
        assert_eq!(
            s.policy(),
            Policy {
                user_install: false,
                user_grants: false
            }
        );
        std::fs::remove_file(&pol).unwrap();
        assert_eq!(s.policy(), Policy::default());
    }

    #[test]
    fn user_install_off_leaves_the_user_root_out() {
        let (s, d) = store("plg-useroff");
        let z = d.join("p.zip");
        zip_of(
            &z,
            &[
                ("plugin.toml", TOML.as_bytes()),
                ("plugin.wasm", b" asm   "),
            ],
        );
        s.install_zip(&z).unwrap();
        s.set_enabled("demo.plugin", true).unwrap();
        assert_eq!(s.enabled().len(), 1);
        std::fs::create_dir_all(d.join("machine")).unwrap();
        std::fs::write(
            d.join("machine/policy.toml"),
            "[plugins]
user_install = false
",
        )
        .unwrap();
        assert!(s.enabled().is_empty());
        assert!(s.list().is_empty());
    }

    #[test]
    fn a_new_manifest_resets_consent_and_drops_old_versions() {
        let (s, d) = store("plg-reinstall");
        let z = d.join("p.zip");
        let wasm: &[u8] = b" asm   ";
        zip_of(
            &z,
            &[("plugin.toml", TOML.as_bytes()), ("plugin.wasm", wasm)],
        );
        s.install_zip(&z).unwrap();
        s.set_enabled("demo.plugin", true).unwrap();
        s.set_grant("demo.plugin", "https://api.example.org", true)
            .unwrap();
        // the same package again keeps the consent
        s.install_zip(&z).unwrap();
        assert_eq!(s.enabled()[0].grant, ["https://api.example.org"]);
        // another version with another manifest: disabled, no grants, and the old folder is gone
        let t2 = TOML.replace("1.0.0", "0.9.0").replace("Demo", "Demo2");
        zip_of(&z, &[("plugin.toml", t2.as_bytes()), ("plugin.wasm", wasm)]);
        s.install_zip(&z).unwrap();
        assert!(s.enabled().is_empty());
        assert_eq!(s.list().len(), 1);
        assert!(
            s.state()
                .grants
                .get("demo.plugin")
                .is_none_or(|g| g.is_empty())
        );
        assert!(s.state().enabled.is_empty());
    }

    #[test]
    fn bad_packages_are_refused() {
        let (s, d) = store("plg-bad");
        let z = d.join("p.zip");
        let ok: (&str, &[u8]) = ("plugin.toml", TOML.as_bytes());
        for (what, entries) in [
            (
                "a path",
                vec![
                    ("../plugin.toml", TOML.as_bytes()),
                    ("plugin.wasm", &b"x"[..]),
                ],
            ),
            (
                "an extra file",
                vec![ok, ("plugin.wasm", b"x"), ("run.exe", b"x")],
            ),
            ("no wasm", vec![ok]),
            (
                "a bad manifest",
                vec![("plugin.toml", b"id = 1".as_slice()), ("plugin.wasm", b"x")],
            ),
        ] {
            zip_of(&z, &entries);
            assert!(s.install_zip(&z).is_err(), "{what}");
        }
        std::fs::write(&z, b"not a zip").unwrap();
        assert!(s.install_zip(&z).is_err());
        assert!(scan(&s.user).is_empty(), "nothing is left behind");
    }

    #[test]
    fn the_machine_folder_wins_and_needs_no_record() {
        let (s, d) = store("plg-machine");
        let m = s.machine.clone().unwrap().join("demo");
        std::fs::create_dir_all(&m).unwrap();
        std::fs::write(m.join("plugin.toml"), TOML).unwrap();
        std::fs::write(m.join("plugin.wasm"), b"\0asm\x01\0\0\0").unwrap();
        let z = d.join("p.zip");
        zip_of(
            &z,
            &[
                ("plugin.toml", TOML.as_bytes()),
                ("plugin.wasm", b"\0asm\x01\0\0\0\0"),
            ],
        );
        s.install_zip(&z).unwrap();
        s.set_enabled("demo.plugin", true).unwrap();
        let e = s.enabled();
        assert_eq!(e.len(), 1);
        assert!(e[0].package.dir.starts_with(s.machine.as_ref().unwrap()));
        assert!(read_wasm(&e[0].package).is_ok());
        std::fs::write(m.join("plugin.wasm"), b"changed").unwrap();
        assert!(read_wasm(&e[0].package).is_err());
    }
}
