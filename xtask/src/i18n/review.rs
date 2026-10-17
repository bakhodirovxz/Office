//! Review pipeline: `review-export` (CSV with BOM for Excel), `review-import`, `coverage`.
//! No tool in here ever writes or alters Uzbek text on its own: import only records the owner's verdict
//! and the owner's own text.

use super::catalog::{APPS, Catalog, Row, row_id, sha1, state};
use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;

pub const COLUMNS: [&str; 13] = [
    "key",
    "context",
    "en",
    "ru",
    "uz-Latn draft",
    "uz-Cyrl draft",
    "status",
    "owner comment",
    "scope",
    "where",
    "pass",
    "en_hash",
    "glossary terms",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pass {
    Both,
    Latn,
    Cyrl,
}

impl Pass {
    pub fn parse(s: &str) -> Result<Pass, String> {
        match s {
            "both" => Ok(Pass::Both),
            "latn" => Ok(Pass::Latn),
            "cyrl" => Ok(Pass::Cyrl),
            o => Err(format!("unknown pass {o:?}; use both, latn or cyrl")),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Pass::Both => "both",
            Pass::Latn => "latn",
            Pass::Cyrl => "cyrl",
        }
    }
    fn locales(self) -> &'static [&'static str] {
        match self {
            Pass::Both => &["uz-latn", "uz-cyrl"],
            Pass::Latn => &["uz-latn"],
            Pass::Cyrl => &["uz-cyrl"],
        }
    }
}

pub fn en_hash(source: &str) -> String {
    sha1(source.as_bytes())
        .iter()
        .take(4)
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn csv_cell(s: &str, delim: char) -> String {
    if s.contains(delim) || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn write_csv(records: &[Vec<String>]) -> String {
    let mut out = String::from("\u{feff}");
    for r in records {
        out.push_str(
            &r.iter()
                .map(|c| csv_cell(c, ','))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push_str("\r\n");
    }
    out
}

/// Parse CSV text (BOM tolerated). The delimiter is chosen from the header line: Excel in some locales saves `;`.
pub fn read_csv(text: &str) -> Vec<Vec<String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let first = text.lines().next().unwrap_or("");
    let delim = [',', ';', '\t']
        .into_iter()
        .max_by_key(|d| first.matches(*d).count())
        .unwrap_or(',');
    let mut rows = Vec::new();
    let (mut row, mut cell) = (Vec::new(), String::new());
    let (mut quoted, mut chars) = (false, text.chars().peekable());
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cell.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                cell.push(c);
            }
        } else if c == '"' && cell.is_empty() {
            quoted = true;
        } else if c == delim {
            row.push(std::mem::take(&mut cell));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut cell));
            if !(row.len() == 1 && row[0].is_empty()) {
                rows.push(std::mem::take(&mut row));
            } else {
                row.clear();
            }
        } else {
            cell.push(c);
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows
}

fn display_status(r: &Row, pass: Pass) -> &'static str {
    let ok = |loc: &str| r.value(loc).is_empty() || r.status(loc).starts_with("reviewed:owner");
    if state(&r.uz_latn_status) == "rejected" || state(&r.uz_cyrl_status) == "rejected" {
        "rejected"
    } else if pass.locales().iter().all(|l| ok(l)) {
        "reviewed"
    } else {
        "draft-unreviewed"
    }
}

fn has_uzbek(r: &Row) -> bool {
    matches!(state(&r.uz_latn_status), "draft" | "reviewed" | "rejected") && !r.uz_latn.is_empty()
        || matches!(state(&r.uz_cyrl_status), "draft" | "reviewed" | "rejected")
            && !r.uz_cyrl.is_empty()
}

