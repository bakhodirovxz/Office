//! Glossary use index and re-apply (`glossary-scan`, `glossary-apply`).
//!
//! `i18n/glossary-use.tsv` records, for every UI string that contains a glossary term, which glossary row it uses and the exact
//! uz-Latn / uz-Cyrl term that was in force when the string was drafted. When the owner changes a glossary term,
//! `glossary-apply` finds the strings whose recorded term differs from the current one:
//!
//! - a draft string gets the new term in place of the old one (status `draft:glossary-apply:<date>`), and is listed;
//! - a string the owner already reviewed is never edited: it gets a `glossary-changed:` note and is listed for a new look;
//! - a draft in which the old term cannot be found is listed for manual repair.
//!
//! The default is a dry run. Nothing here is silent: every touched row is printed.

use super::catalog::{state, Catalog, Row};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

pub const INDEX_HEADER: &str = "scope\tid\tglossary_id\tlatn_term\tcyrl_term";
pub const INDEX_FILE: &str = "glossary-use.tsv";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Use {
    pub scope: String,
    pub id: String,
    pub glossary_id: String,
    pub latn: String,
    pub cyrl: String,
}

pub fn load_index(dir: &Path) -> Result<Vec<Use>, String> {
    let path = dir.join(INDEX_FILE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    parse_index(&fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?)
}

pub fn parse_index(text: &str) -> Result<Vec<Use>, String> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if n == 0 {
            if line != INDEX_HEADER {
                return Err("glossary-use.tsv: header does not match".into());
            }
            continue;
        }
        if line.is_empty() {
            continue;
        }
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() != 5 {
            return Err(format!("glossary-use.tsv line {}: {} cells, expected 5", n + 1, c.len()));
        }
        out.push(Use { scope: c[0].into(), id: c[1].into(), glossary_id: c[2].into(), latn: c[3].into(), cyrl: c[4].into() });
    }
    Ok(out)
}

pub fn write_index(uses: &[Use]) -> String {
    let mut sorted = uses.to_vec();
    sorted.sort_by(|a, b| (&a.scope, &a.id, &a.glossary_id).cmp(&(&b.scope, &b.id, &b.glossary_id)));
    let mut out = String::from(INDEX_HEADER);
    out.push('\n');
    for u in sorted {
        let _ = writeln!(out, "{}\t{}\t{}\t{}\t{}", u.scope, u.id, u.glossary_id, u.latn, u.cyrl);
    }
    out
}

fn lower(s: &str) -> Vec<char> {
    s.chars().flat_map(char::to_lowercase).collect()
}

/// Positions (char indexes) where `needle` occurs in `hay` (both lower-cased) with a non-letter before it,
/// and, when `whole` is set, also a non-letter after it.
fn find_word_starts(hay: &[char], needle: &[char], whole: bool) -> Vec<usize> {
    let mut out = Vec::new();
    if needle.is_empty() || hay.len() < needle.len() {
        return out;
    }
    for i in 0..=hay.len() - needle.len() {
        if hay[i..i + needle.len()] != *needle {
            continue;
        }
        let before = i == 0 || !hay[i - 1].is_alphanumeric();
        let after = i + needle.len() == hay.len() || !hay[i + needle.len()].is_alphanumeric();
        if before && (after || !whole) {
            out.push(i);
        }
    }
    out
}

fn english_has(en: &str, term: &str) -> bool {
    !find_word_starts(&lower(en), &lower(term), true).is_empty()
}

fn uz_has(uz: &str, term: &str) -> bool {
    !find_word_starts(&lower(uz), &lower(term), false).is_empty()
}

fn drafted(status: &str, value: &str) -> bool {
    !value.is_empty() && matches!(state(status), "draft" | "reviewed")
}

