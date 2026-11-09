//! The per-user settings file, `<settings dir>/settings.toml` (PLAN 5.1). One file for the shell host and the four apps.
//!
//! - **Typed keys.** Every key has a type and a range ([`KEYS`]). A value that is out of range, or of the wrong TOML type, is
//!   reported in [`Loaded::warnings`] and replaced by its default; the other keys of the file still apply.
//! - **Tolerant.** A missing file is the defaults. A file that is not valid TOML (or is over 1 MiB, or unreadable) is renamed to
//!   `settings.toml.corrupt` (the backup, overwritten by a later one) and the defaults apply. Nothing here panics or fails a start.
//! - **Atomic writes.** A change goes to a temporary file in the same folder and is renamed over the old one, so a crash leaves
//!   the old file. Only the key being set changes: other keys, unknown keys and comments stay as the user left them.
//! - **No locking.** Two processes setting different keys at the same instant can lose one change (last rename wins). Settings
//!   change on a click, so this is accepted; a reader never sees a half-written file.
//!
//! The file is read by whoever starts (apps read it once at start-up; do not call [`load`] per frame).
//!
//! ```toml
//! lang = "auto"              # auto | en | ru | uz-Latn | uz-Latn-2026 | uz-Cyrl
//! latin_alphabet = "2026"    # 2026 | 1995 | auto: the Latin alphabet the transliteration writes
//! toolbar = "simple"         # simple | full: Docs, Sheets and Slides (PDF ignores it; on the web: saved choice in localStorage, then the URL's ?toolbar=, then simple)
//! theme = "system"           # system | light | dark: the shell and every app, each in its own palette (read at start)
//! spell_dictionaries = ""    # a folder with the dictionaries; empty = the install folder's
//! xdfu = "off"               # off | on
//! [recent]
//! limit = 20                 # files on the home screen (0..=50)
//! [history]
//! versions = 50              # versions kept per document
//! days = 90                  # ... or this many days, whichever keeps more
//! max_mb = 2048              # all history together
//! big_file_mb = 50           # a file over this size ...
//! big_file_versions = 5      # ... keeps only this many versions
//! [server]
//! url = ""                   # empty = local mode (nothing ever asks for a login)
//! ```

use crate::lang::LangChoice;
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, Item, Value};

/// The file name inside the settings folder.
pub const FILE_NAME: &str = "settings.toml";
/// Where a file that does not parse is moved.
pub const CORRUPT_NAME: &str = "settings.toml.corrupt";
/// Larger files are not settings.
const MAX_BYTES: u64 = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ToolbarMode {
    /// The few commands office staff use (the default).
    #[default]
    Simple,
    /// The upstream ribbon.
    Full,
}

impl ToolbarMode {
    pub const fn tag(self) -> &'static str {
        match self {
            ToolbarMode::Simple => "simple",
            ToolbarMode::Full => "full",
        }
    }
}

/// The interface theme of the shell and the four apps (owner, 2026-10-29). Each app applies it to its own light or dark palette
/// at start; `System` follows the operating system's light/dark choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub const fn tag(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }
}

/// The Latin alphabet the transliteration writes (mirrors `uz_translit::LatinAlphabet`; this crate does not link it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LatinAlphabet {
    /// The 1995 alphabet (oʻ gʻ sh ch). An explicit choice; stored as "1995" and kept.
    Y1995,
    /// The default (owner decision O13, 2026-10-29): an absent or invalid value is 2026.
    #[default]
    Y2026,
    Auto,
}

impl LatinAlphabet {
    pub const fn tag(self) -> &'static str {
        match self {
            LatinAlphabet::Y1995 => "1995",
            LatinAlphabet::Y2026 => "2026",
            LatinAlphabet::Auto => "auto",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct History {
    pub versions: u32,
    pub days: u32,
    pub max_mb: u32,
    pub big_file_mb: u32,
    pub big_file_versions: u32,
}

/// Every setting, with its value or its default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub lang: LangChoice,
    pub latin_alphabet: LatinAlphabet,
    pub toolbar: ToolbarMode,
    pub theme: Theme,
    pub spell_dictionaries: String,
    /// XDFU is opt-in and off by default everywhere.
    pub xdfu: bool,
    /// How many recent files the home screen lists.
    pub recent_limit: u32,
    pub history: History,
    pub server_url: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            lang: LangChoice::Auto,
            latin_alphabet: LatinAlphabet::default(),
            toolbar: ToolbarMode::Simple,
            theme: Theme::System,
            spell_dictionaries: String::new(),
            xdfu: false,
            recent_limit: 20,
            history: History {
                versions: 50,
                days: 90,
                max_mb: 2048,
                big_file_mb: 50,
                big_file_versions: 5,
            },
            server_url: String::new(),
        }
    }
}

