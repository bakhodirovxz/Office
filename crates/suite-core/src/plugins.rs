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
//! - **Signatures (O4).** An optional `SIGNATURE` file: Ed25519 over [`signed_message`]. A malformed or non-matching one refuses the
//!   package everywhere; a valid one by a trusted key makes it "verified". Trusted keys: `brand.toml [plugins] publisher_keys`, the
//!   machine policy's `trusted_keys`, the user's `[plugins] trusted_keys` (not when the policy allows signed user packages only).
//!   `policy.toml [plugins] user_unsigned = false` refuses a user package that is not verified. The docserver takes verified only.
use crate::settings::{self, SetError};
use plugin_api::manifest::{self, Manifest};
use ring::signature::{ED25519, Ed25519KeyPair, KeyPair, UnparsedPublicKey};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// The install record inside a package folder (not part of the package itself).
pub const RECORD: &str = "install.toml";
pub const SIGNATURE: &str = "SIGNATURE";
const POLICY: &str = "policy.toml";
/// The first line of what a publisher signs; a new format gets a new line, so an old signature never verifies under it.
const SIGNED_V1: &[u8] = b"plugin-package-signature-v1\n";

fn hex(b: &[u8]) -> String {
    use std::fmt::Write;
    b.iter()
        .fold(String::with_capacity(b.len() * 2), |mut s, x| {
            let _ = write!(s, "{x:02x}");
            s
        })
}

fn sha_hex(b: &[u8]) -> String {
    hex(&Sha256::digest(b))
}

/// `n` bytes from exactly `2n` lower-case hex digits.
fn unhex(s: &str, n: usize) -> Option<Vec<u8>> {
    if s.len() != 2 * n || !s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return None;
    }
    (0..n)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok())
        .collect()
}

/// What a publisher signs (`docs/plugins.md` 9.1): the line `plugin-package-signature-v1`, then one line `<name> <sha256 hex>` for
/// `plugin.toml`, `plugin.wasm` and (when the package has it) `strings.tsv`, in that order, each ending in `\n`. The Android app
/// (`Plugins.java`) builds the same bytes.
pub fn signed_message(toml: &[u8], wasm: &[u8], strings: Option<&[u8]>) -> Vec<u8> {
    let mut m = SIGNED_V1.to_vec();
    for (n, b) in [
        ("plugin.toml", Some(toml)),
        ("plugin.wasm", Some(wasm)),
        ("strings.tsv", strings),
    ] {
        if let Some(b) = b {
            m.extend_from_slice(format!("{n} {}\n", sha_hex(b)).as_bytes());
        }
    }
    m
}

/// The `SIGNATURE` file, `ed25519 <public key: 64 hex> <signature: 128 hex>` and an optional line end, checked against the key it
/// names. `Ok(key)`: valid for that key (whether the key is trusted is decided by the caller).
fn check_signature(file: &[u8], msg: &[u8]) -> Result<String, String> {
    let bad = || "the SIGNATURE file is malformed".to_string();
    let t = std::str::from_utf8(file).map_err(|_| bad())?;
    let t = t
        .strip_suffix('\n')
        .map_or(t, |t| t.strip_suffix('\r').unwrap_or(t));
    let [alg, key, sig] = t.split(' ').collect::<Vec<_>>()[..] else {
        return Err(bad());
    };
    let (Some(k), Some(s)) = (unhex(key, 32), unhex(sig, 64)) else {
        return Err(bad());
    };
    if alg != "ed25519" {
        return Err(bad());
    }
    UnparsedPublicKey::new(&ED25519, &k)
        .verify(msg, &s)
        .map_err(|_| "the signature does not match the package".to_string())?;
    Ok(key.to_string())
}

/// A `SIGNATURE` file for `msg` ([`signed_message`]) made with the Ed25519 key whose 32-byte seed is `seed` (`cargo xt plugin sign`,
/// tests). The product never signs; this is here so that signing and checking share one definition.
pub fn signature_file(seed: &[u8], msg: &[u8]) -> Result<String, String> {
    let kp = Ed25519KeyPair::from_seed_unchecked(seed).map_err(|e| e.to_string())?;
    Ok(format!(
        "ed25519 {} {}\n",
        hex(kp.public_key().as_ref()),
        hex(kp.sign(msg).as_ref())
    ))
}

