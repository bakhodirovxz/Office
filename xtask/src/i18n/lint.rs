//! `cargo xt i18n lint`: checks every Uzbek draft against its English source and against its sibling script.
//!
//! Errors (the command fails): placeholder/format parity, apostrophe code points, script purity, upstream product tokens,
//! plural form count, statuses, and P1 rows without a draft. Warnings (listed in the report, the command still passes):
//! length over 1.4x the English text in ribbon/menu contexts, and uz-Latn/uz-Cyrl pairs that do not agree under the
//! letter mapping. The lint reports; it never changes a value.

use super::catalog::{state, Catalog, Row};
use super::review::lint_uz_latn;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Upstream product tokens that `rebrand` replaces after lookup (design 4.3): a translation keeps them as often as the English does.
pub const PRODUCT_TOKENS: [&str; 5] = ["WordCraft", "GridCraft", "DeckCraft", "PdfCraft", "ArtCraft"];

/// Source files whose strings are ribbon, menu or backstage labels (length matters there).
const LABEL_FILES: [&str; 8] = ["ribbon.rs", "menus.rs", "backstage.rs", "qat.rs", "keytips.rs", "commands.rs", "i18n.rs", "chrome.rs"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Error,
    Warn,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub level: Level,
    pub code: &'static str,
    pub scope: String,
    pub id: String,
    pub source: String,
    pub detail: String,
}

fn is_cyrillic(c: char) -> bool {
    matches!(c, '\u{0400}'..='\u{04FF}')
}

/// Placeholders and format specifiers: `{...}` groups and `%s`-style specifiers, in order of appearance.
pub fn placeholders(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '{' => {
                if let Some(j) = chars[i..].iter().position(|c| *c == '}') {
                    out.push(chars[i..=i + j].iter().collect());
                    i += j;
                }
            }
            '%' => {
                // %s %d %i %f %x %% %1$s
                let mut j = i + 1;
                while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '$' || chars[j] == '.') {
                    j += 1;
                }
                if j < chars.len() && matches!(chars[j], 's' | 'd' | 'i' | 'f' | 'x' | '%' | 'u') {
                    out.push(chars[i..=j].iter().collect());
                    i = j;
                }
            }
            _ => {}
        }
        i += 1;
    }
    out.sort();
    out
}

/// `&X` menu accelerators: an ampersand directly followed by a letter or digit.
fn accelerators(s: &str) -> usize {
    let chars: Vec<char> = s.chars().collect();
    chars.windows(2).filter(|w| w[0] == '&' && w[1].is_alphanumeric()).count()
}

fn count_char(s: &str, c: char) -> usize {
    s.chars().filter(|x| *x == c).count()
}

fn ends(s: &str) -> (bool, bool, bool, bool) {
    let t = s.trim_end();
    (t.ends_with('…'), t.ends_with(':'), s.starts_with(char::is_whitespace), s.ends_with(char::is_whitespace))
}

/// Parity between the English source and one Uzbek value. Returns the problems found.
pub fn parity(en: &str, uz: &str) -> Vec<String> {
    let mut p = Vec::new();
    if placeholders(en) != placeholders(uz) {
        p.push(format!("placeholders differ: en {:?}, uz {:?}", placeholders(en), placeholders(uz)));
    }
    if accelerators(en) != accelerators(uz) {
        p.push("accelerator (&X) count differs".to_string());
    }
    for (c, name) in [('\n', "line breaks"), ('\t', "tabs")] {
        if count_char(en, c) != count_char(uz, c) {
            p.push(format!("{name} differ: en {}, uz {}", count_char(en, c), count_char(uz, c)));
        }
    }
    let (e, u) = (ends(en), ends(uz));
    if e.0 != u.0 {
        p.push("trailing ellipsis (…) differs".to_string());
    }
    if e.1 != u.1 {
        p.push("trailing colon differs".to_string());
    }
    if (e.2, e.3) != (u.2, u.3) {
        p.push("leading/trailing whitespace differs".to_string());
    }
    if uz.contains("...") && !en.contains("...") {
        p.push("three ASCII dots: use the single ellipsis character".to_string());
    }
    if uz.contains('\u{fffd}') {
        p.push("replacement character (broken encoding)".to_string());
    }
    p
}