/// How visible a string is to a user: lower tier first. Menus and ribbon labels come before dialogs, panes, then
/// validation messages, credits and keyboard hints. A string that is exactly a glossary term (File, Save, Open) is tier 0.
fn visibility(
    scope: &str,
    r: &Row,
    glossary_en: &HashSet<String>,
) -> (u8, std::cmp::Reverse<usize>, usize, String) {
    let file = r
        .where_
        .split(':')
        .next()
        .unwrap_or("")
        .rsplit('/')
        .next()
        .unwrap_or("");
    let mut tier: u8 = match file {
        "menus.rs" | "qat.rs" => 0,
        "backstage.rs" | "ribbon.rs" | "lib.rs" | "chrome.rs" | "commands.rs" => 1,
        "i18n.rs" if scope == "gridcraft" => 1,
        "dialogs.rs" | "i18n.rs" => 2,
        "dates.rs" | "bulk_fields.rs" | "equation.rs" | "outline.rs" | "caret.rs"
        | "catalog.rs" | "keytips.rs" => 4,
        "pdf.rs" | "credits.rs" => 5,
        _ => 3,
    };
    if r.context.starts_with("date.") {
        tier = 4;
    }
    if glossary_en.contains(&r.source) {
        tier = 0;
    }
    if r.source.chars().count() > 60 {
        tier = (tier + 2).min(6);
    }
    let apps = r.apps.split(',').filter(|a| !a.is_empty()).count().max(1);
    (
        tier,
        std::cmp::Reverse(apps),
        r.source.chars().count(),
        r.source.to_lowercase(),
    )
}

/// Rows in review order: glossary (batch 0); then every P1 row with a draft, most visible first (see `visibility`);
/// then P2 rows with a draft by scope and English text.
pub fn ordered_rows(cat: &Catalog, batch: usize) -> Vec<(String, &Row)> {
    if batch == 0 {
        let mut v: Vec<_> = cat
            .scopes
            .get("glossary")
            .into_iter()
            .flatten()
            .map(|r| ("glossary".to_string(), r))
            .collect();
        v.sort_by(|a, b| {
            a.1.priority
                .cmp(&b.1.priority)
                .then(a.1.source.to_lowercase().cmp(&b.1.source.to_lowercase()))
        });
        return v;
    }
    let glossary_en: HashSet<String> = cat
        .scopes
        .get("glossary")
        .into_iter()
        .flatten()
        .map(|g| g.source.clone())
        .collect();
    let mut v: Vec<(String, &Row)> = Vec::new();
    let mut p1: Vec<(String, &Row)> = Vec::new();
    for scope in ["shared", "wordcraft", "gridcraft", "pdfcraft", "deckcraft"] {
        p1.extend(
            cat.scopes
                .get(scope)
                .into_iter()
                .flatten()
                .filter(|r| r.priority.starts_with("P1") && has_uzbek(r))
                .map(|r| (scope.to_string(), r)),
        );
    }
    p1.sort_by(|a, b| {
        visibility(&a.0, a.1, &glossary_en)
            .cmp(&visibility(&b.0, b.1, &glossary_en))
            .then(a.0.cmp(&b.0))
            .then(a.1.id.cmp(&b.1.id))
    });
    v.extend(p1);
    for scope in ["shared", "wordcraft", "gridcraft", "pdfcraft", "deckcraft"] {
        let mut rows: Vec<_> = cat
            .scopes
            .get(scope)
            .into_iter()
            .flatten()
            .filter(|r| r.priority.starts_with("P2") && has_uzbek(r))
            .collect();
        rows.sort_by(|a, b| {
            a.source
                .to_lowercase()
                .cmp(&b.source.to_lowercase())
                .then(a.id.cmp(&b.id))
        });
        v.extend(rows.into_iter().map(|r| (scope.to_string(), r)));
    }
    v
}

/// The slice of `total` rows that batch `batch` (1-based) covers. `sizes[k]` is the size of batch k+1; the last size repeats.
pub fn batch_range(batch: usize, sizes: &[usize], total: usize) -> (usize, usize) {
    let size_of = |k: usize| sizes.get(k).or(sizes.last()).copied().unwrap_or(400);
    let start: usize = (0..batch - 1).map(size_of).sum();
    (start.min(total), (start + size_of(batch - 1)).min(total))
}

