//! `i18n export`: native catalog files for the build-time overlay, with the fallback chain applied
//! at export time (design section 5): the app only ever sees one catalog and keeps its own
//! "missing means English" behaviour. The exporter never writes English text into a locale column.

use super::catalog::{adapt_plural, escape, exportable, plural_forms, Catalog, Mode, Row};
use super::extract::{parse_rust_table, read_upstream_tsv, split_grid_key};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::DocumentMut;

pub const EXPORT_LOCALES: [&str; 2] = ["uz-latn", "uz-cyrl"];

/// Fallback chains per locale (from `i18n/config.toml`; defaults are the owner's decision of 2026-10-17).
#[derive(Debug, Clone)]
pub struct Chains(pub BTreeMap<String, Vec<String>>);

impl Default for Chains {
    fn default() -> Self {
        let mut m = BTreeMap::new();
        for (k, v) in [("uz-latn", vec!["uz-latn", "en"]), ("uz-cyrl", vec!["uz-cyrl", "ru", "en"]), ("ru", vec!["ru", "en"])] {
            m.insert(k.to_string(), v.into_iter().map(String::from).collect());
        }
        Chains(m)
    }
}

impl Chains {
    pub fn parse(text: &str) -> Result<Chains, String> {
        let doc = text.parse::<DocumentMut>().map_err(|e| format!("config.toml: {e}"))?;
        let mut chains = Chains::default();
        if let Some(t) = doc.get("chains").and_then(|c| c.as_table()) {
            for (loc, v) in t.iter() {
                let arr = v.as_array().ok_or_else(|| format!("config.toml: chains.{loc} must be an array"))?;
                let list: Vec<String> = arr.iter().filter_map(|x| x.as_str().map(String::from)).collect();
                if list.first().map(String::as_str) != Some(loc) {
                    return Err(format!("config.toml: chains.{loc} must start with {loc}"));
                }
                chains.0.insert(loc.to_string(), list);
            }
        }
        Ok(chains)
    }
    pub fn load(i18n_dir: &Path) -> Result<Chains, String> {
        match fs::read_to_string(i18n_dir.join("config.toml")) {
            Ok(t) => Chains::parse(&t),
            Err(_) => Ok(Chains::default()),
        }
    }
    pub fn of(&self, locale: &str) -> Vec<String> {
        self.0.get(locale).cloned().unwrap_or_else(|| vec![locale.to_string(), "en".into()])
    }
}

/// Number of plural forms a locale's catalog uses (pdfcraft: `plural_one_other` for uz, three for ru).
pub fn plural_count(locale: &str) -> usize {
    if locale == "ru" { 3 } else { 2 }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub context: String,
    pub source: String,
    pub value: String,
}

#[derive(Debug, Default)]
pub struct Built {
    pub entries: Vec<Entry>,
    /// Entries per chain locale that supplied the value.
    pub from: BTreeMap<String, usize>,
    /// Rows whose own-locale value was a draft (review mode) or reviewed.
    pub own_draft: usize,
    pub own_reviewed: usize,
    pub skipped_plural: usize,
}

/// The value a row gives to `locale` through its chain: (value, supplying locale).
pub fn resolve(row: &Row, app: &str, locale: &str, chains: &Chains, mode: Mode) -> Option<(String, String, bool)> {
    for step in chains.of(locale) {
        if step == "en" {
            return None; // the app falls back to English by itself
        }
        let v = row.value(&step);
        if v.is_empty() || !exportable(row.status(&step), mode) || (step == "ru" && !row.ru_in_app(app)) {
            continue;
        }
        let v = if row.context == "@plural" && step != locale {
            match adapt_plural(v, plural_count(locale)) {
                Some(a) => a,
                None => continue,
            }
        } else {
            v.to_string()
        };
        if row.context == "@plural" && plural_forms(&v) != plural_count(locale) {
            continue;
        }
        let reviewed = row.status(&step).starts_with("reviewed") || row.status(&step) == "upstream";
        return Some((v, step, reviewed));
    }
    None
}

fn sort_entries(entries: &mut [Entry]) {
    entries.sort_by(|a, b| a.source.to_lowercase().cmp(&b.source.to_lowercase()).then_with(|| a.source.cmp(&b.source)).then_with(|| a.context.cmp(&b.context)));
}

