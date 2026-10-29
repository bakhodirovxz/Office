//! `suite-desktop`, the logic of the desktop host (PLAN phase 4, section 3.4, WP9): the command line, the brand config, the file
//! checks, the IPC allowlist and the small on-disk store. It is portable and testable; the window, the server and the pipe are in
//! `host.rs` (Windows only).
//!
//! Trust boundary: the page is our own shell, but a message is still data. `ipc::parse` accepts only the types in `Request`, with
//! exactly their fields, in at most `MAX_MESSAGE` bytes. A path is checked again by `check_file` before anything is started, and
//! only an editor named in `brand.toml` is ever started.
pub mod guard;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::Mutex;

pub const MAX_MESSAGE: usize = 8 * 1024;
const HISTORY_CAP: usize = 200;
/// The settings a page may read and change. The rest of `settings.toml` (`server.url` decides where the shell navigates,
/// `spell_dictionaries` which folder is read) is not the page's to set. Each value is checked by `suite_core::settings::validate`.
pub const PAGE_SETTINGS: [&str; 4] = ["lang", "toolbar", "latin_alphabet", "theme"];

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
    /// start Excel on a workbook the guard stopped (or warned about); only a file in the history, only a Sheets file
    OpenInExcel {
        path: String,
    },
    Recent {},
    /// the documents that have saved versions (WP10)
    History {},
    HistoryVersions {
        path: String,
    },
    /// a read-only copy of a version, opened in its editor
    HistoryView {
        path: String,
        v: u64,
    },
    HistoryRestore {
        path: String,
        v: u64,
    },
    /// a copy of a version in the downloads folder
    HistoryCopy {
        path: String,
        v: u64,
    },
    HistoryName {
        path: String,
        v: u64,
        name: String,
    },
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

/// WebView2 reads these from the environment and they beat what the host sets: extra browser arguments can open an unauthenticated
/// remote-debugging port on loopback, and a browser folder picks the executable (gate review F15). The first one `is_set` says is
/// set. Only a release build asks; the debug build's checks use the arguments on purpose.
pub fn webview2_env_override(is_set: impl Fn(&str) -> bool) -> Option<&'static str> {
    [
        "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
        "WEBVIEW2_BROWSER_EXECUTABLE_FOLDER",
    ]
    .into_iter()
    .find(|k| is_set(k))
}

/// The shell route that shows the History panel of `path` (percent-encoded, only unreserved characters stay).
pub fn history_fragment(path: &str) -> String {
    let enc: String = path
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect();
    format!("#history={enc}")
}

/// The script that delivers a reply to the page: `addEventListener('suite-host', e => e.detail)`.
pub fn reply_script(v: &Value) -> String {
    format!("window.dispatchEvent(new CustomEvent('suite-host',{{detail:{v}}}))")
}

// ---- the store: <data dir>/history.json (opened files, newest last); the settings are the shared settings.toml ----

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Entry {
    pub path: String,
    pub app: String,
    pub ts: u64,
}

pub struct Store {
    dir: PathBuf,
    /// the folder of the shared `settings.toml` (`suite_core::settings::dir()` in the real host)
    settings_dir: PathBuf,
    lock: Mutex<()>,
}

