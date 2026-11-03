//! `plugin.toml` (PLAN phase 7, section 4.1): the types, the parser and every refusal.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Deserialize;
use serde_json::{Map, Value};

pub const MAX_PACKAGE: u64 = 8 << 20;
pub const MAX_WASM: u64 = 4 << 20;
pub const MAX_MANIFEST: usize = 64 << 10;
pub const MAX_ENTRIES: usize = 64;
pub const MAX_COMMANDS: usize = 16;
pub const MAX_TOOLBAR: usize = 8;
pub const MAX_MENU: usize = 8;
pub const MAX_PANELS: usize = 2;
pub const MAX_ORIGINS: usize = 8;
pub const MAX_LABEL: usize = 64;
pub const MAX_PUBLISHER: usize = 128;
/// The host cap of `memory_mb` (an admin may lower it).
pub const MEMORY_CAP_MB: u32 = 64;
const DEFAULT_MEMORY_MB: u32 = 16;
const MAX_SETTINGS_BYTES: usize = 4096;
const MAX_PARAMS_BYTES: usize = 1024;

pub const LANGS: &[&str] = &["en", "ru", "uz-Latn", "uz-Cyrl"];
pub const APPS: &[&str] = &["docs", "sheets", "slides", "pdf"];
/// The only names a package may hold (flat: a path, a directory or `..` is refused by the name alone).
pub const PACKAGE_FILES: &[&str] = &["plugin.toml", "plugin.wasm", "strings.tsv", "SIGNATURE"];

/// `en`, `ru`, `uz-Latn`, `uz-Cyrl` to text; `en` is required, a missing language falls back to it.
pub type Localized = BTreeMap<String, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocAccess {
    #[default]
    None,
    Read,
    Write,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    #[serde(default)]
    pub document: DocAccess,
    #[serde(default)]
    pub network: Vec<String>,
    #[serde(default = "default_memory")]
    pub memory_mb: u32,
}

impl Default for Permissions {
    fn default() -> Self {
        Permissions {
            document: DocAccess::None,
            network: Vec::new(),
            memory_mb: DEFAULT_MEMORY_MB,
        }
    }
}