/// Build the CSV for batch `batch`. Batch 0 is the glossary; batch N (>= 1) is the N-th chunk of `size` rows.
#[cfg(test)]
pub fn review_export(
    cat: &Catalog,
    batch: usize,
    pass: Pass,
    size: usize,
) -> Result<(String, usize), String> {
    review_export_with(cat, batch, pass, &[size], &BTreeMap::new())
}

/// Like `review_export`, with per-batch sizes and the glossary terms each row uses (column "glossary terms").
pub fn review_export_with(
    cat: &Catalog,
    batch: usize,
    pass: Pass,
    sizes: &[usize],
    terms: &BTreeMap<(String, String), Vec<String>>,
) -> Result<(String, usize), String> {
    let all = ordered_rows(cat, batch);
    let chunk: Vec<_> = if batch == 0 {
        all
    } else {
        let (a, b) = batch_range(batch, sizes, all.len());
        all.into_iter().skip(a).take(b - a).collect()
    };
    if chunk.is_empty() {
        return Err(format!(
            "batch {batch} is empty: no rows with an Uzbek draft{}",
            if batch == 0 { " in glossary.tsv" } else { "" }
        ));
    }
    let mut records = vec![COLUMNS.iter().map(|s| s.to_string()).collect::<Vec<_>>()];
    for (scope, r) in chunk {
        if pass == Pass::Cyrl && !r.uz_latn_status.starts_with("reviewed:owner") && batch != 0 {
            continue; // spelling pass only after the meaning pass
        }
        records.push(vec![
            r.id.clone(),
            super::catalog::escape(&r.context),
            super::catalog::escape(&r.source),
            super::catalog::escape(&r.ru),
            super::catalog::escape(&r.uz_latn),
            super::catalog::escape(&r.uz_cyrl),
            display_status(r, pass).to_string(),
            r.note.clone(),
            scope.clone(),
            r.where_.clone(),
            pass.name().to_string(),
            en_hash(&r.source),
            terms
                .get(&(scope, r.id.clone()))
                .map(|t| t.join("; "))
                .unwrap_or_default(),
        ]);
    }
    let n = records.len() - 1;
    Ok((write_csv(&records), n))
}

/// The apostrophe rule for uz-Latn (design 6.2): oʻ gʻ use U+02BB, the tutuq belgisi U+02BC.
pub fn lint_uz_latn(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '`' | '\u{00b4}') {
            let near_letter = |j: Option<usize>| {
                j.and_then(|j| chars.get(j))
                    .is_some_and(|x| x.is_alphabetic())
            };
            if near_letter(i.checked_sub(1)) && near_letter(Some(i + 1))
                || near_letter(i.checked_sub(1)) && matches!(chars.get(i + 1), None | Some(' '))
            {
                return Some(format!(
                    "apostrophe {c:?} at {i}: use U+02BB (o\u{2bb}, g\u{2bb}) or U+02BC (tutuq belgisi)"
                ));
            }
        }
    }
    None
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ImportStats {
    pub reviewed: usize,
    pub rejected: usize,
    pub edited_reviewed: usize,
    pub lint_failed: usize,
    pub unchanged: usize,
    pub stale: Vec<String>,
    pub unknown: Vec<String>,
}