/// Build the index from the current catalogs: a string uses a term when its English contains the term's English
/// (as a whole word) and its Uzbek contains the term's Uzbek at the start of a word (suffixes may follow).
pub fn scan(cat: &Catalog) -> Vec<Use> {
    let mut out = Vec::new();
    let Some(gloss) = cat.scopes.get("glossary") else { return out };
    for (scope, rows) in &cat.scopes {
        if scope == "glossary" {
            continue;
        }
        for r in rows.iter().filter(|r| r.priority != "obsolete") {
            let (lat, cyr) = (drafted(&r.uz_latn_status, &r.uz_latn), drafted(&r.uz_cyrl_status, &r.uz_cyrl));
            if !lat && !cyr {
                continue;
            }
            for g in gloss {
                if g.uz_latn.is_empty() || !english_has(&r.source, &g.source) {
                    continue;
                }
                let l = lat && uz_has(&r.uz_latn, &g.uz_latn);
                let c = cyr && !g.uz_cyrl.is_empty() && uz_has(&r.uz_cyrl, &g.uz_cyrl);
                if l || c {
                    out.push(Use { scope: scope.clone(), id: r.id.clone(), glossary_id: g.id.clone(), latn: g.uz_latn.clone(), cyrl: g.uz_cyrl.clone() });
                }
            }
        }
    }
    out
}

/// English glossary terms used by each `(scope, id)`, for the review export.
pub fn terms_by_row(cat: &Catalog, uses: &[Use]) -> BTreeMap<(String, String), Vec<String>> {
    let names: BTreeMap<&str, String> = cat
        .scopes
        .get("glossary")
        .into_iter()
        .flatten()
        .map(|g| (g.id.as_str(), if g.context.is_empty() { g.source.clone() } else { format!("{} ({})", g.source, g.context) }))
        .collect();
    let mut map: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for u in uses {
        if let Some(n) = names.get(u.glossary_id.as_str()) {
            let list = map.entry((u.scope.clone(), u.id.clone())).or_default();
            if !list.contains(n) {
                list.push(n.clone());
            }
        }
    }
    map
}

/// Replace every word-start occurrence of `old` in `text` by `new`, keeping an initial capital. None when `old` is absent.
pub fn replace_term(text: &str, old: &str, new: &str) -> Option<String> {
    let hay: Vec<char> = text.chars().collect();
    let low = lower(text);
    if low.len() != hay.len() {
        return None; // lower-casing changed the length: leave it to a human
    }
    let needle = lower(old);
    let hits = find_word_starts(&low, &needle, false);
    if hits.is_empty() {
        return None;
    }
    let mut out = String::new();
    let mut i = 0;
    let mut next = hits.iter().peekable();
    while i < hay.len() {
        if next.peek().is_some_and(|h| **h == i) {
            next.next();
            let cap = hay[i].is_uppercase();
            let mut word = new.to_string();
            if cap {
                let mut cs = word.chars();
                if let Some(f) = cs.next() {
                    word = f.to_uppercase().chain(cs).collect();
                }
            } else {
                let mut cs = word.chars();
                if let Some(f) = cs.next() {
                    word = f.to_lowercase().chain(cs).collect();
                }
            }
            out.push_str(&word);
            i += needle.len();
        } else {
            out.push(hay[i]);
            i += 1;
        }
    }
    Some(out)
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Applied {
    pub changed: Vec<String>,
    pub reviewed_flagged: Vec<String>,
    pub not_found: Vec<String>,
    pub orphans: Vec<String>,
}

impl Applied {
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.reviewed_flagged.is_empty() && self.not_found.is_empty() && self.orphans.is_empty()
    }
    pub fn text(&self, write: bool) -> String {
        let mut out = format!(
            "glossary-apply ({}): {} string(s) re-applied, {} reviewed string(s) flagged, {} to repair by hand, {} orphan use(s)\n",
            if write { "written" } else { "dry run, nothing written; add --write" },
            self.changed.len(),
            self.reviewed_flagged.len(),
            self.not_found.len(),
            self.orphans.len()
        );
        for (title, list) in [("re-applied", &self.changed), ("reviewed, needs a new look", &self.reviewed_flagged), ("old term not found, repair by hand", &self.not_found), ("orphan", &self.orphans)] {
            for l in list {
                let _ = writeln!(out, "  {title}: {l}");
            }
        }
        out
    }
}

fn find_row_mut<'a>(cat: &'a mut Catalog, scope: &str, id: &str) -> Option<&'a mut Row> {
    cat.scopes.get_mut(scope).and_then(|rows| rows.iter_mut().find(|r| r.id == id))
}