fn default_memory() -> u32 {
    DEFAULT_MEMORY_MB
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReadKind {
    Textmap,
    Selection,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Event {
    DocumentOpened,
    DocumentSaved,
    SelectionChanged,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub id: String,
    pub label: Localized,
    #[serde(default)]
    pub reads: Vec<ReadKind>,
    #[serde(default)]
    pub params: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Toolbar {
    pub command: String,
    pub icon: Option<String>,
    pub group: Option<Localized>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum MenuPlace {
    #[serde(rename = "tools")]
    Tools,
    #[serde(rename = "context.text")]
    ContextText,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Menu {
    pub command: String,
    pub place: MenuPlace,
    pub group: Option<Localized>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Panel {
    pub id: String,
    pub title: Localized,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub id: String,
    pub version: String,
    pub api: u32,
    pub apps: Vec<String>,
    pub publisher: String,
    pub name: Localized,
    #[serde(default)]
    pub permissions: Permissions,
    // Top-level keys must come before the first table header in the file: after a `[[panel]]` they belong to the panel.
    #[serde(default)]
    pub events: Vec<Event>,
    #[serde(default, rename = "command")]
    pub commands: Vec<Command>,
    #[serde(default)]
    pub toolbar: Vec<Toolbar>,
    #[serde(default)]
    pub menu: Vec<Menu>,
    #[serde(default, rename = "panel")]
    pub panels: Vec<Panel>,
    #[serde(default)]
    pub settings: Map<String, Value>,
}

/// Why a manifest or a package is refused. Every variant is covered by a test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    TooLarge {
        len: usize,
        max: usize,
    },
    Syntax(String),
    UnknownKey(String),
    BadId(String),
    BadVersion(String),
    UnsupportedApi(u32),
    BadApps(String),
    /// A label, name, publisher or title that is empty, too long, in an unknown language or holds control/bidi characters.
    BadText {
        field: String,
        why: &'static str,
    },
    MemoryOutOfRange {
        mb: u32,
        cap: u32,
    },
    BadOrigin(String),
    TooMany {
        what: &'static str,
        n: usize,
        max: usize,
    },
    Duplicate {
        what: &'static str,
        id: String,
    },
    UnknownCommand(String),
    BadIcon(String),
    BadParams {
        field: &'static str,
        why: &'static str,
    },
    /// A command reads the document but the manifest asks for `document = "none"`.
    ReadsNeedDocument(String),
    Package(String),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::TooLarge { len, max } => {
                write!(f, "manifest is {len} bytes, the limit is {max}")
            }
            Refusal::Syntax(m) => write!(f, "manifest does not parse: {m}"),
            Refusal::UnknownKey(m) => write!(f, "unknown key: {m}"),
            Refusal::BadId(s) => write!(f, "bad id `{s}`"),
            Refusal::BadVersion(s) => {
                write!(f, "bad version `{s}` (expected MAJOR.MINOR.PATCH[-pre])")
            }
            Refusal::UnsupportedApi(n) => write!(
                f,
                "api {n} is not supported (host runs {:?})",
                crate::HOST_APIS
            ),
            Refusal::BadApps(s) => write!(f, "bad apps: {s}"),
            Refusal::BadText { field, why } => write!(f, "{field}: {why}"),
            Refusal::MemoryOutOfRange { mb, cap } => {
                write!(f, "memory_mb {mb} is outside 1..={cap}")
            }
            Refusal::BadOrigin(s) => write!(f, "`{s}` is not an https://host[:port] origin"),
            Refusal::TooMany { what, n, max } => write!(f, "{n} {what}, the limit is {max}"),
            Refusal::Duplicate { what, id } => write!(f, "duplicate {what} `{id}`"),
            Refusal::UnknownCommand(s) => write!(f, "`{s}` is not a declared command"),
            Refusal::BadIcon(s) => write!(f, "bad icon name `{s}`"),
            Refusal::BadParams { field, why } => write!(f, "{field}: {why}"),
            Refusal::ReadsNeedDocument(s) => write!(
                f,
                "command `{s}` reads the document but permissions.document is none"
            ),
            Refusal::Package(s) => write!(f, "package: {s}"),
        }
    }
}

impl std::error::Error for Refusal {}

/// Parse and validate `plugin.toml`. Nothing about the manifest is trusted before this returns `Ok`.
pub fn parse(text: &str) -> Result<Manifest, Refusal> {
    if text.len() > MAX_MANIFEST {
        return Err(Refusal::TooLarge {
            len: text.len(),
            max: MAX_MANIFEST,
        });
    }
    let m: Manifest = toml::from_str(text).map_err(|e| {
        let s = e.message().to_string();
        if s.starts_with("unknown field") {
            Refusal::UnknownKey(s)
        } else {
            Refusal::Syntax(s)
        }
    })?;
    validate(&m)?;
    Ok(m)
}

/// `parse`, and the manifest as JSON with the file's own key names (what a page host gets in the descriptor).
pub fn parse_json(text: &str) -> Result<(Manifest, Value), Refusal> {
    let m = parse(text)?;
    let v = toml::from_str::<toml::Value>(text)
        .ok()
        .and_then(|t| serde_json::to_value(t).ok())
        .ok_or_else(|| Refusal::Syntax("manifest to JSON".into()))?;
    Ok((m, v))
}

fn ident_ok(s: &str, extra: &[char], max: usize) -> bool {
    !s.is_empty()
        && s.len() <= max
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || extra.contains(&c))
}

fn plugin_id_ok(s: &str) -> bool {
    let edge = |c: Option<char>| c.is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    (3..=64).contains(&s.len())
        && ident_ok(s, &['-', '.'], 64)
        && edge(s.chars().next())
        && edge(s.chars().last())
        && !s.contains("..")
}

/// A command or panel id: letters, digits, `_`, `-`; no `.` so that `plugin.<plugin id>.<command id>` splits at the last dot.
fn local_id_ok(s: &str) -> bool {
    s.len() <= 48
        && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn semver_ok(s: &str) -> bool {
    let (core, pre) = match s.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (s, None),
    };
    let nums: Vec<&str> = core.split('.').collect();
    s.len() <= 32
        && nums.len() == 3
        && nums.iter().all(|n| {
            !n.is_empty()
                && n.len() <= 9
                && n.bytes().all(|b| b.is_ascii_digit())
                && (*n == "0" || !n.starts_with('0'))
        })
        && pre.is_none_or(|p| {
            !p.is_empty()
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        })
}

/// Control characters and the invisible/bidi format characters that make a label read differently from its bytes.
fn spoofy(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{200B}'..='\u{200F}' | '\u{2028}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
}

/// Text shown to the user: not empty, at most `max` characters, nothing that spoofs.
pub fn check_text(field: &str, s: &str, max: usize) -> Result<(), Refusal> {
    let bad = |why| {
        Err(Refusal::BadText {
            field: field.to_string(),
            why,
        })
    };
    if s.trim().is_empty() {
        return bad("empty");
    }
    if s.chars().count() > max {
        return bad("too long");
    }
    if s.chars().any(spoofy) {
        return bad("control or bidi/invisible character");
    }
    Ok(())
}

fn check_localized(field: &str, l: &Localized, required: bool) -> Result<(), Refusal> {
    if required && !l.contains_key("en") {
        return Err(Refusal::BadText {
            field: field.to_string(),
            why: "missing the `en` text",
        });
    }
    for (lang, text) in l {
        if !LANGS.contains(&lang.as_str()) {
            return Err(Refusal::BadText {
                field: format!("{field}.{lang}"),
                why: "unknown language",
            });
        }
        check_text(&format!("{field}.{lang}"), text, MAX_LABEL)?;
    }
    Ok(())
}

/// `https://host[:port]`: lowercase ASCII host with at least two labels, no IP literal, no `localhost`, no userinfo, path,
/// query or trailing slash. (Private addresses behind a public name are the host's check at connect time.)
pub fn check_origin(o: &str) -> bool {
    let Some(rest) = o.strip_prefix("https://") else {
        return false;
    };
    let (host, port) = match rest.split_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (rest, None),
    };
    if let Some(p) = port
        && (p.is_empty()
            || p.len() > 5
            || !p.bytes().all(|b| b.is_ascii_digit())
            || p.parse::<u32>().is_ok_and(|n| n == 0 || n > 65535))
    {
        return false;
    }
    let labels: Vec<&str> = host.split('.').collect();
    host.len() <= 253
        && labels.len() >= 2
        && labels.iter().all(|l| {
            (1..=63).contains(&l.len())
                && l.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                && !l.starts_with('-')
                && !l.ends_with('-')
        })
        && !labels
            .last()
            .is_some_and(|l| l.bytes().all(|b| b.is_ascii_digit()))
        && labels.last() != Some(&"localhost")
}