/// Apostrophe rules for both scripts (design 6.2).
pub fn apostrophes(latn: bool, uz: &str) -> Vec<String> {
    let mut p = Vec::new();
    let chars: Vec<char> = uz.chars().collect();
    if latn {
        if let Some(m) = lint_uz_latn(uz) {
            p.push(m);
        }
        for (i, c) in chars.iter().enumerate() {
            let prev = i.checked_sub(1).map(|j| chars[j]);
            if *c == '\u{02bb}' && !matches!(prev, Some('o' | 'O' | 'g' | 'G')) {
                p.push(format!("U+02BB at {i} must follow o or g"));
            }
            if *c == '\u{02bc}' && !prev.is_some_and(char::is_alphabetic) {
                p.push(format!("U+02BC at {i} must follow a letter"));
            }
        }
    } else {
        for (i, c) in chars.iter().enumerate() {
            if matches!(c, '\u{02bb}' | '\u{02bc}' | '\'' | '\u{2018}' | '\u{2019}' | '`') {
                let near = |j: Option<usize>| j.and_then(|j| chars.get(j)).is_some_and(|x| x.is_alphabetic());
                if near(i.checked_sub(1)) || near(Some(i + 1)) {
                    p.push(format!("{c:?} at {i} in Cyrillic text: use the letters ў ғ and ъ"));
                }
            }
        }
    }
    p
}

/// Latin words (ASCII letter/digit runs) of a string, outside `{...}` placeholders.
fn latin_words(s: &str) -> Vec<String> {
    let mut words = Vec::new();
    let (mut cur, mut depth, mut after_dot, mut cur_ext) = (String::new(), 0usize, false, false);
    let flush = |cur: &mut String, ext: bool, words: &mut Vec<String>| {
        if !cur.is_empty() && !ext {
            words.push(std::mem::take(cur));
        }
        cur.clear();
    };
    for c in s.chars() {
        if c == '{' {
            depth += 1;
        }
        if depth == 0 && (c.is_ascii_alphanumeric() || c == '_') {
            if cur.is_empty() {
                cur_ext = after_dot; // a run right after a dot is a file extension (.docx): not a word
            }
            cur.push(c);
        } else {
            flush(&mut cur, cur_ext, &mut words);
        }
        after_dot = c == '.';
        if c == '}' {
            depth = depth.saturating_sub(1);
        }
    }
    flush(&mut cur, cur_ext, &mut words);
    words.retain(|w| w.chars().any(|c| c.is_ascii_alphabetic()));
    words
}

fn is_kept_token(w: &str) -> bool {
    w.chars().filter(|c| c.is_ascii_alphabetic()).count() >= 1
        && (w.chars().filter(|c| c.is_ascii_alphabetic()).all(|c| c.is_ascii_uppercase()) // PDF, OK, SHA
            || w.chars().zip(w.chars().skip(1)).any(|(a, b)| a.is_ascii_lowercase() && b.is_ascii_uppercase())) // CamelCase
}

/// Script purity: no Cyrillic in a Latin cell; no foreign Latin words in a Cyrillic cell.
pub fn script_purity(latn: bool, en: &str, uz: &str) -> Vec<String> {
    let mut p = Vec::new();
    if latn {
        if let Some(c) = uz.chars().find(|c| is_cyrillic(*c)) {
            p.push(format!("Cyrillic letter {c:?} in the Latin text"));
        }
    } else {
        let en_words = latin_words(en);
        for w in latin_words(uz) {
            if !en_words.contains(&w) && !is_kept_token(&w) {
                p.push(format!("Latin word {w:?} in the Cyrillic text (not in the English source)"));
            }
        }
    }
    p
}

/// Whole-stem exceptions shared with `i18n/tools/p1_drafts.py`: Uzbek Cyrillic spells these differently from the letter mapping.
const STEM_PAIRS: [(&str, &str); 2] = [("obyekt", "ob\u{02bc}ekt"), ("funksiya", "funktsiya")];

/// Letters of a Cyrillic string mapped to the Latin letters of the official alphabet; other characters unchanged.
fn cyrl_to_latn_skeleton(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        let m = match c {
            'а' => "a", 'б' => "b", 'в' => "v", 'г' => "g", 'д' => "d", 'е' | 'э' => "e", 'ё' => "yo", 'ж' => "j", 'з' => "z",
            'и' | 'ы' => "i", 'й' => "y", 'к' => "k", 'л' => "l", 'м' => "m", 'н' => "n", 'о' => "o", 'п' => "p", 'р' => "r",
            'с' => "s", 'т' => "t", 'у' => "u", 'ф' => "f", 'х' => "x", 'ц' => "ts", 'ч' => "ch", 'ш' => "sh", 'щ' => "shch",
            'ъ' => "\u{02bc}", 'ь' => "", 'ю' => "yu", 'я' => "ya", 'ў' => "o\u{02bb}", 'қ' => "q", 'ғ' => "g\u{02bb}", 'ҳ' => "h",
            c => {
                out.push(c);
                continue;
            }
        };
        out.push_str(m);
    }
    out
}