/// How a key's value is checked.
#[derive(Clone, Copy)]
enum Ty {
    Choice(&'static [&'static str]),
    Int(i64, i64),
    /// free text up to this many bytes, no control characters
    Text(usize),
    /// empty, or `http://` / `https://` and no whitespace, up to 2048 bytes
    Url,
}

const LANG_TAGS: &[&str] = &["auto", "en", "ru", "uz-Latn", "uz-Latn-2026", "uz-Cyrl"];

/// Every key, by its dotted name.
const KEYS: &[(&str, Ty)] = &[
    ("lang", Ty::Choice(LANG_TAGS)),
    ("latin_alphabet", Ty::Choice(&["1995", "2026", "auto"])),
    ("toolbar", Ty::Choice(&["simple", "full"])),
    ("theme", Ty::Choice(&["system", "light", "dark"])),
    ("spell_dictionaries", Ty::Text(1024)),
    ("xdfu", Ty::Choice(&["off", "on"])),
    ("recent.limit", Ty::Int(0, 50)),
    ("history.versions", Ty::Int(1, 1000)),
    ("history.days", Ty::Int(1, 3650)),
    ("history.max_mb", Ty::Int(64, 102_400)),
    ("history.big_file_mb", Ty::Int(1, 4096)),
    ("history.big_file_versions", Ty::Int(1, 50)),
    ("server.url", Ty::Url),
];

/// The names of all keys.
pub fn key_names() -> impl Iterator<Item = &'static str> {
    KEYS.iter().map(|(k, _)| *k)
}

fn ty(key: &str) -> Option<Ty> {
    KEYS.iter().find(|(k, _)| *k == key).map(|(_, t)| *t)
}

/// Why a [`set_in`] was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum SetError {
    UnknownKey,
    BadValue(String),
    Io(String),
}

impl std::fmt::Display for SetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SetError::UnknownKey => f.write_str("unknown setting"),
            SetError::BadValue(why) => write!(f, "bad value: {why}"),
            SetError::Io(e) => write!(f, "cannot write the settings file: {e}"),
        }
    }
}

impl std::error::Error for SetError {}

/// Checks `value` (the text form) against the key's type and range.
pub fn validate(key: &str, value: &str) -> Result<(), SetError> {
    let bad = |why: &str| Err(SetError::BadValue(why.into()));
    match ty(key).ok_or(SetError::UnknownKey)? {
        Ty::Choice(allowed) if allowed.contains(&value) => Ok(()),
        Ty::Choice(allowed) => bad(&format!("one of {}", allowed.join(", "))),
        Ty::Int(lo, hi) => match value.parse::<i64>() {
            Ok(n) if (lo..=hi).contains(&n) => Ok(()),
            _ => bad(&format!("a whole number from {lo} to {hi}")),
        },
        Ty::Text(max) if value.len() <= max && !value.chars().any(char::is_control) => Ok(()),
        Ty::Text(max) => bad(&format!(
            "text of at most {max} bytes without control characters"
        )),
        Ty::Url => {
            let ok = value.is_empty()
                || ((value.starts_with("http://") || value.starts_with("https://"))
                    && value.len() <= 2048
                    && !value.chars().any(|c| c.is_whitespace() || c.is_control()));
            if ok {
                Ok(())
            } else {
                bad("empty, or an http:// or https:// address")
            }
        }
    }
}

