//! `suite-desktop`, the logic of the desktop host (PLAN phase 4, section 3.4, WP9): the command line, the brand config, the file
//! checks, the IPC allowlist and the small on-disk store. It is portable and testable; the window, the server and the pipe are in
//! `host.rs` (Windows only).
//!
//! Trust boundary: the page is our own shell, but a message is still data. `ipc::parse` accepts only the types in `Request`, with
//! exactly their fields, in at most `MAX_MESSAGE` bytes. A path is checked again by `check_file` before anything is started, and
//! only an editor named in `brand.toml` is ever started.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::Mutex;

pub const MAX_MESSAGE: usize = 8 * 1024;
const HISTORY_CAP: usize = 200;
pub const LANGS: [&str; 4] = ["en", "ru", "uz-Latn", "uz-Cyrl"];

/// (brand.toml key, extensions the editor opens). The lists are those of PLAN section 7.2, which come from the engines.
const APPS: [(&str, &[&str]); 4] = [
    (
        "docs",
        &[
            "docx", "docm", "dotx", "dotm", "doc", "dot", "odt", "rtf", "txt", "md", "markdown",
            "html", "htm",
        ],
    ),
    (
        "sheets",
        &[
            "xlsx", "xlsm", "xltx", "xltm", "xls", "xlt", "xlsb", "ods", "csv", "tsv",
        ],
    ),
    ("slides", &["pptx", "potx", "ppsx"]),
    ("pdf", &["pdf"]),
];

pub struct AppDef {
    pub id: &'static str,
    /// the editor's exe name without `.exe`, from `brand.toml [apps.*] bin`
    pub bin: String,
    pub exts: &'static [&'static str],
}

pub struct Brand {
    pub product: String,
    pub id: String,
    pub apps: Vec<AppDef>,
}

impl Brand {
    /// No name is written in this crate: the product name and the editors' exe names come from the one brand config.
    pub fn parse(text: &str) -> Result<Brand, String> {
        let t: toml::Table = text.parse().map_err(|e| format!("brand.toml: {e}"))?;
        let s = |v: Option<&toml::Value>, what: &str| {
            v.and_then(|v| v.as_str())
                .map(str::to_string)
                .ok_or_else(|| format!("brand.toml: {what} is missing"))
        };
        let product = t.get("product");
        let mut apps = vec![];
        for (id, exts) in APPS {
            let bin = s(
                t.get("apps")
                    .and_then(|a| a.get(id))
                    .and_then(|a| a.get("bin")),
                &format!("apps.{id}.bin"),
            )?;
            // the bin name is joined into a path: a plain file name only (review m12)
            if !bin.starts_with(|c: char| c.is_ascii_alphanumeric())
                || !bin
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
            {
                return Err(format!(
                    "brand.toml: apps.{id}.bin must be letters, digits, '.', '_' or '-'"
                ));
            }
            apps.push(AppDef { id, bin, exts });
        }
        Ok(Brand {
            product: s(product.and_then(|p| p.get("name")), "product.name")?,
            id: s(product.and_then(|p| p.get("id")), "product.id")?,
            apps,
        })
    }
    pub fn app(&self, id: &str) -> Option<&AppDef> {
        self.apps.iter().find(|a| a.id == id)
    }
}

/// Where a path came from. A path from the page is the strictest: no network share (an `\\host\share` path makes Windows send
/// the user's credentials to that host). A path from the command line, the second-launch pipe or the native file dialog is the
/// user's own choice.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Source {
    Page,
    User,
}

/// A reserved DOS device name, with or without an extension (`NUL`, `com1.txt`, `LPT¹`): Windows opens the device, not a file.
fn dos_device(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    if matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) {
        return true;
    }
    let n = stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"));
    n.is_some_and(|n| {
        let mut c = n.chars();
        matches!((c.next(), c.next()), (Some(d), None) if ('1'..='9').contains(&d) || "¹²³".contains(d))
    })
}