impl Store {
    /// The settings live in `dir` too (tests and the debug data-dir hook stay inside one folder).
    pub fn new(dir: PathBuf) -> Store {
        Store::with_settings_dir(dir.clone(), dir)
    }
    pub fn with_settings_dir(dir: PathBuf, settings_dir: PathBuf) -> Store {
        Store {
            dir,
            settings_dir,
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
    /// The page-visible settings, from the shared `settings.toml` (defaults when it is missing or damaged).
    pub fn settings(&self) -> std::collections::BTreeMap<String, String> {
        let all = suite_core::settings::load_in(&self.settings_dir)
            .settings
            .to_map();
        PAGE_SETTINGS
            .iter()
            .filter_map(|k| all.get(k).map(|v| (k.to_string(), v.clone())))
            .collect()
    }
    /// Only the keys in `PAGE_SETTINGS`, with a value `suite-core` accepts.
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        if !PAGE_SETTINGS.contains(&key) {
            return Err("unknown setting or value".into());
        }
        let _g = self.lock.lock();
        suite_core::settings::set_in(&self.settings_dir, key, value).map_err(|e| match e {
            suite_core::settings::SetError::Io(_) => "the settings file cannot be written".into(),
            _ => "unknown setting or value".into(),
        })
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
    /// the local version history (`None`: unavailable, every history message then answers with an error)
    pub history: Option<HStore>,
    /// where "save a copy" of a version goes
    pub downloads: PathBuf,
}

type HStore = suite_core::history::Store;

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
            // an editor in the window saved: that is a version too (an error is not the download's problem)
            // a View copy (temp, deleted at the next start) is not a document with a history (F7)
            if let Some(h) = self
                .history
                .as_ref()
                .filter(|_| !p.starts_with(self.store.dir.join("temp")))
            {
                let _ = h.snapshot(&p, None);
            }
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
            // a file the host knows is the user's own, also on a mapped drive (F10)
            let (app, p) = check_file(&self.brand, path, Source::User)?;
            let exe = self.bin_dir.join(format!("{}.exe", app.bin));
            if !exe.is_file() {
                return Err(format!("the editor {}.exe is not installed", app.bin));
            }
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            // F18: a workbook over the wall is never handed to Sheets to fail silently; external links are said, not hidden
            let verdict = guard::check(app.id, &p);
            if verdict == guard::Verdict::Wall {
                // listed in Recent, so the page can offer Excel for it later
                self.store.record(&p, app.id, now());
                return Ok(
                    json!({"type": "opened", "ok": false, "error": guard::WALL_TEXT, "guard": "wall", "excel": guard::excel_exe().is_some(), "app": app.id, "name": name, "path": path}),
                );
            }
            (self.spawn)(&exe, &p).map_err(|e| format!("cannot start the editor: {e}"))?;
            self.store.record(&p, app.id, now());
            let mut reply = json!({"type": "opened", "ok": true, "app": app.id, "name": name});
            if let Some(code) = verdict.code() {
                reply["guard"] = json!(code);
                reply["warning"] = json!(guard::LINKS_TEXT);
                reply["excel"] = json!(guard::excel_exe().is_some());
                reply["path"] = json!(path);
            }
            Ok(reply)
        };
        go().unwrap_or_else(|error| json!({"type": "opened", "ok": false, "error": error}))
    }

    /// Starts Excel on a Sheets workbook. The page may name only a file the host already knows; Excel must be installed.
    pub fn open_in_excel(&self, path: &str, src: Source) -> Value {
        let go = || -> Result<(), String> {
            if src == Source::Page && !self.store.knows(path) {
                return Err("this file is not in the recent list".into());
            }
            let (app, p) = check_file(&self.brand, path, Source::User)?;
            if app.id != "sheets" {
                return Err("this file is not a workbook".into());
            }
            let excel = guard::excel_exe().ok_or("Excel is not installed")?;
            std::process::Command::new(excel)
                .arg(&p)
                .spawn()
                .map(|_| ())
                .map_err(|e| format!("cannot start Excel: {e}"))
        };
        match go() {
            Ok(()) => json!({"type": "excel", "ok": true}),
            Err(error) => json!({"type": "excel", "ok": false, "error": error}),
        }
    }

    /// The history store and the file for a path from the page: it must be the path of a document that has versions, and satisfy the
    /// path rules (the file must still exist: restoring a deleted file is not done yet).
    fn history_target(&self, path: &str) -> Result<(&HStore, &AppDef, PathBuf), String> {
        let h = self
            .history
            .as_ref()
            .ok_or("the version history is not available")?;
        // the store keeps the canonical path (`\\?\C:\...`): the page may send it with or without the prefix
        let same = |d: &str| d == path || d.strip_prefix(r"\\?\") == Some(path);
        // a mapped drive (`H:\a.docx`) is stored as `\\?\UNC\srv\share\a.docx`: a drive-letter path matches by document id (this
        // touches only the user's own drive, never a UNC path from the page)
        let drive = matches!(Path::new(path).components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)));
        let id = drive.then(|| suite_core::history::doc_id(Path::new(path)));
        if !h
            .documents()
            .iter()
            .any(|d| same(&d.path) || id.as_deref() == Some(d.id.as_str()))
        {
            return Err("this file has no saved versions".into());
        }
        // the store holds only files the user opened, so a share is allowed here (F10)
        let (app, p) = check_file(&self.brand, path, Source::User)?;
        Ok((h, app, p))
    }

    /// Runs one history action. A refused path is `rejected`; a failed action is the message's own type with `ok: false`.
    fn history_op(
        &self,
        ty: &str,
        path: &str,
        f: impl FnOnce(&HStore, &AppDef, &Path) -> Result<Value, String>,
    ) -> Value {
        let (h, app, p) = match self.history_target(path) {
            Ok(t) => t,
            Err(error) => return json!({"type": "rejected", "error": error}),
        };
        match f(h, app, &p) {
            Ok(mut v) => {
                v["type"] = json!(ty);
                v["ok"] = json!(true);
                v
            }
            Err(error) => json!({"type": ty, "ok": false, "error": error}),
        }
    }

    /// Version `v` as `name (v3 view).ext` in `dir` (or the first free `name (v3 view) (n).ext`).
    fn write_version_copy(
        &self,
        h: &HStore,
        p: &Path,
        v: u64,
        dir: &Path,
        suffix: &str,
    ) -> Result<PathBuf, String> {
        let bytes = h.read(p, v).map_err(|e| e.to_string())?;
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
        let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let to = download_target(
            &self.brand,
            &dir.join(format!("{stem} (v{v}{suffix}).{ext}")),
        )?;
        std::fs::write(&to, bytes).map_err(|e| e.to_string())?;
        Ok(to)
    }

    fn history_view(&self, path: &str, v: u64) -> Value {
        self.history_op("history_view", path, |h, app, p| {
            let exe = self.bin_dir.join(format!("{}.exe", app.bin));
            if !exe.is_file() {
                return Err(format!("the editor {}.exe is not installed", app.bin));
            }
            // a read-only copy: the editor refuses its save, and the version in the history and the document stay as they are (F7)
            let to = self.write_version_copy(h, p, v, &self.store.dir.join("temp"), " view")?;
            let mut perm = std::fs::metadata(&to)
                .map_err(|e| e.to_string())?
                .permissions();
            perm.set_readonly(true);
            std::fs::set_permissions(&to, perm).map_err(|e| e.to_string())?;
            (self.spawn)(&exe, &to).map_err(|e| format!("cannot start the editor: {e}"))?;
            let name = to.file_name().map(|n| n.to_string_lossy().into_owned());
            Ok(json!({"v": v, "name": name, "copy": true}))
        })
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
            Request::OpenInExcel { path } => self.open_in_excel(&path, Source::Page),
            Request::Recent {} => json!({"type": "recent", "items": items(self.store.recent(20))}),
            // the open log stays internal (it is the whitelist of `open`); the page sees the documents that have versions
            Request::History {} => {
                let docs = self.history.as_ref().map(HStore::documents).unwrap_or_default();
                let items: Vec<Value> = docs
                    .iter()
                    .map(|d| {
                        let ext = Path::new(&d.path)
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(str::to_ascii_lowercase)
                            .unwrap_or_default();
                        let app = self.brand.apps.iter().find(|a| a.exts.contains(&ext.as_str()));
                        json!({"path": d.path, "app": app.map(|a| a.id), "versions": d.versions, "ts": d.last_time, "size": d.size})
                    })
                    .collect();
                json!({"type": "history", "items": items})
            }
            Request::HistoryVersions { path } => {
                self.history_op("history_versions", &path, |h, _, p| {
                    let items: Vec<Value> = h
                        .list(p)
                        .map_err(|e| e.to_string())?
                        .iter()
                        .map(suite_core::history::Version::to_json)
                        .collect();
                    let verified = h.verify(p);
                    Ok(json!({"path": path, "items": items, "verified": verified.is_ok(), "problem": verified.err()}))
                })
            }
            Request::HistoryView { path, v } => self.history_view(&path, v),
            Request::HistoryRestore { path, v } => {
                self.history_op("history_restore", &path, |h, _, p| {
                    let made = h.restore(p, v).map_err(|e| e.to_string())?;
                    Ok(json!({"version": made.to_json()}))
                })
            }
            Request::HistoryCopy { path, v } => self.history_op("history_copy", &path, |h, _, p| {
                let to = self.write_version_copy(h, p, v, &self.downloads, "")?;
                Ok(json!({"v": v, "path": to}))
            }),
            Request::HistoryName { path, v, name } => {
                self.history_op("history_name", &path, |h, _, p| {
                    h.name(p, v, &name).map_err(|e| e.to_string())?;
                    Ok(json!({"v": v}))
                })
            }
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
    /// show the History panel for this file (an editor's "Version history" command)
    pub history: Option<String>,
    pub files: Vec<String>,
}