pub fn build_locale(cat: &Catalog, app: &str, locale: &str, chains: &Chains, mode: Mode) -> Built {
    let mut b = Built::default();
    for row in cat.app_rows(app) {
        if row.context.starts_with("date.") {
            continue;
        }
        let Some((value, step, reviewed)) = resolve(row, app, locale, chains, mode) else {
            if row.context == "@plural" && !row.value(locale).is_empty() {
                b.skipped_plural += 1;
            }
            continue;
        };
        if step == locale {
            if reviewed { b.own_reviewed += 1 } else { b.own_draft += 1 }
        }
        *b.from.entry(step).or_default() += 1;
        b.entries.push(Entry { context: row.context.clone(), source: row.source.clone(), value });
    }
    sort_entries(&mut b.entries);
    b
}

fn header(app: &str, locale: &str, mode: Mode) -> String {
    let label = match mode {
        Mode::Review => "REVIEW build: includes unreviewed DRAFT translations",
        Mode::Release => "RELEASE build: reviewed or upstream rows only",
    };
    let cols = if app == "gridcraft" { "key (`context|label`, or the label) <TAB> translation" } else { "context <TAB> English source <TAB> translation" };
    format!(
        "# {locale} interface catalog for {app}, generated by `cargo xt i18n export` from i18n/*.tsv. Do not edit.\n# {label}.\n# Columns, separated by a TAB: {cols}.\n# Escapes: \\n \\t \\\\ . Lines starting with # and blank lines are ignored. A missing entry shows in English.\n# Clean room: every entry was written from the plain meaning of the English label; no proprietary localisation was consulted.\n\n"
    )
}

/// The native overlay file text.
pub fn render(app: &str, locale: &str, entries: &[Entry], mode: Mode) -> String {
    let mut out = header(app, locale, mode);
    for e in entries {
        if app == "gridcraft" {
            let key = if e.context.is_empty() { e.source.clone() } else { format!("{}|{}", e.context, e.source) };
            out.push_str(&format!("{}\t{}\n", escape(&key), escape(&e.value)));
        } else {
            out.push_str(&format!("{}\t{}\t{}\n", escape(&e.context), escape(&e.source), escape(&e.value)));
        }
    }
    out
}

