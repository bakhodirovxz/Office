//! The canonical catalog: row model, TSV reading and writing, ids, statuses, plural forms.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub const HEADER: [&str; 14] = [
    "id", "context", "source_en", "uz_latn", "uz_latn_status", "uz_cyrl", "uz_cyrl_status", "ru", "ru_status", "ru_apps", "priority", "apps",
    "where", "note",
];

pub const APPS: [&str; 4] = ["wordcraft", "gridcraft", "pdfcraft", "deckcraft"];
/// Catalog files, in load order. `glossary` and `shared` are scopes of their own.
pub const SCOPES: [&str; 6] = ["glossary", "shared", "wordcraft", "gridcraft", "pdfcraft", "deckcraft"];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub context: String,
    pub source: String,
    pub uz_latn: String,
    pub uz_latn_status: String,
    pub uz_cyrl: String,
    pub uz_cyrl_status: String,
    pub ru: String,
    pub ru_status: String,
    /// Apps whose upstream `ru` carries this key (shared rows only; empty = every app in `apps`).
    pub ru_apps: String,
    pub priority: String,
    pub apps: String,
    pub where_: String,
    pub note: String,
}

impl Row {
    pub fn value(&self, locale: &str) -> &str {
        match locale {
            "uz-latn" => &self.uz_latn,
            "uz-cyrl" => &self.uz_cyrl,
            "ru" => &self.ru,
            _ => &self.source,
        }
    }
    pub fn status(&self, locale: &str) -> &str {
        match locale {
            "uz-latn" => &self.uz_latn_status,
            "uz-cyrl" => &self.uz_cyrl_status,
            "ru" => &self.ru_status,
            _ => "reviewed:source",
        }
    }
    pub fn set(&mut self, locale: &str, value: String, status: String) {
        match locale {
            "uz-latn" => (self.uz_latn, self.uz_latn_status) = (value, status),
            "uz-cyrl" => (self.uz_cyrl, self.uz_cyrl_status) = (value, status),
            _ => (self.ru, self.ru_status) = (value, status),
        }
    }
    /// `P1` (all apps of the row), `P1:app+app` (P1 only in the listed apps), or `P2`.
    pub fn is_p1(&self, app: &str) -> bool {
        self.priority == "P1" || self.priority.strip_prefix("P1:").is_some_and(|l| l.split('+').any(|a| a == app))
    }
    pub fn has_app(&self, app: &str) -> bool {
        self.apps.split(',').any(|a| a == app)
    }
    /// Whether the `ru` value of this row belongs to `app` (see `ru_apps`).
    pub fn ru_in_app(&self, app: &str) -> bool {
        !self.ru.is_empty() && (self.ru_apps.is_empty() || self.ru_apps.split(',').any(|a| a == app))
    }
}

/// The part of a status before the first `:`.
pub fn state(status: &str) -> &str {
    status.split(':').next().unwrap_or("")
}

/// Only `reviewed:owner[:date]` is a trusted Uzbek value; `upstream` (shipped ru) is trusted as shipped.
pub fn is_trusted(status: &str) -> bool {
    status.starts_with("reviewed:owner") || state(status) == "upstream"
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Review,
    Release,
}

/// May a value with this status be exported in `mode`?
pub fn exportable(status: &str, mode: Mode) -> bool {
    match mode {
        Mode::Release => is_trusted(status),
        Mode::Review => is_trusted(status) || state(status) == "draft" || state(status) == "reviewed",
    }
}

pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

pub fn unescape(text: &str) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(o) => return Err(format!("unknown escape \\{o}")),
            None => return Err("trailing backslash".into()),
        }
    }
    Ok(out)
}

/// SHA-1 (RFC 3174), for stable row ids. Not used for security.
pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476, 0xC3D2_E1F0];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for (i, b) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6u32),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            (e, d, c, b, a) = (d, c, b.rotate_left(30), a, t);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 20];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

/// The first 12 hex digits of SHA-1 over `scope \x1f context \x1f source_en`.
pub fn row_id(scope: &str, context: &str, source: &str) -> String {
    let digest = sha1(format!("{scope}\u{1f}{context}\u{1f}{source}").as_bytes());
    digest.iter().take(6).map(|b| format!("{b:02x}")).collect()
}

/// All catalog files of the tree: scope name -> rows (sorted by id on write).
#[derive(Debug, Default, Clone)]
pub struct Catalog {
    pub scopes: BTreeMap<String, Vec<Row>>,
}

impl Catalog {
    pub fn load(dir: &Path) -> Result<Catalog, String> {
        let mut cat = Catalog::default();
        for scope in SCOPES {
            let path = dir.join(format!("{scope}.tsv"));
            if path.exists() {
                let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                cat.scopes.insert(scope.to_string(), parse_rows(&text).map_err(|e| format!("{}: {e}", path.display()))?);
            }
        }
        Ok(cat)
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        for (scope, rows) in &self.scopes {
            let mut rows = rows.clone();
            rows.sort_by(|a, b| a.id.cmp(&b.id));
            fs::write(dir.join(format!("{scope}.tsv")), write_rows(&rows)).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// The rows an app uses: its own scope and the shared rows that list it. Obsolete rows are left out.
    pub fn app_rows<'a>(&'a self, app: &str) -> Vec<&'a Row> {
        let own = self.scopes.get(app).into_iter().flatten();
        let shared = self.scopes.get("shared").into_iter().flatten().filter(|r| r.has_app(app));
        own.chain(shared).filter(|r| r.priority != "obsolete").collect()
    }
}