fn check_map(
    field: &'static str,
    m: &Map<String, Value>,
    max_keys: usize,
    max_bytes: usize,
) -> Result<(), Refusal> {
    if m.len() > max_keys {
        return Err(Refusal::BadParams {
            field,
            why: "too many keys",
        });
    }
    // The encoded size bounds nesting depth and string lengths together.
    if serde_json::to_string(m).map_or(true, |s| s.len() > max_bytes) {
        return Err(Refusal::BadParams {
            field,
            why: "too large",
        });
    }
    Ok(())
}

fn too_many(what: &'static str, n: usize, max: usize) -> Result<(), Refusal> {
    if n > max {
        Err(Refusal::TooMany { what, n, max })
    } else {
        Ok(())
    }
}

fn unique<'a>(
    what: &'static str,
    ids: impl Iterator<Item = &'a str>,
) -> Result<BTreeSet<&'a str>, Refusal> {
    let mut set = BTreeSet::new();
    for id in ids {
        if !set.insert(id) {
            return Err(Refusal::Duplicate {
                what,
                id: id.to_string(),
            });
        }
    }
    Ok(set)
}

/// Every refusal of PLAN 4.1 (and the additions listed in the plan text). Public so a host can re-check a built manifest.
pub fn validate(m: &Manifest) -> Result<(), Refusal> {
    if !plugin_id_ok(&m.id) {
        return Err(Refusal::BadId(m.id.clone()));
    }
    if !semver_ok(&m.version) {
        return Err(Refusal::BadVersion(m.version.clone()));
    }
    if !crate::supports(m.api) {
        return Err(Refusal::UnsupportedApi(m.api));
    }
    if m.apps.is_empty() || m.apps.len() > APPS.len() {
        return Err(Refusal::BadApps("one to four apps".into()));
    }
    if let Some(a) = m.apps.iter().find(|a| !APPS.contains(&a.as_str())) {
        return Err(Refusal::BadApps(format!("unknown app `{a}`")));
    }
    unique("app", m.apps.iter().map(String::as_str))?;
    check_text("publisher", &m.publisher, MAX_PUBLISHER)?;
    check_localized("name", &m.name, true)?;

    let p = &m.permissions;
    if p.memory_mb == 0 || p.memory_mb > MEMORY_CAP_MB {
        return Err(Refusal::MemoryOutOfRange {
            mb: p.memory_mb,
            cap: MEMORY_CAP_MB,
        });
    }
    too_many("network origins", p.network.len(), MAX_ORIGINS)?;
    if let Some(o) = p.network.iter().find(|o| !check_origin(o)) {
        return Err(Refusal::BadOrigin(o.clone()));
    }
    unique("origin", p.network.iter().map(String::as_str))?;

    too_many("commands", m.commands.len(), MAX_COMMANDS)?;
    too_many("toolbar items", m.toolbar.len(), MAX_TOOLBAR)?;
    too_many("menu items", m.menu.len(), MAX_MENU)?;
    too_many("panels", m.panels.len(), MAX_PANELS)?;
    for c in &m.commands {
        if !local_id_ok(&c.id) {
            return Err(Refusal::BadId(c.id.clone()));
        }
        check_localized(&format!("command.{}.label", c.id), &c.label, true)?;
        check_map("command params", &c.params, 16, MAX_PARAMS_BYTES)?;
        if p.document == DocAccess::None && c.reads.iter().any(|r| *r != ReadKind::None) {
            return Err(Refusal::ReadsNeedDocument(c.id.clone()));
        }
    }
    let commands = unique("command", m.commands.iter().map(|c| c.id.as_str()))?;
    for t in &m.toolbar {
        if !commands.contains(t.command.as_str()) {
            return Err(Refusal::UnknownCommand(t.command.clone()));
        }
        if let Some(i) = &t.icon
            && !ident_ok(i, &['-', '_'], 32)
        {
            return Err(Refusal::BadIcon(i.clone()));
        }
        if let Some(g) = &t.group {
            check_localized("toolbar.group", g, false)?;
        }
    }
    for it in &m.menu {
        if !commands.contains(it.command.as_str()) {
            return Err(Refusal::UnknownCommand(it.command.clone()));
        }
        if let Some(g) = &it.group {
            check_localized("menu.group", g, false)?;
        }
    }
    for pn in &m.panels {
        if !local_id_ok(&pn.id) {
            return Err(Refusal::BadId(pn.id.clone()));
        }
        check_localized(&format!("panel.{}.title", pn.id), &pn.title, true)?;
    }
    unique("panel", m.panels.iter().map(|p| p.id.as_str()))?;
    unique("event", m.events.iter().map(|e| e.name()))?;
    check_map("settings", &m.settings, 32, MAX_SETTINGS_BYTES)?;
    Ok(())
}