/// A value already validated. Writes it into `s`.
fn apply(s: &mut Settings, key: &str, v: &str) {
    let n = || v.parse::<u32>().unwrap_or_default();
    match key {
        "lang" => s.lang = LangChoice::parse(v).unwrap_or_default(),
        "latin_alphabet" => {
            s.latin_alphabet = match v {
                "1995" => LatinAlphabet::Y1995,
                "auto" => LatinAlphabet::Auto,
                _ => LatinAlphabet::Y2026,
            }
        }
        "toolbar" => {
            s.toolbar = if v == "full" {
                ToolbarMode::Full
            } else {
                ToolbarMode::Simple
            }
        }
        "theme" => {
            s.theme = match v {
                "light" => Theme::Light,
                "dark" => Theme::Dark,
                _ => Theme::System,
            }
        }
        "spell_dictionaries" => s.spell_dictionaries = v.into(),
        "xdfu" => s.xdfu = v == "on",
        "recent.limit" => s.recent_limit = n(),
        "history.versions" => s.history.versions = n(),
        "history.days" => s.history.days = n(),
        "history.max_mb" => s.history.max_mb = n(),
        "history.big_file_mb" => s.history.big_file_mb = n(),
        "history.big_file_versions" => s.history.big_file_versions = n(),
        "server.url" => s.server_url = v.into(),
        _ => {}
    }
}

impl Settings {
    /// The settings in the text form [`set_in`] takes, by dotted key name (what the desktop host sends to the shell).
    pub fn to_map(&self) -> BTreeMap<&'static str, String> {
        let h = &self.history;
        [
            ("lang", self.lang.tag().to_string()),
            ("latin_alphabet", self.latin_alphabet.tag().into()),
            ("toolbar", self.toolbar.tag().into()),
            ("theme", self.theme.tag().into()),
            ("spell_dictionaries", self.spell_dictionaries.clone()),
            ("xdfu", if self.xdfu { "on" } else { "off" }.into()),
            ("recent.limit", self.recent_limit.to_string()),
            ("history.versions", h.versions.to_string()),
            ("history.days", h.days.to_string()),
            ("history.max_mb", h.max_mb.to_string()),
            ("history.big_file_mb", h.big_file_mb.to_string()),
            ("history.big_file_versions", h.big_file_versions.to_string()),
            ("server.url", self.server_url.clone()),
        ]
        .into_iter()
        .collect()
    }

    /// Parses the text of a settings file. `Err` only when it is not TOML; bad values become warnings and defaults.
    pub fn from_toml(text: &str) -> Result<(Settings, Vec<String>), String> {
        let doc: DocumentMut = text
            .parse()
            .map_err(|e: toml_edit::TomlError| e.to_string())?;
        let mut s = Settings::default();
        let mut warnings = Vec::new();
        for (key, kind) in KEYS {
            let Some(item) = lookup(&doc, key) else {
                continue;
            };
            // the TOML type must match the key's type: lang = 5 and recent.limit = "5" are both mistakes
            let raw = match (kind, item.as_value()) {
                (Ty::Int(..), Some(Value::Integer(x))) => Some(x.value().to_string()),
                (Ty::Choice(_) | Ty::Text(_) | Ty::Url, Some(Value::String(x))) => {
                    Some(x.value().clone())
                }
                _ => None,
            };
            let Some(raw) = raw else {
                warnings.push(format!("{key}: wrong type, using the default"));
                continue;
            };
            match validate(key, &raw) {
                Ok(()) => apply(&mut s, key, &raw),
                Err(e) => warnings.push(format!("{key}: {e}, using the default")),
            }
        }
        Ok((s, warnings))
    }
}

fn lookup<'a>(doc: &'a DocumentMut, key: &str) -> Option<&'a Item> {
    let mut item = doc.as_item();
    for part in key.split('.') {
        item = item.get(part)?;
    }
    Some(item)
}

/// What [`load_in`] found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loaded {
    pub settings: Settings,
    /// Things worth a log line: a value replaced by its default, a corrupt file set aside.
    pub warnings: Vec<String>,
}

/// The settings folder: `<config base>/<product>` (`%APPDATA%\<product>` on Windows). `None` when the platform gives no base.
/// Debug builds honour `SUITE_SETTINGS_DIR` (tests of the apps and the host); a release build ignores it.
pub fn dir() -> Option<PathBuf> {
    dir_with(&|k| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    })
}