/// A canonical path that leads to a network share (a directory symlink or junction to `\\host\share`).
fn is_network_target(canonical: &Path) -> bool {
    let s = canonical.to_string_lossy();
    s.starts_with(r"\\?\UNC\") || (s.starts_with(r"\\") && !s.starts_with(r"\\?\"))
}

/// The editor for `path` and the path itself, when it is a file we may open: absolute, no `..`, a known extension, exists.
pub fn check_file<'a>(
    brand: &'a Brand,
    path: &str,
    src: Source,
) -> Result<(&'a AppDef, PathBuf), String> {
    let p = Path::new(path);
    if path.is_empty() || path.len() > 32767 || path.contains('\0') {
        return Err("bad path".into());
    }
    let mut c = p.components();
    let ok_root = match c.next() {
        Some(Component::Prefix(pre)) => match pre.kind() {
            Prefix::Disk(_) | Prefix::VerbatimDisk(_) => true,
            Prefix::UNC(..) | Prefix::VerbatimUNC(..) => src == Source::User,
            Prefix::DeviceNS(_) | Prefix::Verbatim(_) => false,
        },
        _ => false,
    };
    if !ok_root || !p.is_absolute() || p.components().any(|c| c == Component::ParentDir) {
        return Err("the path must be absolute".into());
    }
    // no alternate data stream (`a.txt:s.docx`) and no DOS device (`COM1.docx`) after the prefix (review m1)
    for c in p.components().skip(1) {
        if let Component::Normal(n) = c {
            let n = n.to_string_lossy();
            if n.contains(':') || dos_device(&n) {
                return Err("this path is not allowed".into());
            }
        }
    }
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let app = brand
        .apps
        .iter()
        .find(|a| a.exts.contains(&ext.as_str()))
        .ok_or("this file type is not supported")?;
    if !p.is_file() {
        return Err("the file does not exist".into());
    }
    // a path from the page must not reach a share through a symlink or a junction either
    if src == Source::Page && std::fs::canonicalize(p).is_ok_and(|c| is_network_target(&c)) {
        return Err("the path must be absolute".into());
    }
    Ok((app, p.to_path_buf()))
}

// ---- the IPC: the only way a desktop action starts (never the HTTP port) ----

/// Every message type the host accepts. Anything else, and any extra field, is refused.
#[derive(Deserialize, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    /// open a file in its editor (a separate editor process, as today)
    Open {
        path: String,
    },
    /// the native file dialog, limited to one editor's types (`app`) or all; the chosen file is opened
    Pick {
        #[serde(default)]
        app: Option<String>,
    },
    Recent {},
    History {},
    SettingsGet {},
    SettingsSet {
        key: String,
        value: String,
    },
}

pub fn parse(body: &str) -> Result<Request, String> {
    if body.len() > MAX_MESSAGE {
        return Err("message too large".into());
    }
    // the serde message can quote the input: it is not passed on
    serde_json::from_str(body).map_err(|_| "unknown or malformed message".to_string())
}

/// A page may send IPC only from our own loopback origin (a navigation to anywhere else is blocked as well).
pub fn origin_ok(uri: &str, port: u16) -> bool {
    let o = format!("http://127.0.0.1:{port}");
    uri == o
        || uri
            .strip_prefix(&o)
            .is_some_and(|r| r.starts_with(['/', '?', '#']))
}

/// What the window may navigate to: our origin, a `blob:` of it (the editors save by a download link) and `about:blank`.
pub fn nav_ok(url: &str, port: u16) -> bool {
    url == "about:blank"
        || origin_ok(url, port)
        || url
            .strip_prefix("blob:")
            .is_some_and(|r| origin_ok(r, port))
}

/// One message from the page, with the address of the document that sent it. A document of any other origin gets no answer at all
/// and nothing runs (review m13).
pub fn ipc_message(ctx: &Ctx, uri: &str, port: u16, body: &str) -> Option<Out> {
    origin_ok(uri, port).then(|| ctx.message(body))
}