/// Both scripts reduced to one comparable form: lower case, stem exceptions, `ye` = `e`, and `ts` = `s` (Russian loans differ here).
fn skeleton(s: &str, from_cyrl: bool) -> String {
    let mut t = if from_cyrl { cyrl_to_latn_skeleton(s) } else { s.to_lowercase() };
    if !from_cyrl {
        for (lat, cyr) in STEM_PAIRS {
            t = t.replace(lat, cyr);
        }
    }
    t = t.replace("ye", "e").replace("ts", "s");
    // drop everything that is not a letter, so spacing and punctuation never decide
    t.chars().filter(|c| c.is_alphanumeric() || *c == '\u{02bb}' || *c == '\u{02bc}').collect()
}

/// Do the Latin and the Cyrillic text say the same under the letter mapping?
pub fn pair_consistent(latn: &str, cyrl: &str) -> bool {
    // placeholders and kept Latin tokens are the same on both sides; the skeletons compare them as plain letters
    let strip = |s: &str| {
        let mut out = String::new();
        let mut depth = 0;
        for c in s.chars() {
            match c {
                '{' => depth += 1,
                '}' if depth > 0 => depth -= 1,
                _ if depth == 0 => out.push(c),
                _ => {}
            }
        }
        out
    };
    skeleton(&strip(latn), false) == skeleton(&strip(cyrl), true)
}

fn label_context(row: &Row) -> bool {
    let file = row.where_.split(':').next().unwrap_or("").rsplit('/').next().unwrap_or("");
    LABEL_FILES.contains(&file)
}

fn visible_len(s: &str) -> usize {
    s.chars().filter(|c| *c != '\n' && *c != '\t').count()
}

/// True when the row is exempt from the P1 completeness check (a `no-uz:` note says why).
pub fn exempt(row: &Row) -> bool {
    row.note.starts_with("no-uz:")
}

fn has_draft(status: &str, value: &str) -> bool {
    !value.is_empty() && matches!(state(status), "draft" | "reviewed")
}

pub fn lint(cat: &Catalog) -> Vec<Finding> {
    let mut f = Vec::new();
    for (scope, rows) in &cat.scopes {
        for r in rows {
            if r.priority == "obsolete" {
                continue;
            }
            let mut add = |level: Level, code: &'static str, detail: String| {
                f.push(Finding { level, code, scope: scope.clone(), id: r.id.clone(), source: r.source.clone(), detail });
            };
            let p1 = r.priority.starts_with("P1");
            let (lat, cyr) = (has_draft(&r.uz_latn_status, &r.uz_latn), has_draft(&r.uz_cyrl_status, &r.uz_cyrl));
            if p1 && !exempt(r) && !(lat && cyr) {
                add(Level::Error, "missing-p1", format!("P1 row without a {} draft", if !lat && !cyr { "uz-Latn or uz-Cyrl" } else if !lat { "uz-Latn" } else { "uz-Cyrl" }));
            }
            for (loc, value, status, is_latn) in [("uz-Latn", &r.uz_latn, &r.uz_latn_status, true), ("uz-Cyrl", &r.uz_cyrl, &r.uz_cyrl_status, false)] {
                if value.is_empty() {
                    continue;
                }
                if !matches!(state(status), "draft" | "reviewed" | "rejected") {
                    add(Level::Error, "status", format!("{loc}: value present but status is {status:?}"));
                }
                let mut errs: Vec<(&'static str, String)> = Vec::new();
                errs.extend(parity(&r.source, value).into_iter().map(|m| ("parity", m)));
                errs.extend(apostrophes(is_latn, value).into_iter().map(|m| ("apostrophe", m)));
                errs.extend(script_purity(is_latn, &r.source, value).into_iter().map(|m| ("script", m)));
                for tok in PRODUCT_TOKENS {
                    if r.source.matches(tok).count() != value.matches(tok).count() {
                        errs.push(("product-token", format!("{tok} appears {} times in English, {} in the translation", r.source.matches(tok).count(), value.matches(tok).count())));
                    }
                }
                if r.context == "@plural" && (value.split('|').count() != 2) {
                    errs.push(("plural", format!("{} plural forms, Uzbek needs 2 (one|other)", value.split('|').count())));
                }
                for (code, m) in errs {
                    add(Level::Error, code, format!("{loc}: {m}"));
                }
                let (el, ul) = (visible_len(&r.source), visible_len(value));
                if is_latn && label_context(r) && el > 0 && (ul as f64) > 1.4 * (el as f64) {
                    add(Level::Warn, "length", format!("{loc}: {ul} characters against {el} in English ({:.1}x)", ul as f64 / el as f64));
                }
            }
            if lat && cyr && !pair_consistent(&r.uz_latn, &r.uz_cyrl) {
                add(Level::Warn, "script-mismatch", format!("Latin {:?} and Cyrillic {:?} disagree under the letter mapping", r.uz_latn, r.uz_cyrl));
            }
        }
    }
    f
}