fn dir_with(env: &dyn Fn(&str) -> Option<PathBuf>) -> Option<PathBuf> {
    if cfg!(debug_assertions)
        && let Some(p) = env("SUITE_SETTINGS_DIR")
    {
        return Some(p);
    }
    crate::config_base().map(|b| b.join(crate::PRODUCT))
}

/// Moves a damaged file out of the way. Best effort: if it cannot be renamed it is left, and the next write replaces it anyway.
fn set_aside(path: &Path, why: &str, warnings: &mut Vec<String>) {
    let to = path.with_file_name(CORRUPT_NAME);
    let kept = std::fs::rename(path, to).is_ok();
    let tail = if kept {
        format!("kept as {CORRUPT_NAME}, using the defaults")
    } else {
        "using the defaults".into()
    };
    warnings.push(format!("{}: {why}; {tail}", path.display()));
}

/// Reads `<dir>/settings.toml`. Never fails.
pub fn load_in(dir: &Path) -> Loaded {
    let path = dir.join(FILE_NAME);
    let mut warnings = Vec::new();
    let text = match std::fs::metadata(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Ok(m) if m.is_file() && m.len() <= MAX_BYTES => {
            match std::fs::read(&path).map(String::from_utf8) {
                Ok(Ok(t)) => Some(t),
                _ => {
                    set_aside(&path, "unreadable or not UTF-8", &mut warnings);
                    None
                }
            }
        }
        Ok(_) => {
            set_aside(&path, "not a settings file", &mut warnings);
            None
        }
        Err(_) => {
            warnings.push(format!(
                "{}: cannot be read, using the defaults",
                path.display()
            ));
            None
        }
    };
    let settings = match text.map(|t| Settings::from_toml(&t)) {
        None => Settings::default(),
        Some(Ok((s, w))) => {
            warnings.extend(w);
            s
        }
        Some(Err(e)) => {
            set_aside(
                &path,
                &format!("not valid TOML ({})", e.lines().next().unwrap_or("")),
                &mut warnings,
            );
            Settings::default()
        }
    };
    Loaded { settings, warnings }
}

/// The settings of this user: the file in [`dir`], or the defaults. Never fails; a damaged file is set aside.
pub fn load() -> Settings {
    dir().map(|d| load_in(&d).settings).unwrap_or_default()
}

/// Which toolbar mode wins on the web: the user's saved choice, then the URL's `?toolbar=full|simple` (the document server
/// passes it), then simple. Any other value counts as absent.
pub fn resolve_toolbar(saved: Option<&str>, query: Option<&str>) -> ToolbarMode {
    let pick = |v: Option<&str>| match v {
        Some("full") => Some(ToolbarMode::Full),
        Some("simple") => Some(ToolbarMode::Simple),
        _ => None,
    };
    pick(saved).or(pick(query)).unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
const WEB_TOOLBAR_KEY: &str = "suite.toolbar";

#[cfg(target_arch = "wasm32")]
fn web_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

/// The toolbar mode of this user (`simple` unless the file says `full`; on the web see [`resolve_toolbar`]). Read it once at
/// start-up.
pub fn toolbar_mode() -> ToolbarMode {
    #[cfg(target_arch = "wasm32")]
    {
        let saved = web_storage().and_then(|s| s.get_item(WEB_TOOLBAR_KEY).ok().flatten());
        let query = web_sys::window()
            .and_then(|w| w.location().search().ok())
            .and_then(|q| web_sys::UrlSearchParams::new_with_str(&q).ok())
            .and_then(|p| p.get("toolbar"));
        return resolve_toolbar(saved.as_deref(), query.as_deref());
    }
    #[cfg(not(target_arch = "wasm32"))]
    load().toolbar
}

/// Saves the toolbar mode for all apps. The other apps see it the next time they start. On the web it goes to `localStorage`.
pub fn set_toolbar_mode(mode: ToolbarMode) -> io::Result<()> {
    #[cfg(target_arch = "wasm32")]
    return web_storage()
        .ok_or_else(|| io::Error::other("no localStorage"))?
        .set_item(WEB_TOOLBAR_KEY, mode.tag())
        .map_err(|_| io::Error::other("localStorage refused the write"));
    #[cfg(not(target_arch = "wasm32"))]
    {
        let dir = dir().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "no settings folder on this platform",
            )
        })?;
        set_in(&dir, "toolbar", mode.tag()).map_err(|e| io::Error::other(e.to_string()))
    }
}