/// Where a download may be written: the folder WebView2 chose, a file type of one of our apps, no stream or device name, and never
/// over an existing file (`name (1).ext`). A refusal cancels the download (review m6, m2).
pub fn download_target(brand: &Brand, wanted: &Path) -> Result<PathBuf, String> {
    let refused = || "this file type cannot be saved".to_string();
    let name = wanted
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(refused)?;
    let (stem, ext) = (
        Path::new(name).file_stem().and_then(|s| s.to_str()),
        Path::new(name).extension().and_then(|e| e.to_str()),
    );
    let (Some(stem), Some(ext)) = (stem, ext) else {
        return Err(refused());
    };
    let known = brand
        .apps
        .iter()
        .any(|a| a.exts.contains(&ext.to_ascii_lowercase().as_str()));
    let dir = wanted.parent().filter(|d| d.is_absolute());
    let (true, Some(dir)) = (known && !name.contains(':') && !dos_device(name), dir) else {
        return Err(refused());
    };
    (0..1000)
        .map(|n| match n {
            0 => dir.join(name),
            n => dir.join(format!("{stem} ({n}).{ext}")),
        })
        .find(|f| !f.exists())
        .ok_or_else(|| "too many files with this name".to_string())
}

/// The script that delivers a reply to the page: `addEventListener('suite-host', e => e.detail)`.
pub fn reply_script(v: &Value) -> String {
    format!("window.dispatchEvent(new CustomEvent('suite-host',{{detail:{v}}}))")
}

// ---- the store: <data dir>/history.json (opened files, newest last) and settings.json ----

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Entry {
    pub path: String,
    pub app: String,
    pub ts: u64,
}

pub struct Store {
    dir: PathBuf,
    lock: Mutex<()>,
}

impl Store {
    pub fn new(dir: PathBuf) -> Store {
        Store {
            dir,
            lock: Mutex::new(()),
        }
    }
    fn read<T: for<'a> Deserialize<'a> + Default>(&self, name: &str) -> T {
        std::fs::read_to_string(self.dir.join(name))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
    fn write(&self, name: &str, v: &impl Serialize) {
        let _ = std::fs::create_dir_all(&self.dir);
        let tmp = self.dir.join(format!("{name}.tmp"));
        if std::fs::write(&tmp, serde_json::to_vec(v).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, self.dir.join(name));
        }
    }
    pub fn record(&self, path: &Path, app: &str, ts: u64) {
        let _g = self.lock.lock();
        let mut h: Vec<Entry> = self.read("history.json");
        h.push(Entry {
            path: path.to_string_lossy().into_owned(),
            app: app.into(),
            ts,
        });
        let drop = h.len().saturating_sub(HISTORY_CAP);
        h.drain(..drop);
        self.write("history.json", &h);
    }
    /// Every open, newest first.
    pub fn history(&self) -> Vec<Entry> {
        let mut h: Vec<Entry> = self.read("history.json");
        h.reverse();
        h
    }
    /// Whether the host itself has opened or saved this exact path before.
    pub fn knows(&self, path: &str) -> bool {
        let h: Vec<Entry> = self.read("history.json");
        h.iter().any(|e| e.path == path)
    }
    /// The last `n` different files that still exist, newest first.
    pub fn recent(&self, n: usize) -> Vec<Entry> {
        let mut seen = std::collections::HashSet::new();
        self.history()
            .into_iter()
            .filter(|e| seen.insert(e.path.clone()) && Path::new(&e.path).is_file())
            .take(n)
            .collect()
    }
    pub fn settings(&self) -> std::collections::BTreeMap<String, String> {
        self.read("settings.json")
    }
    /// Only the keys in the allowlist, with their allowed values.
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        match key {
            "lang" if LANGS.contains(&value) => {}
            _ => return Err("unknown setting or value".into()),
        }
        let _g = self.lock.lock();
        let mut s = self.settings();
        s.insert(key.into(), value.into());
        self.write("settings.json", &s);
        Ok(())
    }
}