/// Summary line per code and level.
pub fn summary(findings: &[Finding]) -> String {
    let mut by: BTreeMap<(u8, &str), usize> = BTreeMap::new();
    for x in findings {
        *by.entry((matches!(x.level, Level::Warn) as u8, x.code)).or_default() += 1;
    }
    let mut out = String::new();
    for ((w, code), n) in by {
        let _ = writeln!(out, "  {} {code}: {n}", if w == 1 { "warning" } else { "ERROR  " });
    }
    out
}

/// The markdown report: every finding, errors first.
pub fn report(findings: &[Finding], rows_checked: usize) -> String {
    let mut out = String::from("# i18n lint report (WP9e)\n\nGenerated by `cargo xt i18n lint --report`. Errors fail the command; warnings are flags for the owner's review.\n\n");
    let _ = writeln!(out, "Rows with an Uzbek draft checked: {rows_checked}.\n\n```\n{}```\n", summary(findings));
    for (title, level) in [("Errors", Level::Error), ("Warnings", Level::Warn)] {
        let list: Vec<&Finding> = findings.iter().filter(|x| x.level == level).collect();
        let _ = writeln!(out, "## {title} ({})\n", list.len());
        if list.is_empty() {
            out.push_str("None.\n\n");
            continue;
        }
        out.push_str("| code | scope | id | English | detail |\n|---|---|---|---|---|\n");
        let esc = |s: &str| s.replace('|', "\\|").replace('\n', " ");
        let mut list = list;
        list.sort_by(|a, b| a.code.cmp(b.code).then(a.scope.cmp(&b.scope)).then(a.id.cmp(&b.id)));
        for x in list {
            let _ = writeln!(out, "| {} | {} | {} | {} | {} |", x.code, x.scope, x.id, esc(&x.source), esc(&x.detail));
        }
        out.push('\n');
    }
    out
}