/// Sets one key (`value` in the text form, checked by [`validate`]) in `<dir>/settings.toml`, atomically, leaving everything
/// else in the file as it is. A file that does not parse is set aside first and a new one is started.
pub fn set_in(dir: &Path, key: &str, value: &str) -> Result<(), SetError> {
    validate(key, value)?;
    let path = dir.join(FILE_NAME);
    let read = match std::fs::metadata(&path) {
        Ok(m) if m.is_file() && m.len() <= MAX_BYTES => std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| t.parse::<DocumentMut>().ok()),
        _ => None,
    };
    if read.is_none() && path.exists() {
        set_aside(&path, "not valid TOML", &mut Vec::new());
    }
    let mut doc = read.unwrap_or_default();
    let new = match ty(key) {
        Some(Ty::Int(..)) => Value::from(value.parse::<i64>().unwrap_or_default()),
        _ => Value::from(value),
    };
    let mut parts: Vec<&str> = key.split('.').collect();
    let leaf = parts.pop().ok_or(SetError::UnknownKey)?;
    let mut table = doc.as_table_mut();
    for p in parts {
        let entry = table
            .entry(p)
            .or_insert_with(|| Item::Table(toml_edit::Table::new()));
        if !entry.is_table() {
            // `recent = 5` where `[recent]` belongs: the user's value was never a valid setting
            *entry = Item::Table(toml_edit::Table::new());
        }
        table = entry.as_table_mut().ok_or(SetError::UnknownKey)?;
    }
    table.insert(leaf, Item::Value(new));
    write_atomic(&path, doc.to_string().as_bytes()).map_err(|e| SetError::Io(e.to_string()))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("toml.{}.tmp", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

#[cfg(test)]
pub(crate) mod tests {
    #[test]
    fn web_toolbar_precedence_saved_then_query_then_simple() {
        use ToolbarMode::{Full, Simple};
        assert_eq!(resolve_toolbar(None, None), Simple);
        assert_eq!(resolve_toolbar(None, Some("full")), Full);
        assert_eq!(resolve_toolbar(Some("simple"), Some("full")), Simple);
        assert_eq!(resolve_toolbar(Some("full"), Some("simple")), Full);
        assert_eq!(resolve_toolbar(Some("junk"), Some("full")), Full);
        assert_eq!(resolve_toolbar(Some("junk"), Some("junk")), Simple);
    }

    use super::*;
    use crate::lang::Lang;

    /// A fresh empty folder under the system temp dir.
    pub(crate) fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("suite-core-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_missing_file_is_the_defaults() {
        let d = tmp("missing");
        let l = load_in(&d);
        assert_eq!(l.settings, Settings::default());
        assert!(l.warnings.is_empty());
        assert_eq!(l.settings.toolbar, ToolbarMode::Simple);
        assert_eq!(l.settings.lang, LangChoice::Auto);
        assert!(!l.settings.xdfu);
        assert_eq!(l.settings.server_url, "");
    }

    #[test]
    fn alphabet_defaults_to_2026_and_a_stored_1995_stays() {
        let d = tmp("alphabet");
        assert_eq!(load_in(&d).settings.latin_alphabet, LatinAlphabet::Y2026);
        std::fs::write(d.join(FILE_NAME), "latin_alphabet = \"1995\"\n").unwrap();
        assert_eq!(load_in(&d).settings.latin_alphabet, LatinAlphabet::Y1995);
    }

    #[test]
    fn theme_defaults_to_system_and_a_bad_value_costs_only_the_theme() {
        let d = tmp("theme");
        assert_eq!(load_in(&d).settings.theme, Theme::System);
        for (tag, want) in [
            ("light", Theme::Light),
            ("dark", Theme::Dark),
            ("system", Theme::System),
        ] {
            set_in(&d, "theme", tag).unwrap();
            assert_eq!(load_in(&d).settings.theme, want);
            assert_eq!(want.tag(), tag);
        }
        assert!(matches!(
            set_in(&d, "theme", "Dark"),
            Err(SetError::BadValue(_))
        ));
        std::fs::write(d.join(FILE_NAME), "theme = \"blue\"\ntoolbar = \"full\"\n").unwrap();
        let l = load_in(&d);
        assert_eq!(
            (l.settings.theme, l.settings.toolbar),
            (Theme::System, ToolbarMode::Full)
        );
        assert_eq!(l.warnings.len(), 1, "{:?}", l.warnings);
    }

    #[test]
    fn typed_keys_are_read() {
        let d = tmp("read");
        std::fs::write(
            d.join(FILE_NAME),
            "lang = \"uz-Latn-2026\"\ntoolbar = \"full\"\nlatin_alphabet = \"2026\"\nxdfu = \"on\"\nunknown = 1\n[recent]\nlimit = 7\n[history]\nversions = 9\n[server]\nurl = \"https://example.org/x\"\n",
        )
        .unwrap();
        let l = load_in(&d);
        assert!(l.warnings.is_empty(), "{:?}", l.warnings);
        let s = l.settings;
        assert_eq!(s.lang, LangChoice::Fixed(Lang::UzLatn2026));
        assert_eq!(
            (s.toolbar, s.latin_alphabet, s.xdfu),
            (ToolbarMode::Full, LatinAlphabet::Y2026, true)
        );
        assert_eq!(
            (s.recent_limit, s.history.versions, s.history.days),
            (7, 9, 90)
        );
        assert_eq!(s.server_url, "https://example.org/x");
    }

    #[test]
    fn a_bad_value_costs_only_that_key() {
        let d = tmp("badvalue");
        std::fs::write(
            d.join(FILE_NAME),
            "lang = \"klingon\"\ntoolbar = \"full\"\nrecent = { limit = 99 }\nxdfu = 3\n",
        )
        .unwrap();
        let l = load_in(&d);
        assert_eq!(l.settings.toolbar, ToolbarMode::Full);
        assert_eq!(l.settings.lang, LangChoice::Auto);
        assert_eq!(l.settings.recent_limit, 20);
        assert!(!l.settings.xdfu);
        assert_eq!(l.warnings.len(), 3, "{:?}", l.warnings);
        assert!(
            d.join(FILE_NAME).exists(),
            "a bad value does not move the file"
        );
        // a number where text belongs, and text where a number belongs
        std::fs::write(d.join(FILE_NAME), "lang = 5\n[recent]\nlimit = \"5\"\n").unwrap();
        let l = load_in(&d);
        assert_eq!((l.settings, l.warnings.len()), (Settings::default(), 2));
    }

    #[test]
    fn a_corrupt_file_is_set_aside_and_the_defaults_apply() {
        let d = tmp("corrupt");
        std::fs::write(d.join(FILE_NAME), "toolbar = [[[ not toml").unwrap();
        let l = load_in(&d);
        assert_eq!(l.settings, Settings::default());
        assert_eq!(l.warnings.len(), 1);
        assert!(!d.join(FILE_NAME).exists());
        assert_eq!(
            std::fs::read_to_string(d.join(CORRUPT_NAME)).unwrap(),
            "toolbar = [[[ not toml"
        );
        // and a later write starts a clean file
        set_in(&d, "toolbar", "full").unwrap();
        assert_eq!(load_in(&d).settings.toolbar, ToolbarMode::Full);
    }

    #[test]
    fn set_over_a_corrupt_file_keeps_the_backup() {
        let d = tmp("setcorrupt");
        std::fs::write(d.join(FILE_NAME), "{{{{").unwrap();
        set_in(&d, "lang", "ru").unwrap();
        assert_eq!(
            std::fs::read_to_string(d.join(CORRUPT_NAME)).unwrap(),
            "{{{{"
        );
        assert_eq!(load_in(&d).settings.lang, LangChoice::Fixed(Lang::Ru));
    }

    #[test]
    fn invalid_utf8_and_a_directory_in_the_way_are_not_fatal() {
        let d = tmp("odd");
        std::fs::write(d.join(FILE_NAME), [0xff, 0xfe, 0x00]).unwrap();
        assert_eq!(load_in(&d).settings, Settings::default());
        assert!(d.join(CORRUPT_NAME).exists());
        let d2 = tmp("dir");
        std::fs::create_dir(d2.join(FILE_NAME)).unwrap();
        assert_eq!(load_in(&d2).settings, Settings::default());
    }

    #[test]
    fn set_writes_atomically_and_keeps_everything_else() {
        let d = tmp("set");
        let user = "# my notes\nlang = \"ru\"\nextra = \"keep me\"\n\n[recent]\nlimit = 5 # five\n";
        std::fs::write(d.join(FILE_NAME), user).unwrap();
        set_in(&d, "toolbar", "full").unwrap();
        set_in(&d, "history.days", "30").unwrap();
        let text = std::fs::read_to_string(d.join(FILE_NAME)).unwrap();
        assert!(
            text.contains("# my notes")
                && text.contains("extra = \"keep me\"")
                && text.contains("limit = 5 # five"),
            "{text}"
        );
        let s = load_in(&d).settings;
        assert_eq!(
            (s.toolbar, s.history.days, s.lang),
            (ToolbarMode::Full, 30, LangChoice::Fixed(Lang::Ru))
        );
        let leftovers: Vec<_> = std::fs::read_dir(&d)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(leftovers, [FILE_NAME], "no temporary file is left");
    }

    #[test]
    fn set_creates_the_folder_and_the_file() {
        let d = tmp("create").join("a").join("b");
        set_in(&d, "lang", "uz-Cyrl").unwrap();
        assert_eq!(load_in(&d).settings.lang, LangChoice::Fixed(Lang::UzCyrl));
    }

    #[test]
    fn set_refuses_unknown_keys_and_bad_values_and_writes_nothing() {
        let d = tmp("refuse");
        for (k, v) in [
            ("nope", "x"),
            ("lang", "xx"),
            ("lang", ""),
            ("toolbar", "Full"),
            ("recent.limit", "51"),
            ("recent.limit", "-1"),
            ("recent.limit", "five"),
            ("history.versions", "0"),
            ("server.url", "ftp://x"),
            ("server.url", "https://a b"),
            ("spell_dictionaries", "a\nb"),
            ("xdfu", "yes"),
        ] {
            assert!(set_in(&d, k, v).is_err(), "{k}={v:?}");
        }
        assert_eq!(set_in(&d, "nope", "x"), Err(SetError::UnknownKey));
        assert!(!d.join(FILE_NAME).exists());
        set_in(&d, "server.url", "").unwrap();
        set_in(&d, "server.url", "https://example.org").unwrap();
    }

    #[test]
    fn the_map_round_trips_through_set() {
        let d = tmp("map");
        let want = Settings {
            toolbar: ToolbarMode::Full,
            theme: Theme::Dark,
            lang: LangChoice::Fixed(Lang::UzLatn2026),
            recent_limit: 0,
            xdfu: true,
            ..Settings::default()
        };
        for (k, v) in want.to_map() {
            set_in(&d, k, &v).unwrap();
        }
        let got = load_in(&d);
        assert!(got.warnings.is_empty());
        assert_eq!(got.settings, want);
        assert_eq!(key_names().count(), want.to_map().len());
    }

    #[test]
    fn the_dir_follows_the_override_in_debug_builds_and_the_product() {
        let over = |k: &str| (k == "SUITE_SETTINGS_DIR").then(|| PathBuf::from("x-override"));
        assert_eq!(
            dir_with(&over),
            cfg!(debug_assertions)
                .then(|| PathBuf::from("x-override"))
                .or_else(|| dir_with(&|_| None))
        );
        if let Some(d) = dir_with(&|_| None) {
            assert_eq!(d.file_name().and_then(|n| n.to_str()), Some(crate::PRODUCT));
        }
    }

    #[test]
    fn the_calls_the_lanes_make() {
        // toolbar_mode()/set_toolbar_mode() are load()/set_in() on dir(): the same functions, on an explicit folder here
        let d = tmp("lanes");
        assert_eq!(load_in(&d).settings.toolbar, ToolbarMode::Simple);
        set_in(&d, "toolbar", ToolbarMode::Full.tag()).unwrap();
        assert_eq!(load_in(&d).settings.toolbar, ToolbarMode::Full);
        set_in(&d, "toolbar", ToolbarMode::Simple.tag()).unwrap();
        assert_eq!(load_in(&d).settings.toolbar, ToolbarMode::Simple);
    }
}