pub fn review_import(cat: &mut Catalog, csv: &str, date: &str) -> Result<ImportStats, String> {
    let records = read_csv(csv);
    let header = records.first().ok_or("empty file")?;
    let col = |name: &str| {
        header
            .iter()
            .position(|h| h.trim().eq_ignore_ascii_case(name))
            .ok_or(format!("missing column {name:?}"))
    };
    let (c_key, c_lat, c_cyr, c_st, c_cm) = (
        col("key")?,
        col("uz-Latn draft")?,
        col("uz-Cyrl draft")?,
        col("status")?,
        col("owner comment")?,
    );
    let (c_scope, c_pass, c_hash) = (col("scope")?, col("pass")?, col("en_hash")?);
    let mut st = ImportStats::default();
    for rec in &records[1..] {
        let get = |i: usize| {
            rec.get(i)
                .map(String::as_str)
                .unwrap_or("")
                .trim_end_matches('\r')
        };
        let (key, scope) = (get(c_key).trim(), get(c_scope).trim());
        if key.is_empty() {
            continue;
        }
        let Some(row) = cat
            .scopes
            .get_mut(scope)
            .and_then(|rows| rows.iter_mut().find(|r| r.id == key))
        else {
            st.unknown.push(format!("{scope}/{key}"));
            continue;
        };
        if en_hash(&row.source) != get(c_hash).trim() {
            st.stale.push(format!("{scope}/{key}"));
            continue;
        }
        let pass = Pass::parse(get(c_pass).trim()).unwrap_or(Pass::Both);
        let verdict = get(c_st).trim().to_lowercase();
        let reviewed = matches!(verdict.as_str(), "reviewed" | "ok" | "approved");
        let rejected = matches!(verdict.as_str(), "rejected" | "reject");
        let comment = get(c_cm).trim();
        if !comment.is_empty() && !row.note.contains(comment) {
            if !row.note.is_empty() {
                row.note.push_str(" | ");
            }
            let _ = write!(row.note, "owner: {comment}");
        }
        let mut touched = false;
        for loc in pass.locales() {
            let cell = super::catalog::unescape(get(if *loc == "uz-latn" { c_lat } else { c_cyr }))
                .map_err(|e| format!("{scope}/{key}: {e}"))?;
            if cell.is_empty() {
                continue;
            }
            let changed = cell != row.value(loc);
            if rejected {
                let v = row.value(loc).to_string();
                row.set(loc, v, format!("rejected:owner:{date}"));
                st.rejected += 1;
                touched = true;
            } else if reviewed || changed {
                let lint = if *loc == "uz-latn" {
                    lint_uz_latn(&cell)
                } else {
                    None
                };
                match lint {
                    Some(msg) => {
                        // Owner text is kept as written; it just does not become trusted.
                        if !row.note.is_empty() {
                            row.note.push_str(" | ");
                        }
                        let _ = write!(row.note, "lint: {msg}");
                        row.set(loc, cell, format!("draft:owner-fix:{date}"));
                        st.lint_failed += 1;
                    }
                    None if reviewed => {
                        row.set(loc, cell, format!("reviewed:owner:{date}"));
                        if changed {
                            st.edited_reviewed += 1
                        } else {
                            st.reviewed += 1
                        }
                    }
                    None => row.set(loc, cell, format!("draft:owner-fix:{date}")),
                }
                touched = true;
            }
        }
        if !touched {
            st.unchanged += 1;
        }
    }
    Ok(st)
}

pub fn log_line(file: &str, st: &ImportStats, date: &str) -> String {
    format!(
        "- {date} `{file}`: reviewed {}, reviewed with owner edit {}, rejected {}, lint-failed (kept as draft) {}, no verdict {}, stale {} ({}), unknown {}\n",
        st.reviewed,
        st.edited_reviewed,
        st.rejected,
        st.lint_failed,
        st.unchanged,
        st.stale.len(),
        st.stale.join(" "),
        st.unknown.len()
    )
}