// ---- the actions ----

/// What the host does for the platform: starts an editor. Replaced in tests.
pub type Spawn = Box<dyn Fn(&Path, &Path) -> std::io::Result<()> + Send + Sync>;

pub struct Ctx {
    pub brand: Brand,
    pub store: Store,
    /// the folder with the editor exes
    pub bin_dir: PathBuf,
    pub spawn: Spawn,
}

pub enum Out {
    Reply(Value),
    /// the host must show the file dialog for these extensions (it blocks, so not on the UI thread)
    Pick(Vec<&'static str>),
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Ctx {
    /// A file the host wrote (a download) is recorded like an open, so Recent lists it. The reply carries the file name only.
    pub fn saved(&self, path: &Path) -> Value {
        let go = || -> Result<Value, String> {
            let (app, p) = check_file(&self.brand, &path.to_string_lossy(), Source::User)?;
            self.store.record(&p, app.id, now());
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned());
            Ok(json!({"type": "saved", "ok": true, "app": app.id, "name": name}))
        };
        go().unwrap_or_else(|error| json!({"type": "saved", "ok": false, "error": error}))
    }

    /// Checks the file, starts its editor and records it. The reply carries the file name only. The page may reopen only a file
    /// the host already knows (history); a path from the command line, the second launch or the native dialog is the user's own.
    pub fn open(&self, path: &str, src: Source) -> Value {
        let go = || -> Result<Value, String> {
            if src == Source::Page && !self.store.knows(path) {
                return Err("this file is not in the recent list".into());
            }
            let (app, p) = check_file(&self.brand, path, src)?;
            let exe = self.bin_dir.join(format!("{}.exe", app.bin));
            if !exe.is_file() {
                return Err(format!("the editor {}.exe is not installed", app.bin));
            }
            (self.spawn)(&exe, &p).map_err(|e| format!("cannot start the editor: {e}"))?;
            self.store.record(&p, app.id, now());
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok(json!({"type": "opened", "ok": true, "app": app.id, "name": name}))
        };
        go().unwrap_or_else(|error| json!({"type": "opened", "ok": false, "error": error}))
    }

    pub fn handle(&self, req: Request) -> Out {
        let items = |v: Vec<Entry>| {
            v.into_iter()
                .map(|e| json!({"path": e.path, "app": e.app, "ts": e.ts}))
                .collect::<Vec<_>>()
        };
        Out::Reply(match req {
            Request::Open { path } => self.open(&path, Source::Page),
            Request::Pick { app } => {
                let exts: Vec<&'static str> = match &app {
                    None => self
                        .brand
                        .apps
                        .iter()
                        .flat_map(|a| a.exts.iter().copied())
                        .collect(),
                    Some(id) => match self.brand.app(id) {
                        Some(a) => a.exts.to_vec(),
                        None => {
                            return Out::Reply(json!({"type": "rejected", "error": "unknown app"}));
                        }
                    },
                };
                return Out::Pick(exts);
            }
            Request::Recent {} => json!({"type": "recent", "items": items(self.store.recent(20))}),
            Request::History {} => json!({"type": "history", "items": items(self.store.history())}),
            Request::SettingsGet {} => json!({"type": "settings", "values": self.store.settings()}),
            Request::SettingsSet { key, value } => match self.store.set_setting(&key, &value) {
                Ok(()) => json!({"type": "settings", "values": self.store.settings()}),
                Err(error) => json!({"type": "rejected", "error": error}),
            },
        })
    }

    /// One message from the page: parse, then act. A refusal is a reply, never a panic or a silent drop.
    pub fn message(&self, body: &str) -> Out {
        match parse(body) {
            Ok(r) => self.handle(r),
            Err(error) => Out::Reply(json!({"type": "rejected", "error": error})),
        }
    }
}