/// Re-apply changed glossary terms. Mutates `cat` and `uses` (the recorded terms follow the glossary); the caller saves.
pub fn apply(cat: &mut Catalog, uses: &mut [Use], date: &str) -> Applied {
    let mut rep = Applied::default();
    let gloss: BTreeMap<String, (String, String, String)> = cat
        .scopes
        .get("glossary")
        .into_iter()
        .flatten()
        .map(|g| (g.id.clone(), (g.source.clone(), g.uz_latn.clone(), g.uz_cyrl.clone())))
        .collect();
    for u in uses.iter_mut() {
        let Some((en, new_latn, new_cyrl)) = gloss.get(&u.glossary_id).cloned() else {
            rep.orphans.push(format!("{}/{}: glossary row {} no longer exists", u.scope, u.id, u.glossary_id));
            continue;
        };
        let label = format!("{}/{} [{en}]", u.scope, u.id);
        for (loc, old, new) in [("uz-Latn", u.latn.clone(), new_latn.clone()), ("uz-Cyrl", u.cyrl.clone(), new_cyrl.clone())] {
            if old == new || new.is_empty() {
                continue;
            }
            let Some(row) = find_row_mut(cat, &u.scope, &u.id) else {
                rep.orphans.push(format!("{label}: row no longer exists"));
                continue;
            };
            let (value, status) = (row.value(loc_key(loc)).to_string(), row.status(loc_key(loc)).to_string());
            if status.starts_with("reviewed") {
                let flag = format!("glossary-changed: {en} {old:?} -> {new:?}");
                if !row.note.contains(&flag) {
                    if !row.note.is_empty() {
                        row.note.push_str(" | ");
                    }
                    row.note.push_str(&flag);
                }
                rep.reviewed_flagged.push(format!("{label} {loc}: {value:?} uses the old term {old:?}, new term {new:?}"));
            } else if let Some(text) = replace_term(&value, &old, &new) {
                row.set(loc_key(loc), text.clone(), format!("draft:glossary-apply:{date}"));
                rep.changed.push(format!("{label} {loc}: {value:?} -> {text:?}"));
            } else {
                rep.not_found.push(format!("{label} {loc}: {old:?} not found in {value:?} (new term {new:?})"));
            }
        }
        u.latn = new_latn;
        u.cyrl = new_cyrl;
    }
    rep
}