pub fn parse_rows(text: &str) -> Result<Vec<Row>, String> {
    let mut rows = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if n == 0 {
            if line.split('\t').collect::<Vec<_>>() != HEADER {
                return Err("line 1: header does not match the catalog format".into());
            }
            continue;
        }
        if line.is_empty() {
            continue;
        }
        let cells: Vec<&str> = line.split('\t').collect();
        if cells.len() != HEADER.len() {
            return Err(format!("line {}: {} cells, expected {}", n + 1, cells.len(), HEADER.len()));
        }
        let u = |i: usize| unescape(cells[i]).map_err(|e| format!("line {}: {e}", n + 1));
        rows.push(Row {
            id: cells[0].into(),
            context: u(1)?,
            source: u(2)?,
            uz_latn: u(3)?,
            uz_latn_status: cells[4].into(),
            uz_cyrl: u(5)?,
            uz_cyrl_status: cells[6].into(),
            ru: u(7)?,
            ru_status: cells[8].into(),
            ru_apps: cells[9].into(),
            priority: cells[10].into(),
            apps: cells[11].into(),
            where_: u(12)?,
            note: u(13)?,
        });
    }
    Ok(rows)
}

pub fn write_rows(rows: &[Row]) -> String {
    let mut out = HEADER.join("\t");
    out.push('\n');
    for r in rows {
        let cells = [
            r.id.clone(),
            escape(&r.context),
            escape(&r.source),
            escape(&r.uz_latn),
            r.uz_latn_status.clone(),
            escape(&r.uz_cyrl),
            r.uz_cyrl_status.clone(),
            escape(&r.ru),
            r.ru_status.clone(),
            r.ru_apps.clone(),
            r.priority.clone(),
            r.apps.clone(),
            escape(&r.where_),
            escape(&r.note),
        ];
        out.push_str(&cells.join("\t"));
        out.push('\n');
    }
    out
}

/// Number of `|`-separated plural forms.
pub fn plural_forms(value: &str) -> usize {
    value.split('|').count()
}

/// Bring a plural value (`one|few|many`) to the form count of the target language.
/// Equal counts pass through. Three forms (ru) to two (uz: one, other) keeps the first and the last
/// (CLDR: ru `many` covers 0 and 5+, the closest to `other`). Anything else cannot be adapted safely.
pub fn adapt_plural(value: &str, to: usize) -> Option<String> {
    let forms: Vec<&str> = value.split('|').collect();
    if forms.len() == to {
        return Some(value.to_string());
    }
    match (forms.len(), to) {
        (3, 2) => Some(format!("{}|{}", forms[0], forms[2])),
        (n, 1) if n >= 1 => Some(forms[n - 1].to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_known_vectors() {
        let hex = |d: [u8; 20]| d.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(hex(sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(hex(sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(row_id("s", "", "x").len(), 12);
    }

    #[test]
    fn escape_round_trip() {
        let s = "a\tb\nc\\d\re";
        assert_eq!(unescape(&escape(s)).unwrap(), s);
        assert!(unescape("bad\\q").is_err());
    }

    #[test]
    fn rows_round_trip() {
        let r = Row {
            id: row_id("shared", "", "Top\nAlign"),
            source: "Top\nAlign".into(),
            uz_latn: "Yuqori".into(),
            uz_latn_status: "draft:agent:2026-10-18".into(),
            ru_status: "upstream".into(),
            priority: "P1".into(),
            apps: "gridcraft,wordcraft".into(),
            ..Row::default()
        };
        let text = write_rows(std::slice::from_ref(&r));
        assert_eq!(parse_rows(&text).unwrap(), vec![r]);
    }

    #[test]
    fn plural_adaptation() {
        assert_eq!(adapt_plural("{n} a|{n} b", 2).as_deref(), Some("{n} a|{n} b"));
        assert_eq!(adapt_plural("{n} страница|{n} страницы|{n} страниц", 2).as_deref(), Some("{n} страница|{n} страниц"));
        assert_eq!(adapt_plural("a|b", 3), None);
        assert_eq!(plural_forms("a|b|c"), 3);
    }

    #[test]
    fn export_gate_by_status() {
        assert!(exportable("reviewed:owner:2026-10-21", Mode::Release));
        assert!(!exportable("draft:agent-sonnet:2026-10-18", Mode::Release));
        assert!(exportable("draft:agent-sonnet:2026-10-18", Mode::Review));
        assert!(exportable("upstream", Mode::Release));
        assert!(!exportable("todo", Mode::Review));
        assert!(!exportable("rejected", Mode::Review));
    }
}