// ---- the command line ----

#[derive(Default, Debug, PartialEq)]
pub struct Args {
    pub dist: Option<PathBuf>,
    pub shell: Option<PathBuf>,
    pub brand: Option<PathBuf>,
    pub bin_dir: Option<PathBuf>,
    /// open the default browser instead of a window
    pub browser: bool,
    pub files: Vec<String>,
}

pub const USAGE: &str = "usage: suite [--dist DIR] [--shell DIR] [--brand FILE] [--bin-dir DIR] [--browser] [--] [FILE...]";

/// Anything that is not a flag is a file, so `suite.exe "C:\Users\Имя\Hisobot 2026.xlsx"` works as "Open with" calls it.
pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut a = Args::default();
    let mut it = args.into_iter();
    let mut flags = true;
    while let Some(x) = it.next() {
        if !flags || !x.starts_with("--") {
            a.files.push(x);
            continue;
        }
        let mut val = || {
            it.next()
                .map(PathBuf::from)
                .ok_or_else(|| USAGE.to_string())
        };
        match x.as_str() {
            "--" => flags = false,
            "--browser" => a.browser = true,
            "--dist" => a.dist = Some(val()?),
            "--shell" => a.shell = Some(val()?),
            "--brand" => a.brand = Some(val()?),
            "--bin-dir" => a.bin_dir = Some(val()?),
            _ => return Err(USAGE.into()),
        }
    }
    Ok(a)
}

#[cfg(windows)]
mod host;
/// Runs the host: the window (or the browser fallback), the in-process server and the single-instance pipe.
#[cfg(windows)]
pub use host::run;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex as M};

    const BRAND: &str = r#"