fn loc_key(loc: &str) -> &str {
    if loc == "uz-Latn" {
        "uz-latn"
    } else {
        "uz-cyrl"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::catalog::row_id;

    fn draft(scope: &str, en: &str, latn: &str, cyrl: &str, status: &str) -> Row {
        Row {
            id: row_id(scope, "", en),
            source: en.into(),
            uz_latn: latn.into(),
            uz_latn_status: status.into(),
            uz_cyrl: cyrl.into(),
            uz_cyrl_status: status.into(),
            priority: "P1".into(),
            ..Row::default()
        }
    }

    const D: &str = "draft:agent:2026-10-17";

    fn cat() -> Catalog {
        let mut c = Catalog::default();
        c.scopes.insert("glossary".into(), vec![draft("glossary", "Save", "Saqlash", "Сақлаш", D), draft("glossary", "Table", "Jadval", "Жадвал", D)]);
        c.scopes.insert(
            "shared".into(),
            vec![
                draft("shared", "Save", "Saqlash", "Сақлаш", D),
                draft("shared", "Save Table", "Jadvalni saqlash", "Жадвални сақлаш", D),
                draft("shared", "Save As", "Boshqa nom bilan saqlash", "Бошқа ном билан сақлаш", "reviewed:owner:2026-10-19"),
                draft("shared", "Saved", "Saqlandi", "Сақланди", D),
            ],
        );
        c
    }

    #[test]
    fn scan_finds_term_uses_with_suffixes_and_whole_english_words() {
        let c = cat();
        let uses = scan(&c);
        let by_row = terms_by_row(&c, &uses);
        let name = |en: &str| by_row.get(&("shared".to_string(), row_id("shared", "", en))).cloned().unwrap_or_default();
        assert_eq!(name("Save"), vec!["Save"]);
        assert_eq!(name("Save Table"), vec!["Save", "Table"]);
        assert_eq!(name("Save As"), vec!["Save"]);
        assert!(name("Saved").is_empty(), "'Saved' is not the word 'Save'");
    }

    #[test]
    fn index_round_trip() {
        let uses = scan(&cat());
        assert_eq!(parse_index(&write_index(&uses)).unwrap().len(), uses.len());
        assert!(parse_index("bad header\n").is_err());
    }

    #[test]
    fn replace_keeps_capitals_and_suffixes() {
        assert_eq!(replace_term("Jadvalni saqlash", "Saqlash", "Yozish").as_deref(), Some("Jadvalni yozish"));
        assert_eq!(replace_term("Saqlashni boshlash", "saqlash", "yozish").as_deref(), Some("Yozishni boshlash"));
        assert_eq!(replace_term("Fayl", "Saqlash", "Yozish"), None);
        assert_eq!(replace_term("Жадвални сақлаш", "Сақлаш", "Ёзиш").as_deref(), Some("Жадвални ёзиш"));
    }

    #[test]
    fn changed_term_is_reapplied_to_drafts_and_flagged_on_reviewed_rows() {
        let mut c = cat();
        let mut uses = scan(&c);
        // the owner changes the glossary term
        let g = c.scopes.get_mut("glossary").unwrap().iter_mut().find(|g| g.source == "Save").unwrap();
        g.uz_latn = "Yozish".into();
        g.uz_cyrl = "Ёзиш".into();
        let rep = apply(&mut c, &mut uses, "2026-10-20");
        let s = &c.scopes["shared"];
        let get = |en: &str| s.iter().find(|r| r.source == en).unwrap().clone();
        assert_eq!(get("Save").uz_latn, "Yozish");
        assert_eq!(get("Save").uz_latn_status, "draft:glossary-apply:2026-10-20");
        assert_eq!(get("Save Table").uz_latn, "Jadvalni yozish");
        assert_eq!(get("Save Table").uz_cyrl, "Жадвални ёзиш");
        // reviewed row: text untouched, flag in the note, listed
        let r = get("Save As");
        assert_eq!(r.uz_latn, "Boshqa nom bilan saqlash");
        assert!(r.note.contains("glossary-changed: Save"));
        assert_eq!(rep.changed.len(), 4);
        assert_eq!(rep.reviewed_flagged.len(), 2);
        assert!(rep.not_found.is_empty());
        // the recorded terms followed the glossary: a second run does nothing
        let again = apply(&mut c, &mut uses, "2026-10-21");
        assert!(again.is_empty(), "{again:?}");
    }

    #[test]
    fn hand_edited_draft_without_the_old_term_is_listed_not_guessed() {
        let mut c = cat();
        let mut uses = scan(&c);
        c.scopes.get_mut("shared").unwrap().iter_mut().find(|r| r.source == "Save").unwrap().uz_latn = "Eslab qolish".into();
        c.scopes.get_mut("glossary").unwrap().iter_mut().find(|g| g.source == "Save").unwrap().uz_latn = "Yozish".into();
        let rep = apply(&mut c, &mut uses, "2026-10-20");
        assert_eq!(rep.not_found.len(), 1);
        assert_eq!(c.scopes["shared"].iter().find(|r| r.source == "Save").unwrap().uz_latn, "Eslab qolish");
    }

    #[test]
    fn dry_run_leaves_the_catalog_alone() {
        let mut c = cat();
        let mut uses = scan(&c);
        c.scopes.get_mut("glossary").unwrap().iter_mut().find(|g| g.source == "Table").unwrap().uz_latn = "Chizma".into();
        // the caller decides about saving; apply on a clone shows what would change
        let mut probe = Catalog { scopes: c.scopes.clone() };
        let mut probe_uses = uses.clone();
        let rep = apply(&mut probe, &mut probe_uses, "2026-10-20");
        assert_eq!(rep.changed.len(), 1);
        assert_eq!(c.scopes["shared"].iter().find(|r| r.source == "Save Table").unwrap().uz_latn, "Jadvalni saqlash");
        let _ = &mut uses;
    }
}