/// The publisher keys built into this product (`brand.toml [plugins] publisher_keys`, checked by build.rs).
pub fn builtin_keys() -> Vec<String> {
    env!("SUITE_PLUGIN_KEYS")
        .split(',')
        .filter(|k| !k.is_empty())
        .map(str::to_string)
        .collect()
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
    /// the public key (hex) of a valid `SIGNATURE`; `None`: unsigned. Trusted or not is [`trusted_by`]'s question.
    pub signer: Option<String>,
}

/// Is `p` signed by one of the built-in keys or one of `keys`?
pub fn trusted_by(p: &Package, keys: &[String]) -> bool {
    p.signer
        .as_ref()
        .is_some_and(|s| keys.contains(s) || builtin_keys().contains(s))
}

fn package_from(
    dir: &Path,
    toml: &[u8],
    wasm: &[u8],
    strings: Option<&[u8]>,
    sig: Option<&[u8]>,
) -> Result<Package, String> {
    if wasm.len() as u64 > manifest::MAX_WASM {
        return Err("plugin.wasm is over 4 MiB".into());
    }
    let text = std::str::from_utf8(toml).map_err(|_| "plugin.toml is not UTF-8")?;
    let (manifest, json) = manifest::parse_json(text).map_err(|e| e.to_string())?;
    if !plugin_api::supports(manifest.api) {
        return Err(format!("API {} is not supported", manifest.api));
    }
    let signer = sig
        .map(|s| check_signature(s, &signed_message(toml, wasm, strings)))
        .transpose()?;
    Ok(Package {
        dir: dir.to_path_buf(),
        manifest,
        json,
        sha256: sha_hex(wasm),
        manifest_sha256: sha_hex(toml),
        signer,
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
    let opt = |n: &str| {
        refs.iter()
            .any(|(e, _)| *e == n)
            .then(|| std::fs::read(dir.join(n)).map_err(io))
            .transpose()
    };
    let (strings, sig) = (opt("strings.tsv")?, opt(SIGNATURE)?);
    package_from(dir, &toml, &wasm, strings.as_deref(), sig.as_deref())
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
    })
}

fn write_record(dir: &Path, r: &Record) -> std::io::Result<()> {
    std::fs::write(
        dir.join(RECORD),
        format!(
            "sha256 = \"{}\"\nmanifest_sha256 = \"{}\"\n",
            r.sha256, r.manifest_sha256
        ),
    )
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
    /// `[plugins.settings.<id>]`: the user's values for a plugin's `[settings]` (they replace the manifest defaults, like an admin's).
    pub settings: BTreeMap<String, Value>,
    /// `[plugins] trusted_keys`: publisher keys (hex) the user (or the web host's operator) trusts
    pub keys: Vec<String>,
}

/// A TOML value as JSON (datetimes as text).
fn json_of(v: &toml_edit::Value) -> Value {
    use toml_edit::Value as T;
    match v {
        T::String(s) => Value::String(s.value().clone()),
        T::Integer(i) => (*i.value()).into(),
        T::Float(f) => serde_json::Number::from_f64(*f.value()).map_or(Value::Null, Value::Number),
        T::Boolean(b) => Value::Bool(*b.value()),
        T::Datetime(d) => Value::String(d.value().to_string()),
        T::Array(a) => Value::Array(a.iter().map(json_of).collect()),
        T::InlineTable(t) => {
            Value::Object(t.iter().map(|(k, v)| (k.to_string(), json_of(v))).collect())
        }
    }
}

fn json_of_item(i: &toml_edit::Item) -> Value {
    match i {
        toml_edit::Item::Value(v) => json_of(v),
        toml_edit::Item::Table(t) => Value::Object(
            t.iter()
                .map(|(k, v)| (k.to_string(), json_of_item(v)))
                .collect(),
        ),
        toml_edit::Item::ArrayOfTables(a) => Value::Array(
            a.iter()
                .map(|t| {
                    Value::Object(
                        t.iter()
                            .map(|(k, v)| (k.to_string(), json_of_item(v)))
                            .collect(),
                    )
                })
                .collect(),
        ),
        toml_edit::Item::None => Value::Null,
    }
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
        settings: p
            .get("settings")
            .and_then(|g| g.as_table())
            .map(|g| {
                g.iter()
                    .map(|(k, v)| (k.to_string(), json_of_item(v)))
                    .filter(|(_, v)| v.is_object())
                    .collect()
            })
            .unwrap_or_default(),
        keys: strings(p.get("trusted_keys")),
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

/// What a machine policy forbids (O3, O4). Everything is allowed unless the file says otherwise.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Policy {
    pub user_install: bool,
    pub user_grants: bool,
    /// `false`: a package in the user folder must be signed by a trusted key (O4)
    pub user_unsigned: bool,
    /// `trusted_keys`: publisher keys IT trusts (hex)
    pub keys: Vec<String>,
}