pub const USAGE: &str = "usage: suite [--dist DIR] [--shell DIR] [--brand FILE] [--bin-dir DIR] [--browser] [--history FILE] [--] [FILE...]";

/// Anything that is not a flag is a file, so `hoshiya.exe "C:\Users\Имя\Hisobot 2026.xlsx"` works as "Open with" calls it.
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
            "--history" => a.history = Some(val()?.to_string_lossy().into_owned()),
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
            history: Some(HStore::new(
                d.join("hist"),
                suite_core::history::Limits::from(
                    &suite_core::settings::Settings::default().history,
                ),
            )),
            downloads: d.join("downloads"),
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
        // recent and the (internal) open log record every open, newest first; the page's History lists documents with versions only
        let r = reply(c.handle(Request::Recent {}));
        assert_eq!(r["items"][0]["app"], "docs");
        assert_eq!(c.store.history().len(), 3);
        assert!(
            reply(c.handle(Request::History {}))["items"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    fn zip_file(path: &Path, parts: &[(&str, u64)]) {
        use std::io::Write;
        let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let o = zip::write::SimpleFileOptions::default().compression_level(Some(1));
        for (name, size) in parts {
            z.start_file(*name, o.large_file(*size > u32::MAX as u64))
                .unwrap();
            let chunk = vec![0u8; 1 << 20];
            let mut left = *size;
            while left > 0 {
                let n = left.min(chunk.len() as u64) as usize;
                z.write_all(&chunk[..n]).unwrap();
                left -= n as u64;
            }
        }
        z.finish().unwrap();
    }

    /// F18: a workbook over the wall never reaches Sheets, is listed in Recent and says so; external links open with a warning.
    #[test]
    fn the_guard_stops_a_workbook_over_the_wall_and_warns_about_links() {
        let (c, d, log) = ctx("guard");
        let wall = d.join("katta.xlsx");
        zip_file(
            &wall,
            &[
                ("xl/workbook.xml", 100),
                ("xl/worksheets/sheet1.xml", guard::MAX_PART + 1),
            ],
        );
        let v = c.open(wall.to_str().unwrap(), Source::User);
        assert_eq!(
            (v["ok"].clone(), v["guard"].clone(), v["error"].clone()),
            (false.into(), "wall".into(), guard::WALL_TEXT.into()),
            "{v}"
        );
        assert_eq!(v["excel"], guard::excel_exe().is_some());
        assert!(log.lock().unwrap().is_empty(), "the editor was not started");
        assert_eq!(
            c.store.history().len(),
            1,
            "listed, so the page can offer Excel"
        );
        // the page may then ask for Excel for it, and only for a workbook
        let missing = c.open_in_excel(&d.join("other.xlsx").display().to_string(), Source::Page);
        assert!(
            missing["error"].as_str().unwrap().contains("recent list"),
            "{missing}"
        );

        let linked = d.join("bog.xlsx");
        zip_file(
            &linked,
            &[
                ("xl/workbook.xml", 100),
                ("xl/externalLinks/externalLink1.xml", 100),
            ],
        );
        let v = c.open(linked.to_str().unwrap(), Source::User);
        assert_eq!(
            (v["ok"].clone(), v["guard"].clone(), v["warning"].clone()),
            (true.into(), "links".into(), guard::LINKS_TEXT.into()),
            "{v}"
        );
        assert_eq!(
            log.lock().unwrap().len(),
            1,
            "a linked workbook still opens"
        );

        let plain = d.join("oddiy.xlsx");
        zip_file(&plain, &[("xl/workbook.xml", 100)]);
        let v = c.open(plain.to_str().unwrap(), Source::User);
        assert!(
            v["ok"] == true && v["guard"].is_null() && v["warning"].is_null(),
            "{v}"
        );

        let doc = d.join("a.docx");
        std::fs::write(&doc, b"x").unwrap();
        c.open(doc.to_str().unwrap(), Source::User);
        let not_sheets = c.open_in_excel(doc.to_str().unwrap(), Source::Page);
        assert_eq!(not_sheets["error"], "this file is not a workbook");
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

    /// F10: a document on a share (UNC) or a mapped drive keeps its history and can be reopened from the page. The share is the
    /// machine's own admin share (`\localhost\C$`), a mapped drive comes from `SUITE_TEST_MAPPED_DIR` (a folder on a mapped
    /// drive); the test is skipped when neither is reachable.
    #[test]
    #[cfg(windows)]
    fn a_document_on_a_share_or_mapped_drive_keeps_its_history() {
        let (c, d, _log) = ctx("f10");
        let s = d.to_string_lossy().into_owned();
        let unc = PathBuf::from(format!(r"\localhost\{}${}", &s[..1], &s[2..]));
        let mut dirs = vec![unc];
        dirs.extend(std::env::var_os("SUITE_TEST_MAPPED_DIR").map(PathBuf::from));
        let h = c.history.as_ref().unwrap();
        let send = |v: Value| reply(c.message(&v.to_string()));
        let mut ran = 0;
        for dir in dirs.iter().filter(|d| d.is_dir()) {
            ran += 1;
            let doc = dir.join(format!("f10-{ran}.docx"));
            std::fs::write(&doc, b"one").unwrap();
            assert_eq!(c.open(doc.to_str().unwrap(), Source::User)["ok"], true);
            h.snapshot(&doc, None).unwrap();
            // the page asks as the user typed the path, and as the store lists it (`\?\...`)
            let stored = h.documents()[0].path.clone();
            for p in [doc.to_str().unwrap(), stored.as_str()] {
                let v = send(json!({"type": "history_versions", "path": p}));
                assert_eq!(v["ok"], true, "{p}: {v}");
            }
            // Recent lists the path as the user opened it
            assert_eq!(c.open(doc.to_str().unwrap(), Source::Page)["ok"], true);
            // a share path that has no versions is still refused for the page
            let other = dir.join(format!("f10-other-{ran}.docx"));
            std::fs::write(&other, b"o").unwrap();
            let v = send(json!({"type": "history_versions", "path": other}));
            assert_eq!(v["type"], "rejected", "{v}");
            std::fs::remove_file(&doc).unwrap();
            std::fs::remove_file(&other).unwrap();
        }
        if ran == 0 {
            eprintln!("skipped: no share reachable");
        }
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
        let get = || reply(c.handle(Request::SettingsGet {}))["values"].clone();
        // nothing saved yet: the defaults of suite-core, and only the keys a page may see
        assert_eq!(
            get(),
            json!({"lang": "auto", "toolbar": "simple", "latin_alphabet": "2026", "theme": "system"})
        );
        assert_eq!(set("lang", "ru")["values"]["lang"], "ru");
        assert_eq!(
            set("lang", "uz-Latn-2026")["values"]["lang"],
            "uz-Latn-2026"
        );
        assert_eq!(set("toolbar", "full")["values"]["toolbar"], "full");
        assert_eq!(
            set("latin_alphabet", "1995")["values"]["latin_alphabet"],
            "1995"
        );
        assert_eq!(set("theme", "dark")["values"]["theme"], "dark");
        for (k, v) in [
            ("lang", "xx"),
            ("cmd", "calc"),
            ("lang", ""),
            ("lang", "uz"),
            ("toolbar", "Full"),
            ("toolbar", ""),
            ("latin_alphabet", "3000"),
            ("theme", "Dark"),
            ("theme", "blue"),
            // real settings, but not the page's to change
            ("server.url", "https://example.org"),
            ("spell_dictionaries", "C:/x"),
            ("recent.limit", "5"),
            ("xdfu", "on"),
        ] {
            assert_eq!(set(k, v)["type"], "rejected", "{k}={v}");
        }
        assert_eq!(
            get(),
            json!({"lang": "uz-Latn-2026", "toolbar": "full", "latin_alphabet": "1995", "theme": "dark"})
        );
        // the shared file is what the apps read
        let shared = suite_core::settings::load_in(&d.join("data")).settings;
        assert_eq!(shared.toolbar, suite_core::settings::ToolbarMode::Full);
        assert_eq!(shared.theme, suite_core::settings::Theme::Dark);
        assert_eq!(shared.lang.tag(), "uz-Latn-2026");
        let text = std::fs::read_to_string(d.join("data").join("settings.toml")).unwrap();
        assert!(!text.contains("server") && !text.contains("xdfu"), "{text}");
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

    /// WP10: the page lists, views, restores, copies and names versions, for documents that have versions only.
    #[test]
    fn the_history_messages() {
        let (c, d, log) = ctx("hist");
        let doc = d.join("Hisobot.docx");
        let other = d.join("other.docx");
        std::fs::write(&other, b"o").unwrap();
        let h = c.history.as_ref().unwrap();
        std::fs::write(&doc, b"one").unwrap();
        h.snapshot(&doc, Some("first")).unwrap();
        std::fs::write(&doc, b"two").unwrap();
        h.snapshot(&doc, None).unwrap();
        let p = doc.to_str().unwrap();
        let send = |v: Value| reply(c.message(&v.to_string()));

        // the list is the documents that have versions, not the open log
        c.store.record(&other, "docs", 1);
        let l = send(json!({"type": "history"}));
        assert_eq!(l["items"].as_array().unwrap().len(), 1, "{l}");
        assert_eq!(
            (
                l["items"][0]["path"]
                    .as_str()
                    .map(|x| x.trim_start_matches(r"\\?\")),
                l["items"][0]["app"].as_str(),
                l["items"][0]["versions"].as_u64()
            ),
            (Some(p), Some("docs"), Some(2))
        );
        let v = send(json!({"type": "history_versions", "path": p}));
        assert_eq!(
            (
                v["ok"].clone(),
                v["verified"].clone(),
                v["items"].as_array().unwrap().len()
            ),
            (json!(true), json!(true), 2),
            "{v}"
        );
        assert_eq!(v["items"][0]["message"], "first");

        // refused: unknown fields, a path with no history, a bad path, a bad version
        for body in [
            json!({"type": "history_versions", "path": p, "x": 1}),
            json!({"type": "history_restore", "path": p}),
            json!({"type": "history_name", "path": p, "v": 1}),
            json!({"type": "history_view", "path": p, "v": -1}),
        ] {
            assert_eq!(send(body.clone())["type"], "rejected", "{body}");
        }
        for path in [
            other.to_str().unwrap(),
            r"C:\nowhere\a.docx",
            r"\\h\s\a.docx",
            "a.docx",
            "",
        ] {
            for ty in [
                "history_versions",
                "history_restore",
                "history_copy",
                "history_view",
            ] {
                let r = send(json!({"type": ty, "path": path, "v": 1}));
                assert_eq!(r["type"], "rejected", "{ty} {path}: {r}");
            }
        }
        assert_eq!(
            send(json!({"type": "history_name", "path": other, "v": 1, "name": "x"}))["type"],
            "rejected"
        );

        // name
        let r = send(json!({"type": "history_name", "path": p, "v": 1, "name": "Final"}));
        assert_eq!(r["ok"], true, "{r}");
        assert_eq!(
            send(json!({"type": "history_versions", "path": p}))["items"][0]["name"],
            "Final"
        );

        // view: a copy opened in the editor, the document untouched
        let r = send(json!({"type": "history_view", "path": p, "v": 1}));
        assert_eq!(
            (r["ok"].clone(), r["name"].as_str()),
            (json!(true), Some("Hisobot (v1 view).docx")),
            "{r}"
        );
        let copy = d.join("data/temp/Hisobot (v1 view).docx");
        assert_eq!(std::fs::read(&copy).unwrap(), b"one");
        assert!(std::fs::metadata(&copy).unwrap().permissions().readonly());
        // a save report for a View copy takes no snapshot
        assert_eq!(c.saved(&copy)["ok"], true);
        assert!(
            c.history
                .as_ref()
                .unwrap()
                .list(&copy)
                .map_or(true, |l| l.is_empty())
        );
        assert_eq!(log.lock().unwrap().last().unwrap().1, copy);
        assert_eq!(std::fs::read(&doc).unwrap(), b"two");

        // copy: into the downloads folder, never over a file
        let r = send(json!({"type": "history_copy", "path": p, "v": 1}));
        assert_eq!(r["ok"], true, "{r}");
        assert_eq!(
            std::fs::read(d.join("downloads/Hisobot (v1).docx")).unwrap(),
            b"one"
        );
        let r = send(json!({"type": "history_copy", "path": p, "v": 1}));
        assert_eq!(
            std::fs::read(d.join("downloads/Hisobot (v1) (1).docx")).unwrap(),
            b"one",
            "{r}"
        );
        assert_eq!(
            send(json!({"type": "history_copy", "path": p, "v": 99}))["ok"],
            false
        );

        // restore round trip: the file has the old bytes, and the list has a new version; the current content is kept
        let r = send(json!({"type": "history_restore", "path": p, "v": 1}));
        assert_eq!(
            (r["ok"].clone(), r["version"]["restored_from"].clone()),
            (json!(true), json!(1)),
            "{r}"
        );
        assert_eq!(std::fs::read(&doc).unwrap(), b"one");
        let v = send(json!({"type": "history_versions", "path": p}));
        assert_eq!(v["items"].as_array().unwrap().len(), 3, "{v}");
        assert_eq!(c.history.as_ref().unwrap().read(&doc, 2).unwrap(), b"two");
        // the next page-visible call sees the same
        assert_eq!(send(json!({"type": "history"}))["items"][0]["versions"], 3);
    }

    #[test]
    fn a_save_from_the_window_is_a_version_and_the_fragment_is_encoded() {
        let (c, d, _log) = ctx("dlsnap");
        let f = d.join("Ҳисобот 1.docx");
        std::fs::write(&f, b"abc").unwrap();
        assert_eq!(c.saved(&f)["ok"], true);
        assert_eq!(c.history.as_ref().unwrap().list(&f).unwrap().len(), 1);
        assert_eq!(
            history_fragment(r"C:\a b\x#.docx"),
            "#history=C%3A%5Ca%20b%5Cx%23.docx"
        );
        let a = parse_args(["--history".to_string(), "X.docx".into()]).unwrap();
        assert_eq!(a.history.as_deref(), Some("X.docx"));
        assert!(parse_args(["--history".to_string()]).is_err());
    }

    #[test]
    fn webview2_env_that_changes_the_browser_is_found() {
        assert_eq!(webview2_env_override(|_| false), None);
        assert_eq!(
            webview2_env_override(|k| k == "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"),
            Some("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS")
        );
        assert!(webview2_env_override(|k| k.ends_with("EXECUTABLE_FOLDER")).is_some());
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