/// Rows that carry at least one Uzbek draft (the number the report quotes).
pub fn rows_with_uzbek(cat: &Catalog) -> usize {
    cat.scopes.values().flatten().filter(|r| r.priority != "obsolete" && (!r.uz_latn.is_empty() || !r.uz_cyrl.is_empty())).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(en: &str, latn: &str, cyrl: &str) -> Row {
        Row {
            id: "x".into(),
            source: en.into(),
            uz_latn: latn.into(),
            uz_latn_status: "draft:agent:2026-10-17".into(),
            uz_cyrl: cyrl.into(),
            uz_cyrl_status: "draft:agent:2026-10-17".into(),
            priority: "P1".into(),
            ..Row::default()
        }
    }

    fn cat_of(rows: Vec<Row>) -> Catalog {
        let mut c = Catalog::default();
        c.scopes.insert("shared".into(), rows);
        c
    }

    fn codes(c: &Catalog) -> Vec<&'static str> {
        lint(c).into_iter().filter(|f| f.level == Level::Error).map(|f| f.code).collect()
    }

    #[test]
    fn a_clean_pair_passes() {
        let c = cat_of(vec![row("Page {page} of {pages}", "{pages} sahifadan {page}-sahifa", "{pages} саҳифадан {page}-саҳифа")]);
        assert!(lint(&c).is_empty(), "{:?}", lint(&c));
    }

    #[test]
    fn placeholder_ellipsis_colon_and_newline_parity() {
        assert_eq!(parity("Open…", "Ochish…"), Vec::<String>::new());
        assert!(!parity("Open…", "Ochish").is_empty());
        assert!(!parity("Font:", "Shrift").is_empty());
        assert!(!parity("{n} files", "{m} fayl").is_empty());
        assert!(!parity("Text\nBox", "Matn qutisi").is_empty());
        assert!(!parity("%s of %d", "%s dan").is_empty());
        assert!(parity("Rows & Columns", "Qatorlar va ustunlar").is_empty());
        assert!(!parity("&File", "Fayl").is_empty());
        assert!(!parity("Wait", "Kuting...").is_empty());
        assert!(!parity("Location: ", "Joylashuv:").is_empty());
    }

    #[test]
    fn apostrophe_code_points() {
        assert!(apostrophes(true, "O\u{02bb}zbek ma\u{02bc}lumot").is_empty());
        assert!(!apostrophes(true, "O'zbek").is_empty());
        assert!(!apostrophes(true, "ko\u{2018}rish").is_empty());
        assert!(!apostrophes(true, "ba\u{02bb}za").is_empty(), "U+02BB only after o or g");
        assert!(apostrophes(false, "Ўзбек маълумот").is_empty());
        assert!(!apostrophes(false, "ма'лумот").is_empty());
        let c = cat_of(vec![row("Go", "Bor'ish", "Бориш")]);
        assert!(codes(&c).contains(&"apostrophe"));
    }

    #[test]
    fn script_purity_rules() {
        assert!(script_purity(true, "Open", "Ochиsh").iter().any(|m| m.contains("Cyrillic")));
        assert!(script_purity(false, "Export PDF", "PDF экспорт").is_empty(), "all-caps token stays Latin");
        assert!(script_purity(false, "Open", "Ochish").iter().any(|m| m.contains("Latin")));
        assert!(script_purity(false, "Insert Word", "Word киритиш").is_empty(), "word taken from the English source");
        assert!(!script_purity(false, "Insert", "Kiritиш").is_empty());
    }

    #[test]
    fn letter_mapping_consistency() {
        assert!(pair_consistent("Oʻzbek tili", "Ўзбек тили"));
        assert!(pair_consistent("Eksport qilish", "Экспорт қилиш"));
        assert!(pair_consistent("Obyektlarni tanlash", "Объектларни танлаш"));
        assert!(pair_consistent("Funksiya", "Функция"));
        assert!(pair_consistent("Maʼlumot {n}", "Маълумот {n}"));
        assert!(pair_consistent("Yoʻq", "Йўқ"));
        assert!(!pair_consistent("Fayl", "Файллар"));
        let c = cat_of(vec![row("File", "Fayl", "Файллар")]);
        assert!(lint(&c).iter().any(|f| f.code == "script-mismatch" && f.level == Level::Warn));
    }

    #[test]
    fn product_tokens_and_plural_forms() {
        let c = cat_of(vec![row("About PdfCraft", "Dastur haqida", "Дастур ҳақида")]);
        assert!(codes(&c).contains(&"product-token"));
        let ok = cat_of(vec![row("About PdfCraft", "PdfCraft haqida", "PdfCraft ҳақида")]);
        assert!(lint(&ok).is_empty(), "{:?}", lint(&ok));
        let mut r = row("{n} page|{n} pages", "{n} ta sahifa", "{n} та саҳифа");
        r.context = "@plural".into();
        assert!(codes(&cat_of(vec![r.clone()])).contains(&"plural"));
        r.uz_latn = "{n} ta sahifa|{n} ta sahifa".into();
        r.uz_cyrl = "{n} та саҳифа|{n} та саҳифа".into();
        assert!(lint(&cat_of(vec![r])).is_empty());
    }

    #[test]
    fn file_extensions_are_not_foreign_words() {
        assert!(script_purity(false, "Word (.docx)", "Word (.docx)").is_empty());
        assert!(!script_purity(false, "Word", "Word docx").is_empty());
    }

    #[test]
    fn length_is_a_warning_in_label_contexts_only() {
        let mut r = row("Cut", "Kesib olish", "Кесиб олиш");
        assert!(lint(&cat_of(vec![r.clone()])).is_empty(), "no source file known: not a label context");
        r.where_ = "crates/ui-egui/src/ribbon.rs:10".into();
        let f = lint(&cat_of(vec![r]));
        assert_eq!(f.len(), 1);
        assert_eq!((f[0].code, f[0].level), ("length", Level::Warn));
    }

    #[test]
    fn missing_p1_is_an_error_unless_exempt() {
        let mut r = row("Open", "", "");
        r.uz_latn_status = "todo".into();
        r.uz_cyrl_status = "todo".into();
        assert!(codes(&cat_of(vec![r.clone()])).contains(&"missing-p1"));
        r.note = "no-uz: part of an ArtCraft sentence".into();
        assert!(codes(&cat_of(vec![r.clone()])).is_empty());
        r.note.clear();
        r.priority = "P2".into();
        assert!(codes(&cat_of(vec![r])).is_empty());
    }

    #[test]
    fn report_lists_errors_before_warnings() {
        let c = cat_of(vec![row("Open…", "Ochish", "Очиш")]);
        let f = lint(&c);
        let text = report(&f, rows_with_uzbek(&c));
        assert!(text.find("## Errors").unwrap() < text.find("## Warnings").unwrap());
        assert!(text.contains("parity"));
    }
}