impl Default for Policy {
    fn default() -> Policy {
        Policy {
            user_install: true,
            user_grants: true,
            user_unsigned: true,
            keys: vec![],
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
        user_unsigned: false,
        keys: vec![],
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
        user_unsigned: flag("user_unsigned"),
        keys: strings(doc.get("plugins").and_then(|p| p.get("trusted_keys"))),
    }
}

/// O4 for one package: `Ok(true)` verified (signed by a trusted key), `Ok(false)` usable but not verified, `Err` refused. A machine
/// or operator folder may hold unverified packages (IT or the operator put them there); a user's must be verified when the policy
/// says `user_unsigned = false`, and then the user's own keys do not count.
pub fn admit(p: &Package, machine: bool, st: &State, pol: &Policy) -> Result<bool, String> {
    let user_keys: &[String] = if pol.user_unsigned { &st.keys } else { &[] };
    if trusted_by(p, &pol.keys) || trusted_by(p, user_keys) {
        Ok(true)
    } else if machine || pol.user_unsigned {
        Ok(false)
    } else {
        Err("the administrator allows only plugins signed by a trusted publisher".into())
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
    /// The user's or operator's `[settings]` values for this plugin (local mode); `None`: the manifest defaults.
    pub settings: Option<Value>,
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
            let p = match p.and_then(|p| {
                verify(&p, r.machine)?;
                admit(&p, r.machine, st, pol)?;
                Ok(p)
            }) {
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
                let settings = st.settings.get(&p.manifest.id).cloned().filter(|v| {
                    let ok = v.to_string().len() <= 4096;
                    if !ok {
                        eprintln!(
                            "plugins: {}: settings over 4096 bytes ignored",
                            p.manifest.id
                        );
                    }
                    ok
                });
                out.push(Enabled {
                    package: p,
                    grant,
                    settings,
                });
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
                e.settings.as_ref(),
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
    /// why it is not usable (refused, changed, no record, unsigned against the policy); `None` when it is
    pub problem: Option<String>,
    pub grant: Vec<String>,
    /// signed by a trusted publisher key
    pub verified: bool,
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
                        verified: false,
                    },
                    Ok(p) => {
                        let ok = admit(&p, r.machine, &st, &pol);
                        Item {
                            problem: verify(&p, r.machine).err().or(ok.clone().err()),
                            verified: ok == Ok(true),
                            enabled: st.enabled.contains(&p.manifest.id),
                            grant: effective_grant(&p, &st, &pol),
                            name,
                            machine: r.machine,
                            package: Some(p),
                        }
                    }
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
        };
        let pkg = package_from(
            &self.user,
            get("plugin.toml").unwrap_or_default(),
            get("plugin.wasm").unwrap_or_default(),
            get("strings.tsv"),
            get(SIGNATURE),
        )?;
        admit(&pkg, false, &self.state(), &self.policy())?;
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

    /// Removes the user's copy of `id` (every version, with its install record), turns it off and withdraws its grants; its
    /// `[plugins.settings.<id>]` values stay. A package in the machine folder is IT's and is not touched (`Err` when only it has the
    /// id). Allowed whatever the policy says, like withdrawing a grant. A plugin already loaded keeps running from memory until its
    /// editor reloads; nothing reads the folder during an invocation.
    pub fn uninstall(&self, id: &str) -> Result<(), String> {
        let folder = self.user.join(id);
        // the id must be one an installed package's (validated) manifest has: never a path from the page
        if !scan(&self.user).iter().any(|(d, p)| {
            d.parent() == Some(&folder) && p.as_ref().is_ok_and(|p| p.manifest.id == id)
        }) {
            let it_has = self
                .list()
                .iter()
                .any(|i| i.machine && i.package.as_ref().is_some_and(|p| p.manifest.id == id));
            return Err(if it_has {
                "this plugin was installed by your administrator".into()
            } else {
                "no such plugin".into()
            });
        }
        set_enabled(&self.settings_dir, id, false).map_err(|e| e.to_string())?;
        clear_grants(&self.settings_dir, id).map_err(|e| e.to_string())?;
        // out of sight first (scan skips dot folders), so a removal cut short never leaves a half package in the list
        let gone = self
            .user
            .join(format!(".uninstall-{}-{id}", std::process::id()));
        let _ = std::fs::remove_dir_all(&gone);
        std::fs::rename(&folder, &gone).map_err(|e| format!("cannot remove the plugin: {e}"))?;
        let _ = std::fs::remove_dir_all(&gone);
        Ok(())
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
        // the user's values for the plugin's [settings] replace its manifest defaults in the descriptor (local mode)
        assert!(desc.get("settings").is_none());
        let f = d.join("settings/settings.toml");
        let mut t = std::fs::read_to_string(&f).unwrap_or_default();
        t.push_str(
            "
[plugins.settings.\"demo.plugin\"]
fields = [{ name = \"field_z\", label = \"Sample zeta\", kind = \"text\" }]
n = 3
",
        );
        std::fs::write(&f, t).unwrap();
        let e = s.enabled();
        assert_eq!(
            local_descriptors(&e)[0]["settings"],
            serde_json::json!({"fields": [{"name": "field_z", "label": "Sample zeta", "kind": "text"}], "n": 3})
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
                user_grants: false,
                ..Policy::default()
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
                user_grants: false,
                user_unsigned: false,
                keys: vec![]
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

    // ---- O4 signatures and uninstall ----

    /// The published test keys (ci/plugins/signing, TEST ONLY).
    fn seed(name: &str) -> Vec<u8> {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../ci/plugins/signing/{name}.seed"));
        unhex(std::fs::read_to_string(p).unwrap().trim(), 32).unwrap()
    }

    fn key(name: &str) -> String {
        signature_file(&seed(name), b"")
            .unwrap()
            .split(' ')
            .nth(1)
            .unwrap()
            .to_string()
    }

    /// A package zip; `sig`: the key that signs it, and the wasm the signature is made for (to fake a tampered package).
    fn pkg(z: &Path, wasm: &[u8], sig: Option<(&str, &[u8])>) {
        let s = sig.map(|(k, w)| {
            signature_file(&seed(k), &signed_message(TOML.as_bytes(), w, None)).unwrap()
        });
        let mut e: Vec<(&str, &[u8])> =
            vec![("plugin.toml", TOML.as_bytes()), ("plugin.wasm", wasm)];
        if let Some(s) = &s {
            e.push((SIGNATURE, s.as_bytes()));
        }
        zip_of(z, &e);
    }

    fn policy(d: &Path, text: &str) {
        std::fs::create_dir_all(d.join("machine")).unwrap();
        std::fs::write(d.join("machine/policy.toml"), text).unwrap();
    }

    const WASM: &[u8] = b"\x00asm\x01\x00\x00\x00";

    #[test]
    fn rfc_8032_vectors_verify_and_a_changed_byte_does_not() {
        // RFC 8032 section 7.1, tests 1-3 (the Android verifier checks the same three)
        for (pk, msg, sig) in [
            (
                "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
                "",
                "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
            ),
            (
                "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
                "72",
                "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
            ),
            (
                "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
                "af82",
                "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a",
            ),
        ] {
            let m = unhex(msg, msg.len() / 2).unwrap();
            let file = format!("ed25519 {pk} {sig}\n");
            assert_eq!(check_signature(file.as_bytes(), &m).unwrap(), pk);
            let mut m2 = m.clone();
            m2.push(0);
            assert!(check_signature(file.as_bytes(), &m2).is_err());
        }
        // the test seed signs and its signature checks
        let msg = signed_message(b"t", b"w", Some(b"s"));
        let file = signature_file(&seed("test-a"), &msg).unwrap();
        assert_eq!(
            check_signature(file.as_bytes(), &msg).unwrap(),
            key("test-a")
        );
        assert_ne!(key("test-a"), key("test-b"));
        // what is signed covers strings.tsv when present
        assert_ne!(
            signed_message(b"t", b"w", None),
            signed_message(b"t", b"w", Some(b""))
        );
        let empty = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(
            String::from_utf8(signed_message(b"", b"", None)).unwrap(),
            format!("plugin-package-signature-v1\nplugin.toml {empty}\nplugin.wasm {empty}\n")
        );
    }

    #[test]
    fn a_signed_package_is_verified_and_a_bad_signature_is_refused() {
        let (s, d) = store("plg-sig");
        let z = d.join("p.zip");
        policy(
            &d,
            &format!("[plugins]\ntrusted_keys = [\"{}\"]\n", key("test-a")),
        );
        // good: signed by a key IT trusts
        pkg(&z, WASM, Some(("test-a", WASM)));
        let p = s.install_zip(&z).unwrap();
        assert_eq!(p.signer.as_deref(), Some(key("test-a").as_str()));
        let it = &s.list()[0];
        assert!(it.verified && it.problem.is_none(), "{it:?}");
        s.set_enabled("demo.plugin", true).unwrap();
        assert_eq!(s.enabled().len(), 1);
        // tampered: the signature was made for other wasm bytes
        pkg(&z, b"\x00asm\x01\x00\x00\x00\x00", Some(("test-a", WASM)));
        let e = s.install_zip(&z).unwrap_err();
        assert!(e.contains("does not match"), "{e}");
        // ... or changed on disk after the install, in a file the install record does not pin
        std::fs::write(p.dir.join("strings.tsv"), "id\ten\n").unwrap();
        assert!(s.enabled().is_empty());
        assert!(
            s.list()[0]
                .problem
                .as_deref()
                .unwrap()
                .contains("does not match")
        );
        std::fs::remove_file(p.dir.join("strings.tsv")).unwrap();
        assert_eq!(s.enabled().len(), 1);
        // wrong key, claimed: the file names the trusted key but test-b signed
        let b = signature_file(
            &seed("test-b"),
            &signed_message(TOML.as_bytes(), WASM, None),
        )
        .unwrap();
        let forged = b.replace(&key("test-b"), &key("test-a"));
        let with_sig = |z: &Path, sig: &str| {
            zip_of(
                z,
                &[
                    ("plugin.toml", TOML.as_bytes()),
                    ("plugin.wasm", WASM),
                    (SIGNATURE, sig.as_bytes()),
                ],
            )
        };
        with_sig(&z, &forged);
        assert!(s.install_zip(&z).unwrap_err().contains("does not match"));
        // the line end is optional, and may be CRLF (the same grammar as Plugins.java)
        for end in ["", "\r\n"] {
            with_sig(&z, &format!("{}{end}", b.trim_end()));
            assert!(s.install_zip(&z).is_ok(), "{end:?}");
        }
        // a malformed SIGNATURE
        for bad in [
            String::new(),
            "ed25519".into(),
            b.replace("ed25519", "rsa"),
            b.to_uppercase(),
            format!("{b}x"),
            b.replace(' ', "  "),
            format!("{b}\n"),
            format!(" {b}"),
            format!("{}\r", b.trim_end()),
        ] {
            with_sig(&z, &bad);
            assert!(
                s.install_zip(&z).unwrap_err().contains("malformed"),
                "{bad:?}"
            );
        }
        // wrong key, honest: a valid signature by a key nobody trusts is "not verified", like an unsigned package (local mode)
        pkg(&z, WASM, Some(("test-b", WASM)));
        s.install_zip(&z).unwrap();
        let it = &s.list()[0];
        assert!(!it.verified && it.problem.is_none(), "{it:?}");
        // the user may trust a key of their own in settings.toml
        std::fs::write(
            d.join("settings/settings.toml"),
            format!("[plugins]\ntrusted_keys = [\"{}\"]\n", key("test-b")),
        )
        .unwrap();
        assert!(s.list()[0].verified);
    }

    #[test]
    fn unsigned_installs_locally_unless_the_policy_forbids_it() {
        let (s, d) = store("plg-unsigned");
        let z = d.join("p.zip");
        // unsigned, local mode, no policy: installs, not verified (the page shows the warning)
        pkg(&z, WASM, None);
        s.install_zip(&z).unwrap();
        s.set_enabled("demo.plugin", true).unwrap();
        let it = &s.list()[0];
        assert!(!it.verified && it.problem.is_none());
        assert_eq!(s.enabled().len(), 1);
        // IT forbids unsigned user packages: the installed one stops working, a new install is refused
        policy(
            &d,
            &format!(
                "[plugins]\nuser_unsigned = false\ntrusted_keys = [\"{}\"]\n",
                key("test-a")
            ),
        );
        assert!(s.enabled().is_empty());
        assert!(s.list()[0].problem.as_deref().unwrap().contains("signed"));
        assert!(s.install_zip(&z).unwrap_err().contains("administrator"));
        // ... and the user's own trusted key does not count then
        std::fs::write(
            d.join("settings/settings.toml"),
            format!(
                "[plugins]\nenabled = [\"demo.plugin\"]\ntrusted_keys = [\"{}\"]\n",
                key("test-b")
            ),
        )
        .unwrap();
        pkg(&z, WASM, Some(("test-b", WASM)));
        assert!(s.install_zip(&z).is_err());
        // a package signed by IT's key installs and runs
        pkg(&z, WASM, Some(("test-a", WASM)));
        s.install_zip(&z).unwrap();
        assert_eq!(s.enabled().len(), 1);
        assert!(s.list()[0].verified);
        // IT's own machine folder may hold unsigned packages
        let m = s.machine.clone().unwrap().join("other");
        std::fs::create_dir_all(&m).unwrap();
        std::fs::write(
            m.join("plugin.toml"),
            TOML.replace("demo.plugin", "it.plugin"),
        )
        .unwrap();
        std::fs::write(m.join("plugin.wasm"), WASM).unwrap();
        let it = s.list().into_iter().find(|i| i.machine).unwrap();
        assert!(it.problem.is_none() && !it.verified);
        // a damaged policy fails closed for this too
        policy(&d, "[plugins\n");
        assert!(!s.policy().user_unsigned);
    }

    #[test]
    fn uninstall_removes_the_package_its_record_and_grants() {
        let (s, d) = store("plg-uninstall");
        let z = d.join("p.zip");
        pkg(&z, WASM, None);
        s.install_zip(&z).unwrap();
        s.set_enabled("demo.plugin", true).unwrap();
        s.set_grant("demo.plugin", "https://api.example.org", true)
            .unwrap();
        let f = d.join("settings/settings.toml");
        let mut t = std::fs::read_to_string(&f).unwrap();
        t.push_str("\n[plugins.settings.\"demo.plugin\"]\nn = 3\n");
        std::fs::write(&f, t).unwrap();
        // what a running editor holds: the loaded package
        let loaded = s.enabled();
        let bytes = read_wasm(&loaded[0].package).unwrap();
        s.uninstall("demo.plugin").unwrap();
        assert!(s.list().is_empty() && s.enabled().is_empty());
        assert!(
            !s.user.join("demo.plugin").exists(),
            "the folder and its install record are gone"
        );
        let st = s.state();
        assert!(st.enabled.is_empty());
        assert!(st.grants.get("demo.plugin").is_none_or(|g| g.is_empty()));
        assert!(
            st.settings.contains_key("demo.plugin"),
            "the user's values stay"
        );
        // the running copy keeps its bytes; nothing can load it again
        assert_eq!(bytes, WASM);
        assert!(read_wasm(&loaded[0].package).is_err());
        assert!(
            std::fs::read_dir(&s.user).unwrap().next().is_none(),
            "nothing is left behind"
        );
        // again, a path, an IT package
        assert_eq!(s.uninstall("demo.plugin").unwrap_err(), "no such plugin");
        assert_eq!(s.uninstall("../settings").unwrap_err(), "no such plugin");
        let m = s.machine.clone().unwrap().join("demo");
        std::fs::create_dir_all(&m).unwrap();
        std::fs::write(m.join("plugin.toml"), TOML).unwrap();
        std::fs::write(m.join("plugin.wasm"), WASM).unwrap();
        assert!(
            s.uninstall("demo.plugin")
                .unwrap_err()
                .contains("administrator")
        );
        assert!(m.join("plugin.wasm").exists());
    }
}