impl Event {
    pub fn name(self) -> &'static str {
        match self {
            Event::DocumentOpened => "documentOpened",
            Event::DocumentSaved => "documentSaved",
            Event::SelectionChanged => "selectionChanged",
        }
    }
}

/// The registry id a plugin command gets (section 4.1).
pub fn registry_id(plugin: &str, command: &str) -> String {
    format!("plugin.{plugin}.{command}")
}

/// Check the entries of a package (name, uncompressed size) before any of them is read: only the four known flat names,
/// no duplicates, both required files present, sizes within the limits. The zip reader supplies the declared sizes and must
/// still stop reading at them.
pub fn check_package(entries: &[(&str, u64)]) -> Result<(), Refusal> {
    let bad = |s: &str| Err(Refusal::Package(s.to_string()));
    if entries.len() > MAX_ENTRIES {
        return bad("more than 64 entries");
    }
    let mut seen = BTreeSet::new();
    let mut total = 0u64;
    for (name, size) in entries {
        if !PACKAGE_FILES.contains(name) {
            return Err(Refusal::Package(format!(
                "entry `{}` is not allowed",
                name.escape_debug()
            )));
        }
        if !seen.insert(*name) {
            return bad("duplicate entry");
        }
        if *name == "plugin.wasm" && *size > MAX_WASM {
            return bad("plugin.wasm is over 4 MiB");
        }
        if *name == "plugin.toml" && *size > MAX_MANIFEST as u64 {
            return bad("plugin.toml is over 64 KiB");
        }
        total = total.saturating_add(*size);
    }
    if total > MAX_PACKAGE {
        return bad("package is over 8 MiB");
    }
    for need in ["plugin.toml", "plugin.wasm"] {
        if !seen.contains(need) {
            return Err(Refusal::Package(format!("missing {need}")));
        }
    }
    Ok(())
}