/// `keys K, P1 K1, todo T, draft D, reviewed R` for each app and Uzbek locale, plus upstream ru coverage.
pub fn coverage(cat: &Catalog) -> String {
    let mut out = String::from(
        "app        keys   P1   ru(upstream) | uz-Latn todo/draft/reviewed (P1 reviewed) | uz-Cyrl todo/draft/reviewed (P1 reviewed)\n",
    );
    for app in APPS {
        let rows = cat.app_rows(app);
        let p1: Vec<&Row> = rows
            .iter()
            .copied()
            .filter(|r| r.is_p1(app) && !r.context.starts_with("date."))
            .collect();
        let ui: Vec<&Row> = rows
            .iter()
            .copied()
            .filter(|r| !r.context.starts_with("date."))
            .collect();
        let ru = ui.iter().filter(|r| r.ru_in_app(app)).count();
        let count = |loc: &str, rows: &[&Row]| {
            let (mut t, mut d, mut r) = (0, 0, 0);
            for row in rows {
                match state(row.status(loc)) {
                    "reviewed" => r += 1,
                    "draft" => d += 1,
                    _ => t += 1,
                }
            }
            (t, d, r)
        };
        let (l, c) = (count("uz-latn", &ui), count("uz-cyrl", &ui));
        let (lp, cp) = (count("uz-latn", &p1).2, count("uz-cyrl", &p1).2);
        let _ = writeln!(
            out,
            "{app:<10} {:>5} {:>4} {:>8}     | {}/{}/{} ({lp}/{}) | {}/{}/{} ({cp}/{})",
            ui.len(),
            p1.len(),
            ru,
            l.0,
            l.1,
            l.2,
            p1.len(),
            c.0,
            c.1,
            c.2,
            p1.len()
        );
    }
    if let Some(g) = cat.scopes.get("glossary") {
        let _ = writeln!(
            out,
            "glossary   {} terms ({} draft)",
            g.len(),
            g.iter()
                .filter(|r| state(&r.uz_latn_status) == "draft")
                .count()
        );
    }
    out
}