pub fn rust_literal(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// What gridcraft's `build_office.rs` generates from the overlay TSV: a `match` shaped like upstream's tables.
pub fn rust_match_fn(name: &str, entries: &[Entry]) -> String {
    let mut out = format!("pub(super) fn {name}(text: &str) -> Option<&'static str> {{\n    Some(match text {{\n");
    for e in entries {
        let key = if e.context.is_empty() { e.source.clone() } else { format!("{}|{}", e.context, e.source) };
        out.push_str(&format!("        {} => {},\n", rust_literal(&key), rust_literal(&e.value)));
    }
    out.push_str("        _ => return None,\n    })\n}\n");
    out
}

/// `dates-<code>.tsv`: 12 months, 12 short months, 7 days, 7 short days. All 38 or nothing.
pub fn build_dates(cat: &Catalog, locale: &str, chains: &Chains, mode: Mode) -> Result<String, String> {
    let rows: Vec<&Row> = cat.app_rows("pdfcraft").into_iter().filter(|r| r.context.starts_with("date.")).collect();
    let mut found: Vec<(String, u32, String, String)> = Vec::new();
    for r in &rows {
        if let Some((v, _, _)) = resolve(r, "pdfcraft", locale, chains, mode) {
            let mut parts = r.context.rsplitn(2, '.');
            let n = parts.next().and_then(|n| n.parse::<u32>().ok()).unwrap_or(0);
            found.push((parts.next().unwrap_or("").to_string(), n, r.source.clone(), v));
        }
    }
    if rows.len() != 38 || found.len() != 38 {
        return Err(format!("{}/{} of 38 date rows available for {locale} (need all 38)", found.len(), rows.len()));
    }
    found.sort();
    let mut out = String::from("# Month and weekday names for Preferences > Date format, generated by `cargo xt i18n export`.\n# kind.index <TAB> English <TAB> name\n\n");
    for (kind, n, en, v) in found {
        out.push_str(&format!("{kind}.{n}\t{}\t{}\n", escape(&en), escape(&v)));
    }
    Ok(out)
}

pub fn overlay_root() -> Result<PathBuf, String> {
    if let Ok(v) = std::env::var("OFFICE_OVERLAY_ROOT") {
        return Ok(PathBuf::from(v));
    }
    std::env::var("OFFICE_TARGET").map(|t| PathBuf::from(t).join("overlay")).map_err(|_| "set OFFICE_OVERLAY_ROOT or OFFICE_TARGET (source scripts/env.sh), or pass --out".to_string())
}

/// Export one app. Returns report lines.
pub fn export_app(cat: &Catalog, app: &str, mode: Mode, chains: &Chains, out_root: &Path, with_ru: bool) -> Result<Vec<String>, String> {
    let dir = out_root.join(app);
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut report = Vec::new();
    let mut meta = vec![format!("MODE={}", if mode == Mode::Review { "review" } else { "release" })];
    let mut locales: Vec<&str> = EXPORT_LOCALES.to_vec();
    if with_ru {
        locales.push("ru");
    }
    for loc in locales {
        let b = build_locale(cat, app, loc, chains, mode);
        let path = dir.join(format!("catalog-{loc}.tsv"));
        fs::write(&path, render(app, loc, &b.entries, mode)).map_err(|e| format!("{}: {e}", path.display()))?;
        let from: Vec<String> = b.from.iter().map(|(k, v)| format!("{k}={v}")).collect();
        report.push(format!("{app} {loc}: {} entries ({}) -> {}", b.entries.len(), from.join(", "), path.display()));
        let key = loc.to_uppercase().replace('-', "_");
        meta.push(format!("{key}_REVIEWED={}", b.own_reviewed));
        meta.push(format!("{key}_DRAFT={}", b.own_draft));
        meta.push(format!("{key}_FILLED={}", b.entries.len() - b.own_reviewed - b.own_draft));
    }
    if app == "pdfcraft" {
        for loc in EXPORT_LOCALES {
            match build_dates(cat, loc, chains, mode) {
                Ok(text) => {
                    fs::write(dir.join(format!("dates-{loc}.tsv")), text).map_err(|e| e.to_string())?;
                    report.push(format!("pdfcraft dates-{loc}.tsv: 38 rows"));
                }
                Err(e) => report.push(format!("pdfcraft dates-{loc}.tsv NOT written: {e}")),
            }
        }
    }
    fs::write(dir.join("catalog-meta.env"), meta.join("\n") + "\n").map_err(|e| e.to_string())?;
    Ok(report)
}

/// Round trip: extract -> export `ru` with no Uzbek -> compare with upstream's own ru catalog.
pub fn roundtrip(root: &Path, cat: &Catalog, chains: &Chains, out_root: &Path) -> Result<Vec<String>, String> {
    let mut report = Vec::new();
    let mut failed = false;
    for app in ["wordcraft", "gridcraft", "pdfcraft"] {
        let built = build_locale(cat, app, "ru", chains, Mode::Release);
        let dir = out_root.join("roundtrip").join(app);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let text = render(app, "ru", &built.entries, Mode::Release);
        fs::write(dir.join("catalog-ru.tsv"), &text).map_err(|e| e.to_string())?;
        let base = root.join("upstream").join(app).join("crates/ui-egui/src");
        // upstream ru as (context, source, value) in file order
        let (up, up_text_lines): (Vec<(String, String, String)>, Vec<String>) = if app == "gridcraft" {
            let src = fs::read_to_string(base.join("i18n/ru.rs")).map_err(|e| e.to_string())?;
            let (pairs, _) = parse_rust_table(&src, None);
            let entries = pairs.iter().map(|(k, v)| { let (c, l) = split_grid_key(k); (c, l, v.clone()) }).collect();
            (entries, Vec::new())
        } else {
            let t = fs::read_to_string(base.join("i18n/ru.tsv")).map_err(|e| e.to_string())?;
            let lines = t.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).map(String::from).collect();
            (read_upstream_tsv(&t).0, lines)
        };
        let ours: BTreeSet<(String, String, String)> = built.entries.iter().map(|e| (e.context.clone(), e.source.clone(), e.value.clone())).collect();
        let theirs: BTreeSet<(String, String, String)> = up.iter().cloned().collect();
        let semantic = ours == theirs && built.entries.len() == up.len();
        // Re-read our own file with the same parser upstream-compatible readers use.
        let reparsed: BTreeSet<(String, String, String)> = if app == "gridcraft" {
            let mut rs: BTreeSet<_> = BTreeSet::new();
            let code = rust_match_fn("translate", &built.entries);
            for (k, v) in parse_rust_table(&code, None).0 {
                let (c, l) = split_grid_key(&k);
                rs.insert((c, l, v));
            }
            rs
        } else {
            read_upstream_tsv(&text).0.into_iter().collect()
        };
        let reparse_ok = reparsed == theirs;
        let mut mine: Vec<String> = text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).map(String::from).collect();
        let (lines_equal_sorted, bytes_in_order) = if app == "gridcraft" {
            (false, false)
        } else {
            let mut a = mine.clone();
            let mut b = up_text_lines.clone();
            a.sort();
            b.sort();
            (a == b, mine == up_text_lines)
        };
        mine.clear();
        report.push(format!(
            "{app}: upstream ru {} entries, exported {} | semantic equal: {semantic} | re-parse of export equal: {reparse_ok}{}",
            up.len(),
            built.entries.len(),
            if app == "gridcraft" { " (via generated Rust match)".to_string() } else { format!(" | entry lines identical as a set: {lines_equal_sorted} | identical in order: {bytes_in_order}") }
        ));
        failed |= !(semantic && reparse_ok);
        // No Uzbek at all: uz-Latn must be empty, uz-Cyrl must equal the ru chain fill.
        let latn = build_locale(cat, app, "uz-latn", chains, Mode::Release);
        let cyrl = build_locale(cat, app, "uz-cyrl", chains, Mode::Release);
        report.push(format!("{app}: with no Uzbek, release export uz-latn = {} entries, uz-cyrl = {} entries (all from ru: {})", latn.entries.len(), cyrl.entries.len(), cyrl.from.keys().all(|k| k == "ru")));
        failed |= !latn.entries.is_empty();
    }
    if failed {
        report.push("ROUND TRIP FAILED".into());
        return Err(report.join("\n"));
    }
    report.push("round trip OK (en is the key itself: nothing to export)".into());
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::catalog::Row;

    fn row(ctx: &str, src: &str) -> Row {
        Row { id: src.into(), context: ctx.into(), source: src.into(), apps: "pdfcraft,wordcraft".into(), priority: "P1".into(), uz_latn_status: "todo".into(), uz_cyrl_status: "todo".into(), ru_status: "todo".into(), ..Row::default() }
    }

    fn cat_of(rows: Vec<Row>) -> Catalog {
        let mut c = Catalog::default();
        c.scopes.insert("shared".into(), rows);
        c
    }

    #[test]
    fn fallback_chain_uz_cyrl_to_ru_and_latn_to_en() {
        let mut a = row("", "Paste");
        a.ru = "Вставить".into();
        a.ru_status = "upstream".into();
        let mut b = row("", "Copy");
        b.uz_cyrl = "Нусхалаш".into();
        b.uz_cyrl_status = "reviewed:owner:2026-10-21".into();
        b.ru = "Копировать".into();
        b.ru_status = "upstream".into();
        let mut c = row("", "Cut");
        c.uz_latn = "Kesish".into();
        c.uz_latn_status = "draft:agent:2026-10-18".into();
        let cat = cat_of(vec![a, b, c]);
        let chains = Chains::default();
        let cyrl = build_locale(&cat, "wordcraft", "uz-cyrl", &chains, Mode::Release);
        let got: Vec<_> = cyrl.entries.iter().map(|e| (e.source.as_str(), e.value.as_str())).collect();
        assert_eq!(got, vec![("Copy", "Нусхалаш"), ("Paste", "Вставить")]); // own reviewed first, ru fills, draft Latn never leaks
        assert_eq!(cyrl.from.get("ru"), Some(&1));
        let latn = build_locale(&cat, "wordcraft", "uz-latn", &chains, Mode::Release);
        assert!(latn.entries.is_empty(), "release: a draft is not exported and en is never written");
        let latn = build_locale(&cat, "wordcraft", "uz-latn", &chains, Mode::Review);
        assert_eq!(latn.entries.len(), 1);
        assert_eq!(latn.own_draft, 1);
    }

    #[test]
    fn latn_does_not_borrow_ru_unless_configured() {
        let mut a = row("", "Paste");
        a.ru = "Вставить".into();
        a.ru_status = "upstream".into();
        let cat = cat_of(vec![a]);
        assert!(build_locale(&cat, "wordcraft", "uz-latn", &Chains::default(), Mode::Release).entries.is_empty());
        let chains = Chains::parse("[chains]\nuz-latn = [\"uz-latn\", \"ru\", \"en\"]\n").unwrap();
        assert_eq!(build_locale(&cat, "wordcraft", "uz-latn", &chains, Mode::Release).entries.len(), 1);
        assert!(Chains::parse("[chains]\nuz-latn = [\"ru\"]\n").is_err());
    }

    #[test]
    fn ru_only_exported_to_apps_that_have_it_upstream() {
        let mut a = row("", "Paste");
        a.ru = "Вставить".into();
        a.ru_status = "upstream".into();
        a.ru_apps = "wordcraft".into();
        let cat = cat_of(vec![a]);
        assert_eq!(build_locale(&cat, "wordcraft", "ru", &Chains::default(), Mode::Release).entries.len(), 1);
        assert_eq!(build_locale(&cat, "pdfcraft", "ru", &Chains::default(), Mode::Release).entries.len(), 0);
    }

    #[test]
    fn plural_rows_adapt_between_form_counts() {
        let mut p = row("@plural", "{n} page|{n} pages");
        p.ru = "{n} страница|{n} страницы|{n} страниц".into();
        p.ru_status = "upstream".into();
        p.uz_latn = "{n} ta sahifa|{n} ta sahifa".into();
        p.uz_latn_status = "reviewed:owner:2026-10-21".into();
        let mut bad = row("@plural", "{n} field|{n} fields");
        bad.uz_latn = "{n} ta maydon".into(); // one form only: must not be exported
        bad.uz_latn_status = "reviewed:owner:2026-10-21".into();
        let cat = cat_of(vec![p, bad]);
        let chains = Chains::default();
        let ru = build_locale(&cat, "pdfcraft", "ru", &chains, Mode::Release);
        assert_eq!(ru.entries[0].value.split('|').count(), 3);
        let cyrl = build_locale(&cat, "pdfcraft", "uz-cyrl", &chains, Mode::Release);
        assert_eq!(cyrl.entries[0].value, "{n} страница|{n} страниц"); // 3 forms -> 2 (one, other)
        let latn = build_locale(&cat, "pdfcraft", "uz-latn", &chains, Mode::Release);
        assert_eq!(latn.entries.len(), 1);
        assert_eq!(latn.skipped_plural, 1);
    }

    #[test]
    fn native_formats_and_rust_table_round_trip() {
        let entries = vec![
            Entry { context: String::new(), source: "Top\nAlign".into(), value: "Yuqori\nsafga".into() },
            Entry { context: "Page Layout".into(), source: "Orientation".into(), value: "Y\"o\u{2018}nalish\\".into() },
        ];
        let tsv = render("wordcraft", "uz-latn", &entries, Mode::Review);
        let (back, bad) = read_upstream_tsv(&tsv);
        assert_eq!(bad, 0);
        assert_eq!(back.len(), 2);
        assert_eq!(back[0], (String::new(), "Top\nAlign".to_string(), "Yuqori\nsafga".to_string()));
        let grid = render("gridcraft", "uz-latn", &entries, Mode::Review);
        assert!(grid.contains("Page Layout|Orientation\t"));
        let rs = rust_match_fn("translate_uz_latn", &entries);
        let (pairs, anomalies) = parse_rust_table(&rs, None);
        assert_eq!(anomalies, 0);
        assert_eq!(pairs[1], ("Page Layout|Orientation".to_string(), "Y\"o\u{2018}nalish\\".to_string()));
    }

    #[test]
    fn dates_need_all_38_rows() {
        let mut rows = Vec::new();
        for (kind, n) in [("date.month", 12), ("date.monthShort", 12), ("date.day", 7), ("date.dayShort", 7)] {
            for i in 1..=n {
                let mut r = row(&format!("{kind}.{i}"), &format!("{kind}{i}"));
                r.apps = "pdfcraft".into();
                r.ru = format!("ru{kind}{i}");
                r.ru_status = "upstream".into();
                rows.push(r);
            }
        }
        let mut cat = Catalog::default();
        cat.scopes.insert("pdfcraft".into(), rows);
        let chains = Chains::default();
        assert!(build_dates(&cat, "uz-latn", &chains, Mode::Release).is_err());
        let ok = build_dates(&cat, "uz-cyrl", &chains, Mode::Release).unwrap();
        assert_eq!(ok.lines().filter(|l| !l.starts_with('#') && !l.is_empty()).count(), 38);
        assert!(ok.contains("date.month.2\t") && ok.find("date.month.2\t") < ok.find("date.month.10\t"));
    }
}