[product]
id = "tst"
name = "Tst"
[apps.docs]
bin = "tst-docs"
[apps.sheets]
bin = "tst-sheets"
[apps.slides]
bin = "tst-slides"
[apps.pdf]
bin = "tst-pdf"
"#;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("suite-desktop-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    type Log = Arc<M<Vec<(PathBuf, PathBuf)>>>;

    fn ctx(name: &str) -> (Ctx, PathBuf, Log) {
        let d = tmp(name);
        let bin = d.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        for b in ["tst-docs", "tst-sheets", "tst-slides", "tst-pdf"] {
            std::fs::write(bin.join(format!("{b}.exe")), b"x").unwrap();
        }
        let log = Arc::new(M::new(vec![]));
        let l = log.clone();
        let c = Ctx {
            brand: Brand::parse(BRAND).unwrap(),
            store: Store::new(d.join("data")),
            bin_dir: bin,
            spawn: Box::new(move |e, f| {
                l.lock().unwrap().push((e.to_path_buf(), f.to_path_buf()));
                Ok(())
            }),
        };
        (c, d, log)
    }

    fn reply(o: Out) -> Value {
        match o {
            Out::Reply(v) => v,
            Out::Pick(_) => panic!("pick"),
        }
    }

    #[test]
    fn an_unknown_message_type_is_rejected() {
        let (c, _d, log) = ctx("unknown");
        for body in [
            r#"{"type":"exec","cmd":"calc.exe"}"#,
            r#"{"type":"open"}"#,
            r#"{"type":"open","path":"C:\\a.docx","extra":1}"#,
            r#"{"type":"recent","x":1}"#,
            r#"{"type":"Open","path":"C:\\a.docx"}"#,
            r#"[]"#,
            r#"{"path":"C:\\a.docx"}"#,
            "not json",
            "",
        ] {
            let v = reply(c.message(body));
            assert_eq!(v["type"], "rejected", "{body}");
        }
        assert!(log.lock().unwrap().is_empty(), "nothing was started");
        let big = format!(r#"{{"type":"open","path":"{}"}}"#, "a".repeat(MAX_MESSAGE));
        assert_eq!(reply(c.message(&big))["type"], "rejected");
    }

    #[test]
    fn only_our_own_origin_may_send() {
        assert!(origin_ok("http://127.0.0.1:5000/", 5000));
        assert!(origin_ok("http://127.0.0.1:5000", 5000));
        assert!(origin_ok("http://127.0.0.1:5000/docs/?host=parent", 5000));
        for u in [
            "http://127.0.0.1:50001/",
            "http://127.0.0.1:5000.evil.com/",
            "http://127.0.0.1:5000@evil.com/",
            "https://127.0.0.1:5000/",
            "http://localhost:5000/",
            "http://evil.com/",
            "",
        ] {
            assert!(!origin_ok(u, 5000), "{u}");
        }
    }

    #[test]
    fn navigation_stays_on_our_origin() {
        assert!(
            nav_ok("http://127.0.0.1:5000/docs/", 5000)
                && nav_ok("blob:http://127.0.0.1:5000/abc", 5000)
        );
        for u in [
            "https://example.com/",
            "file:///C:/x",
            "blob:https://example.com/a",
            "javascript:alert(1)",
            "http://127.0.0.1:1/",
        ] {
            assert!(!nav_ok(u, 5000), "{u}");
        }
    }

    #[test]
    fn open_checks_the_file_and_starts_the_brand_editor() {
        let (c, d, log) = ctx("open");
        // spaces and Cyrillic, as "Open with" passes them
        let dir = d.join("Рабочий стол");
        std::fs::create_dir_all(&dir).unwrap();
        let doc = dir.join("Буйруқ 1.DOCX");
        let sheet = dir.join("Hisobot 2026.xlsx");
        let txt = dir.join("a.exe");
        for f in [&doc, &sheet, &txt] {
            std::fs::write(f, b"x").unwrap();
        }
        // the page may reopen only what the host has opened before (review m1): the first open is the user's
        let other = dir.join("other.docx");
        std::fs::write(&other, b"x").unwrap();
        let refused = c.open(doc.to_str().unwrap(), Source::Page);
        assert!(
            refused["error"].as_str().unwrap().contains("recent list"),
            "{refused}"
        );
        let ok = c.open(doc.to_str().unwrap(), Source::User);
        assert_eq!(
            (ok["ok"].clone(), ok["app"].clone(), ok["name"].clone()),
            (true.into(), "docs".into(), "Буйруқ 1.DOCX".into())
        );
        assert_eq!(
            c.open(sheet.to_str().unwrap(), Source::User)["app"],
            "sheets"
        );
        assert_eq!(c.open(doc.to_str().unwrap(), Source::Page)["ok"], true);
        assert_eq!(c.open(other.to_str().unwrap(), Source::Page)["ok"], false);
        let l = log.lock().unwrap();
        assert!(l[0].0.ends_with("tst-docs.exe") && l[0].1 == doc);
        assert!(l[1].0.ends_with("tst-sheets.exe"));
        drop(l);
        for bad in [
            txt.to_str().unwrap(),
            "relative.docx",
            "C:\\no\\such\\file.docx",
            "..\\a.docx",
            "",
            "C:\\x\\..\\y.docx",
        ] {
            assert_eq!(c.open(bad, Source::Page)["ok"], false, "{bad}");
            assert_eq!(c.open(bad, Source::User)["ok"], false, "{bad}");
        }
        assert_eq!(log.lock().unwrap().len(), 3);
        // recent and history record every open, newest first
        let r = reply(c.handle(Request::Recent {}));
        assert_eq!(r["items"][0]["app"], "docs");
        assert_eq!(
            reply(c.handle(Request::History {}))["items"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }

    /// Review m1: an alternate data stream and a DOS device name are refused from every source, before anything is opened.
    #[test]
    #[cfg(windows)]
    fn streams_and_device_names_are_refused() {
        let (c, d, log) = ctx("streams");
        let f = d.join("a.txt");
        std::fs::write(&f, b"x").unwrap();
        for bad in [
            format!("{}:s.docx", f.display()),
            format!("{}:s:$DATA.docx", f.display()),
            d.join("COM1.docx").display().to_string(),
            d.join("nul.xlsx").display().to_string(),
            d.join("Lpt3 .pdf").display().to_string(),
            d.join("COM\u{b9}.docx").display().to_string(),
            d.join("CON.docx").join("x.docx").display().to_string(),
        ] {
            for src in [Source::Page, Source::User] {
                let v = c.open(&bad, src);
                assert_eq!(v["ok"], false, "{bad}");
                if src == Source::User {
                    assert_eq!(v["error"], "this path is not allowed", "{bad}");
                }
            }
        }
        assert!(log.lock().unwrap().is_empty());
        assert!(!dos_device("COM10.docx") && !dos_device("console.docx") && dos_device("AUX"));
        // a canonical path that leads to a share
        assert!(is_network_target(Path::new(r"\\?\UNC\srv\share\a.docx")));
        assert!(!is_network_target(Path::new(r"\\?\C:\Users\a.docx")));
    }

    #[test]
    fn a_page_of_another_origin_cannot_trigger_an_ipc_action() {
        let (c, d, log) = ctx("xorigin");
        let f = d.join("a.docx");
        std::fs::write(&f, b"x").unwrap();
        c.open(f.to_str().unwrap(), Source::User); // known to the host, so the page could reopen it
        let n = log.lock().unwrap().len();
        let open = serde_json::json!({"type": "open", "path": f}).to_string();
        let msgs = [open.as_str(), r#"{"type":"history"}"#, r#"{"type":"pick"}"#];
        for uri in [
            "http://evil.example/",
            "http://127.0.0.1:50001/",
            "http://127.0.0.1:5000.evil.com/",
            "http://127.0.0.1:5000@evil.com/",
            "https://127.0.0.1:5000/",
            "blob:http://127.0.0.1:5000/x",
            "about:blank",
            "file:///C:/x.html",
            "",
        ] {
            for m in msgs {
                assert!(ipc_message(&c, uri, 5000, m).is_none(), "{uri}");
            }
        }
        assert_eq!(log.lock().unwrap().len(), n, "nothing was started");
        // the same message from our own origin is answered
        let own = ipc_message(&c, "http://127.0.0.1:5000/docs/?host=parent", 5000, msgs[1]);
        assert!(matches!(own, Some(Out::Reply(v)) if v["type"] == "history"));
    }

    /// Review M2, m2, m6: a download goes to a known file type under a name that is not taken.
    #[test]
    fn a_download_target_is_checked_and_never_overwrites() {
        let (c, d, _log) = ctx("download");
        let want = d.join("Hisobot 2026.XLSX");
        assert_eq!(download_target(&c.brand, &want).unwrap(), want);
        std::fs::write(&want, b"x").unwrap();
        std::fs::write(d.join("Hisobot 2026 (1).XLSX"), b"x").unwrap();
        assert_eq!(
            download_target(&c.brand, &want).unwrap(),
            d.join("Hisobot 2026 (2).XLSX")
        );
        for bad in [
            "x.lnk",
            "x.hta",
            "x.exe",
            "x",
            "x.docx.exe",
            "NUL.docx",
            "a:b.docx",
        ] {
            assert!(download_target(&c.brand, &d.join(bad)).is_err(), "{bad}");
        }
        assert!(download_target(&c.brand, Path::new("rel.docx")).is_err());
        // a saved file is listed in Recent like an open
        let v = c.saved(&want);
        assert_eq!(
            (v["ok"].clone(), v["name"].clone()),
            (true.into(), "Hisobot 2026.XLSX".into())
        );
        assert_eq!(
            reply(c.handle(Request::Recent {}))["items"][0]["app"],
            "sheets"
        );
        assert_eq!(c.saved(&d.join("gone.docx"))["ok"], false);
    }

    #[test]
    fn the_brand_bin_name_must_be_a_plain_file_name() {
        // the Rust strings are TOML text: `\\\\` is one backslash in the value
        for bin in ["..\\\\evil", "a/b", "a b", "", "x:y", "a\\\\b", ".."] {
            let t = BRAND.replacen("tst-docs", bin, 1);
            assert!(Brand::parse(&t).is_err(), "{bin}");
        }
        assert!(Brand::parse(&BRAND.replacen("tst-docs", "My_Docs-2.v1", 1)).is_ok());
    }

    #[test]
    #[cfg(windows)]
    fn a_network_path_is_refused_from_the_page_only() {
        let b = Brand::parse(BRAND).unwrap();
        for p in [r"\\server\share\a.docx", r"\\?\UNC\server\share\a.docx"] {
            assert!(check_file(&b, p, Source::Page).is_err());
            // allowed by the rule, then it does not exist
            assert_eq!(
                check_file(&b, p, Source::User).err().as_deref(),
                Some("the file does not exist")
            );
        }
        assert!(check_file(&b, r"\\.\pipe\x.docx", Source::User).is_err());
        assert!(check_file(&b, r"\\.\C:\x.docx", Source::User).is_err());
        assert!(check_file(&b, r"C:a.docx", Source::User).is_err());
        assert!(check_file(&b, r"\a.docx", Source::User).is_err());
    }

    #[test]
    fn missing_editor_and_pick_and_settings() {
        let (c, d, _log) = ctx("misc");
        let f = d.join("a.pdf");
        std::fs::write(&f, b"x").unwrap();
        std::fs::remove_file(c.bin_dir.join("tst-pdf.exe")).unwrap();
        let v = c.open(f.to_str().unwrap(), Source::User);
        assert_eq!(v["ok"], false);
        assert!(v["error"].as_str().unwrap().contains("not installed"));
        match c.handle(Request::Pick {
            app: Some("pdf".into()),
        }) {
            Out::Pick(e) => assert_eq!(e, ["pdf"]),
            _ => panic!(),
        }
        assert_eq!(
            reply(c.handle(Request::Pick {
                app: Some("exe".into())
            }))["type"],
            "rejected"
        );
        let set = |k: &str, v: &str| {
            reply(c.handle(Request::SettingsSet {
                key: k.into(),
                value: v.into(),
            }))
        };
        assert_eq!(set("lang", "ru")["values"]["lang"], "ru");
        for (k, v) in [("lang", "xx"), ("cmd", "calc"), ("lang", "")] {
            assert_eq!(set(k, v)["type"], "rejected");
        }
        assert_eq!(
            reply(c.handle(Request::SettingsGet {}))["values"]["lang"],
            "ru"
        );
    }

    #[test]
    fn the_command_line() {
        let s = |v: &[&str]| parse_args(v.iter().map(|s| s.to_string()));
        let a = s(&[
            "--dist",
            "D",
            r"C:\Users\Имя\Hisobot 2026.xlsx",
            "--browser",
            "--",
            "--odd.docx",
        ])
        .unwrap();
        assert_eq!(a.dist, Some("D".into()));
        assert!(a.browser);
        assert_eq!(a.files, [r"C:\Users\Имя\Hisobot 2026.xlsx", "--odd.docx"]);
        assert!(s(&["--nope"]).is_err());
        assert!(s(&["--dist"]).is_err());
        assert_eq!(s(&[]).unwrap(), Args::default());
    }

    #[test]
    fn history_is_capped() {
        let d = tmp("cap");
        let s = Store::new(d.clone());
        for i in 0..HISTORY_CAP + 5 {
            s.record(&d.join(format!("{i}.docx")), "docs", i as u64);
        }
        let h = s.history();
        assert_eq!((h.len(), h[0].ts), (HISTORY_CAP, (HISTORY_CAP + 4) as u64));
    }
}