pub fn check_ids(cat: &Catalog) -> Vec<String> {
    let mut bad = Vec::new();
    for (scope, rows) in &cat.scopes {
        let sc = if scope == "shared" {
            "shared"
        } else {
            scope.as_str()
        };
        for r in rows {
            if r.id != row_id(sc, &r.context, &r.source) {
                bad.push(format!(
                    "{scope}/{}: id does not match scope+context+source",
                    r.id
                ));
            }
        }
    }
    bad
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::catalog::row_id;

    fn draft_row(src: &str, latn: &str) -> Row {
        Row {
            id: row_id("glossary", "", src),
            source: src.into(),
            uz_latn: latn.into(),
            uz_latn_status: "draft:agent:2026-10-18".into(),
            uz_cyrl: "\u{0424}".into(),
            uz_cyrl_status: "draft:agent:2026-10-18".into(),
            priority: "P1".into(),
            ..Row::default()
        }
    }

    fn cat() -> Catalog {
        let mut c = Catalog::default();
        c.scopes.insert(
            "glossary".into(),
            vec![
                draft_row("File", "Fayl"),
                draft_row("Save, as \"x\"\nnow", "Saqlash"),
            ],
        );
        c
    }

    #[test]
    fn csv_round_trip_with_quotes_newlines_and_bom() {
        let recs = vec![
            vec![
                "a,b".to_string(),
                "q\"uote".into(),
                "line1\nline2".into(),
                "".into(),
            ],
            vec!["x".into(), "y".into(), "z".into(), "w".into()],
        ];
        let text = write_csv(&recs);
        assert!(text.starts_with('\u{feff}'));
        assert_eq!(read_csv(&text), recs);
        // semicolon-delimited (Excel in some locales)
        assert_eq!(
            read_csv("a;b;c\r\n1;\"2;x\";3\r\n"),
            vec![vec!["a", "b", "c"], vec!["1", "2;x", "3"]]
        );
    }

    #[test]
    fn export_has_the_agreed_columns_and_batch_zero_is_the_glossary() {
        let (csv, n) = review_export(&cat(), 0, Pass::Both, 400).unwrap();
        assert_eq!(n, 2);
        let recs = read_csv(&csv);
        assert_eq!(
            &recs[0][..8],
            [
                "key",
                "context",
                "en",
                "ru",
                "uz-Latn draft",
                "uz-Cyrl draft",
                "status",
                "owner comment"
            ]
        );
        assert_eq!(recs[1][6], "draft-unreviewed");
        assert!(review_export(&cat(), 1, Pass::Both, 400).is_err());
    }

    #[test]
    fn import_marks_reviewed_and_keeps_owner_text() {
        let mut c = cat();
        let (csv, _) = review_export(&c, 0, Pass::Both, 400).unwrap();
        let mut recs = read_csv(&csv);
        let fa = recs.iter().position(|r| r[2] == "File").unwrap();
        recs[fa][6] = "reviewed".into();
        let sv = recs
            .iter()
            .position(|r| r[2] != "File" && r[2] != "en")
            .unwrap();
        recs[sv][4] = "Saqla'sh".into(); // owner edit that fails the apostrophe lint
        recs[sv][6] = "reviewed".into();
        recs[sv][7] = "prefer this".into();
        let st = review_import(&mut c, &write_csv(&recs), "2026-10-20").unwrap();
        assert_eq!((st.reviewed, st.lint_failed), (3, 1)); // File: latn + cyrl; the second row: cyrl reviewed, latn lint-failed
        let g = &c.scopes["glossary"];
        let file = g.iter().find(|r| r.source == "File").unwrap();
        assert_eq!(
            (file.uz_latn_status.as_str(), file.uz_cyrl_status.as_str()),
            ("reviewed:owner:2026-10-20", "reviewed:owner:2026-10-20")
        );
        let other = g.iter().find(|r| r.source != "File").unwrap();
        assert_eq!(other.uz_latn, "Saqla'sh"); // never silently fixed
        assert_eq!(other.uz_latn_status, "draft:owner-fix:2026-10-20");
        assert!(other.note.contains("prefer this") && other.note.contains("lint:"));
    }

    #[test]
    fn import_skips_stale_rows_and_empty_verdicts() {
        let mut c = cat();
        let (csv, _) = review_export(&c, 0, Pass::Both, 400).unwrap();
        c.scopes.get_mut("glossary").unwrap()[0].source = "File changed".into(); // English changed after export
        let mut recs = read_csv(&csv);
        for r in recs.iter_mut().skip(1) {
            r[6] = "reviewed".into();
        }
        let st = review_import(&mut c, &write_csv(&recs), "2026-10-20").unwrap();
        assert_eq!(st.stale.len() + st.unknown.len(), 1);
        let mut c2 = cat();
        let st = review_import(&mut c2, &csv, "2026-10-20").unwrap();
        assert_eq!((st.reviewed, st.unchanged), (0, 2));
    }

    #[test]
    fn rejection_is_recorded() {
        let mut c = cat();
        let (csv, _) = review_export(&c, 0, Pass::Latn, 400).unwrap();
        let mut recs = read_csv(&csv);
        recs[1][6] = "rejected".into();
        let st = review_import(&mut c, &write_csv(&recs), "2026-10-20").unwrap();
        assert_eq!(st.rejected, 1);
    }

    #[test]
    fn apostrophe_lint() {
        assert!(lint_uz_latn("O'zbek").is_some());
        assert!(lint_uz_latn("o\u{2018}zbek").is_some());
        assert!(lint_uz_latn("O\u{2bb}zbek, ma\u{2bc}lumot").is_none());
        assert!(lint_uz_latn("\"quoted\" text").is_none());
        assert!(
            lint_uz_latn("'text'").is_some(),
            "a closing apostrophe after a letter is flagged too: the owner decides"
        );
    }

    #[test]
    fn ids_are_verified() {
        assert!(check_ids(&cat()).is_empty());
        let mut c = cat();
        c.scopes.get_mut("glossary").unwrap()[0].source = "X".into();
        assert_eq!(check_ids(&c).len(), 1);
    }
}
