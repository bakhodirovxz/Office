//! `i18n extract`: read each app's catalogs or strings from the committed `upstream/` snapshot
//! (read-only) and merge them into our canonical catalog.

use super::catalog::{APPS, Catalog, Row, row_id};
use super::lex::{Token, lex, strip_tests_and_attrs};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

/// The numbers the design (i18n.md section 1) gives for the old BASE.
pub const DESIGN_COUNTS: [(&str, usize); 4] = [
    ("wordcraft", 2616),
    ("gridcraft", 970),
    ("pdfcraft", 2670),
    ("deckcraft", 1525),
];

#[derive(Debug, Default, Clone)]
pub struct KeyInfo {
    pub ru: Option<String>,
    pub p1: bool,
    pub where_: String,
}

#[derive(Debug, Default)]
pub struct Universe {
    pub app: String,
    /// (context, English) -> info. Date rows (`date.*` contexts) are kept apart in `dates`.
    pub keys: BTreeMap<(String, String), KeyInfo>,
    pub dates: BTreeMap<(String, String), KeyInfo>,
    pub notes: Vec<String>,
    /// wordcraft: the keys of pl.tsv; P1 may only use these (design 2.1, `polish_covers_every_key...`).
    pub pl_keys: BTreeSet<(String, String)>,
}

impl Universe {
    pub fn with_ru(&self) -> usize {
        self.keys.values().filter(|k| k.ru.is_some()).count()
    }
    pub fn p1(&self) -> usize {
        self.keys.values().filter(|k| k.p1).count()
    }
}

pub fn app_dir(root: &Path, app: &str) -> PathBuf {
    root.join("upstream").join(app)
}

pub fn walk_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if p.is_dir() {
            if !matches!(
                name.as_str(),
                "i18n" | "tests" | "target" | "benches" | "examples"
            ) {
                walk_rs(&p, out);
            }
        } else if name.ends_with(".rs")
            && !name.ends_with("tests.rs")
            && !name.ends_with("_tests.rs")
        {
            out.push(p);
        }
    }
}

/// Stripped token streams of every non-test source file under the given roots (path relative to `base`).
fn sources(base: &Path, roots: &[&str]) -> Vec<(String, Vec<Token>)> {
    let mut files = Vec::new();
    for r in roots {
        walk_rs(&base.join(r), &mut files);
    }
    files
        .into_iter()
        .filter_map(|p| {
            let text = fs::read_to_string(&p).ok()?;
            let rel = p
                .strip_prefix(base)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            Some((rel, strip_tests_and_attrs(lex(&text))))
        })
        .collect()
}

/// Upstream TSV lines: `context<TAB>source<TAB>translation`. Comment and blank lines skipped.
/// Returns the entries (first duplicate wins, like upstream) and the number of lines skipped as malformed.
pub fn read_upstream_tsv(text: &str) -> (Vec<(String, String, String)>, usize) {
    let mut out: Vec<(String, String, String)> = Vec::new();
    let mut seen = BTreeSet::new();
    let mut bad = 0;
    for line in text.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        let parsed = (cols.len() == 3)
            .then(|| {
                Some((
                    super::catalog::unescape(cols[0]).ok()?,
                    super::catalog::unescape(cols[1]).ok()?,
                    super::catalog::unescape(cols[2]).ok()?,
                ))
            })
            .flatten();
        match parsed {
            Some((c, s, t)) if !s.is_empty() && !t.is_empty() => {
                if seen.insert((c.clone(), s.clone())) {
                    out.push((c, s, t));
                }
            }
            _ => bad += 1,
        }
    }
    (out, bad)
}

fn tsv_files(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut v: Vec<_> = fs::read_dir(dir)
        .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    v.retain(|p: &PathBuf| p.extension().is_some_and(|e| e == "tsv"));
    v.sort();
    v.into_iter()
        .map(|p| {
            (
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string(),
                p,
            )
        })
        .collect()
}

/// literal -> first `path:line` in the app's non-test sources.
fn where_index(srcs: &[(String, Vec<Token>)]) -> HashMap<String, String> {
    let mut m = HashMap::new();
    for (rel, toks) in srcs {
        for t in toks {
            if let Some(l) = t.lit() {
                m.entry(l.to_string())
                    .or_insert_with(|| format!("{rel}:{}", t.line));
            }
        }
    }
    m
}

fn set_where(u: &mut Universe, idx: &HashMap<String, String>) {
    for ((ctx, src), info) in u.keys.iter_mut() {
        if ctx.starts_with('@') && ctx != "@plural" {
            continue;
        }
        let first = src.split('|').next().unwrap_or(src);
        if let Some(w) = idx.get(src).or_else(|| idx.get(first)) {
            info.where_ = w.clone();
        }
    }
}

fn mark_p1(u: &mut Universe, p1: &BTreeMap<String, String>) {
    for ((ctx, src), info) in u.keys.iter_mut() {
        if let Some(w) = p1.get(src)
            && (ctx.is_empty() || ctx == "template" || ctx == "preview")
        {
            info.p1 = true;
            if info.where_.is_empty() {
                info.where_ = w.clone();
            }
        }
    }
}

fn lits_in_call(toks: &[Token], start: usize) -> Vec<&str> {
    // literals directly at depth 1 of the call whose '(' is at toks[start]
    let mut depth = 0;
    let mut out = Vec::new();
    for t in &toks[start..] {
        if t.is_punct('(') || t.is_punct('[') || t.is_punct('{') {
            depth += 1;
        } else if t.is_punct(')') || t.is_punct(']') || t.is_punct('}') {
            depth -= 1;
            if depth == 0 {
                break;
            }
        } else if depth == 1
            && let Some(l) = t.lit()
        {
            out.push(l);
        }
    }
    out
}

fn tokens_after_ident_until_semicolon<'a>(toks: &'a [Token], name: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        if toks[i].is_ident(name) && toks.get(i + 1).is_some_and(|t| t.is_punct(':')) {
            let mut j = i;
            while j < toks.len() && !toks[j].is_punct('=') {
                j += 1;
            }
            while j < toks.len() && !toks[j].is_punct(';') {
                if let Some(l) = toks[j].lit() {
                    out.push(l);
                }
                j += 1;
            }
            break;
        }
        i += 1;
    }
    out
}

// ---------------------------------------------------------------- wordcraft / pdfcraft (TSV)

fn tsv_universe(root: &Path, app: &str) -> Universe {
    let base = app_dir(root, app);
    let mut u = Universe {
        app: app.into(),
        ..Universe::default()
    };
    let dir = base.join("crates/ui-egui/src/i18n");
    let mut pl_keys = BTreeSet::new();
    let mut bad_total = 0;
    for (lang, path) in tsv_files(&dir) {
        let text = fs::read_to_string(&path).unwrap_or_default();
        let (entries, bad) = read_upstream_tsv(&text);
        bad_total += bad;
        for (c, s, t) in entries {
            if lang == "pl" {
                pl_keys.insert((c.clone(), s.clone()));
            }
            let info = u.keys.entry((c, s)).or_default();
            if lang == "ru" {
                info.ru = Some(t);
            }
        }
    }
    if bad_total > 0 {
        u.notes
            .push(format!("{bad_total} malformed upstream TSV lines skipped"));
    }
    let roots: &[&str] = if app == "pdfcraft" {
        &["crates/ui-egui/src", "crates/engine/src", "crates/sign/src"]
    } else {
        &["crates/ui-egui/src", "crates/engine/src"]
    };
    let srcs = sources(&base, roots);
    let idx = where_index(&srcs);
    set_where(&mut u, &idx);

    let mut p1: BTreeMap<String, String> = BTreeMap::new();
    if app == "wordcraft" {
        for (rel, toks) in &srcs {
            if !rel.starts_with("crates/engine/src") {
                continue;
            }
            for i in 0..toks.len().saturating_sub(5) {
                if toks[i].is_ident("CommandSpec")
                    && toks[i + 1].is_punct(':')
                    && toks[i + 3].is_ident("new")
                    && toks[i + 4].is_punct('(')
                {
                    let l = lits_in_call(toks, i + 4);
                    let at = format!("{rel}:{}", toks[i].line);
                    if let Some(label) = l.get(1) {
                        p1.entry((*label).to_string()).or_insert_with(|| at.clone());
                    }
                    if let Some(loc) = l.get(2) {
                        for seg in loc.split(" \u{203a} ") {
                            p1.entry(seg.to_string()).or_insert_with(|| at.clone());
                        }
                    }
                }
            }
            if rel.ends_with("ribbon.rs") {
                for name in ["TABS", "CONTEXTUAL_TABS"] {
                    for l in tokens_after_ident_until_semicolon(toks, name) {
                        p1.entry(l.to_string())
                            .or_insert_with(|| format!("{rel}:1"));
                    }
                }
            }
        }
        // Constraint (design 2.1): P1 keys must be keys of pl.tsv.
        p1.retain(|s, _| pl_keys.contains(&(String::new(), s.clone())));
    } else {
        for (rel, toks) in &srcs {
            let file = rel.rsplit('/').next().unwrap_or("");
            let take_all =
                matches!(file, "commands.rs" | "catalog.rs") && rel.starts_with("crates/engine");
            let bulk = file == "bulk_fields.rs";
            let sign = rel.starts_with("crates/sign") && file == "pdf.rs";
            if take_all || bulk || sign {
                for t in toks {
                    if let Some(l) = t.lit() {
                        p1.entry(l.to_string())
                            .or_insert_with(|| format!("{rel}:{}", t.line));
                    }
                }
            }
        }
        for extra in [
            "Mixed",
            "Custom font",
            "The document is no longer open.",
            "The labels are in English: the signature's font can't draw them in this language.",
        ] {
            p1.entry(extra.to_string()).or_default();
        }
    }
    mark_p1(&mut u, &p1);
    if app == "wordcraft" {
        u.pl_keys = pl_keys;
    }

    if app == "pdfcraft" {
        add_dates(&mut u, &base.join("crates/engine/src/dates.rs"));
    }
    u
}

fn add_dates(u: &mut Universe, path: &Path) {
    let Ok(text) = fs::read_to_string(path) else {
        u.notes.push("dates.rs not found".into());
        return;
    };
    let toks = lex(&text);
    let mut code = String::new();
    let mut by_code: BTreeMap<String, BTreeMap<&str, Vec<String>>> = BTreeMap::new();
    let fields = [
        ("months", "date.month."),
        ("months_short", "date.monthShort."),
        ("days", "date.day."),
        ("days_short", "date.dayShort."),
    ];
    let mut i = 0;
    while i + 2 < toks.len() {
        if toks[i].is_ident("code")
            && toks[i + 1].is_punct(':')
            && let Some(l) = toks[i + 2].lit()
        {
            code = l.to_string();
        }
        for (f, _) in fields {
            if toks[i].is_ident(f) && toks[i + 1].is_punct(':') && toks[i + 2].is_punct('[') {
                let mut j = i + 3;
                let mut v = Vec::new();
                while j < toks.len() && !toks[j].is_punct(']') {
                    if let Some(l) = toks[j].lit() {
                        v.push(l.to_string());
                    }
                    j += 1;
                }
                by_code.entry(code.clone()).or_default().insert(f, v);
            }
        }
        i += 1;
    }
    let (Some(en), Some(ru)) = (by_code.get("en"), by_code.get("ru")) else {
        u.notes.push("dates.rs: en/ru entries not found".into());
        return;
    };
    for (f, prefix) in fields {
        let (Some(e), Some(r)) = (en.get(f), ru.get(f)) else {
            continue;
        };
        for (n, name) in e.iter().enumerate() {
            u.dates.insert(
                (format!("{prefix}{}", n + 1), name.clone()),
                KeyInfo {
                    ru: r.get(n).cloned(),
                    p1: true,
                    where_: "crates/engine/src/dates.rs".into(),
                },
            );
        }
    }
}

// ---------------------------------------------------------------- gridcraft (Rust tables)

/// `"key" => "value"` arms of a `translate` match, or `("key", "value")` tuples of a slice named `slice`.
pub fn parse_rust_table(src: &str, slice: Option<&str>) -> (Vec<(String, String)>, usize) {
    let toks = lex(src);
    let mut out = Vec::new();
    let mut anomalies = 0;
    let mut i = 0;
    let mut in_slice = slice.is_none();
    while i < toks.len() {
        if let Some(name) = slice
            && toks[i].is_ident(name)
            && toks.get(i + 1).is_some_and(|t| t.is_punct(':'))
        {
            in_slice = true;
        }
        if in_slice {
            if slice.is_some() && toks[i].is_punct(';') {
                break;
            }
            if slice.is_none() {
                if let (Some(k), true, true) = (
                    toks[i].lit(),
                    toks.get(i + 1).is_some_and(|t| t.is_punct('=')),
                    toks.get(i + 2).is_some_and(|t| t.is_punct('>')),
                ) {
                    if let Some(v) = toks.get(i + 3).and_then(Token::lit) {
                        out.push((k.to_string(), v.to_string()));
                        i += 4;
                        continue;
                    }
                    // `"key" => { "value" }`
                    if toks.get(i + 3).is_some_and(|t| t.is_punct('{'))
                        && toks.get(i + 5).is_some_and(|t| t.is_punct('}'))
                        && let Some(v) = toks.get(i + 4).and_then(Token::lit)
                    {
                        out.push((k.to_string(), v.to_string()));
                        i += 6;
                        continue;
                    }
                    anomalies += 1;
                } else if toks[i].lit().is_some()
                    && toks.get(i + 1).is_some_and(|t| t.is_punct('|'))
                {
                    anomalies += 1;
                }
            } else if toks[i].is_punct('(')
                && let (Some(k), true, Some(v), true) = (
                    toks.get(i + 1).and_then(Token::lit),
                    toks.get(i + 2).is_some_and(|t| t.is_punct(',')),
                    toks.get(i + 3).and_then(Token::lit),
                    toks.get(i + 4).is_some_and(|t| t.is_punct(')')),
                )
            {
                out.push((k.to_string(), v.to_string()));
                i += 5;
                continue;
            }
        }
        i += 1;
    }
    (out, anomalies)
}

/// Split gridcraft's `context|label` key the way upstream does (`rsplit_once('|')`).
pub fn split_grid_key(key: &str) -> (String, String) {
    match key.rsplit_once('|') {
        Some((c, l)) => (c.to_string(), l.to_string()),
        None => (String::new(), key.to_string()),
    }
}

pub const GRID_CORE: [&str; 6] = ["ru", "de", "fr", "ko", "sr", "zh"];

fn grid_universe(root: &Path) -> Universe {
    let base = app_dir(root, "gridcraft");
    let mut u = Universe {
        app: "gridcraft".into(),
        ..Universe::default()
    };
    let dir = base.join("crates/ui-egui/src/i18n");
    let mut sets: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut anomalies = 0;
    if let Ok(rd) = fs::read_dir(&dir) {
        let mut files: Vec<_> = rd.filter_map(Result::ok).map(|e| e.path()).collect();
        files.sort();
        for p in files
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "rs"))
        {
            let lang = p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let (v, a) = parse_rust_table(&fs::read_to_string(&p).unwrap_or_default(), None);
            anomalies += a;
            sets.insert(lang, v);
        }
    }
    let main = fs::read_to_string(base.join("crates/ui-egui/src/i18n.rs")).unwrap_or_default();
    sets.insert("ja".into(), parse_rust_table(&main, Some("JAPANESE")).0);
    if anomalies > 0 {
        u.notes.push(format!(
            "{anomalies} match arms with alternation or non-literal values were not read"
        ));
    }
    let mut core: Option<BTreeSet<String>> = None;
    for (lang, pairs) in &sets {
        for (k, v) in pairs {
            let (c, l) = split_grid_key(k);
            let info = u.keys.entry((c, l)).or_default();
            if lang == "ru" {
                info.ru = Some(v.clone());
            }
        }
        if GRID_CORE.contains(&lang.as_str()) {
            let keys: BTreeSet<String> = pairs.iter().map(|(k, _)| k.clone()).collect();
            core = Some(match core {
                None => keys,
                Some(c) => c.intersection(&keys).cloned().collect(),
            });
        }
    }
    let core = core.unwrap_or_default();
    for k in &core {
        let (c, l) = split_grid_key(k);
        if let Some(info) = u.keys.get_mut(&(c, l)) {
            info.p1 = true;
        }
    }
    u.notes.push(format!(
        "core set shared by {GRID_CORE:?}: {} keys",
        core.len()
    ));
    let srcs = sources(&base, &["crates/ui-egui/src", "crates/engine/src"]);
    let idx = where_index(&srcs);
    for ((_, src), info) in u.keys.iter_mut() {
        if let Some(w) = idx.get(src) {
            info.where_ = w.clone();
        }
    }
    u
}

// ---------------------------------------------------------------- deckcraft (no catalog)

const SKIP_CALLEES: [&str; 33] = [
    "debug",
    "info",
    "warn",
    "error",
    "trace",
    "panic",
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "unreachable",
    "todo",
    "unimplemented",
    "println",
    "eprintln",
    "dbg",
    "json",
    "expect",
    "env",
    "concat",
    "include_str",
    "include_bytes",
    "contains",
    "starts_with",
    "ends_with",
    "replace",
    "replacen",
    "strip_prefix",
    "strip_suffix",
    "split",
    "find",
];
const SKIP_CALLEES2: [&str; 8] = [
    "trim_start_matches",
    "trim_end_matches",
    "trim_matches",
    "get",
    "remove",
    "insert",
    "entry",
    "var",
];

/// Is this literal shaped like something a user reads (not an id, path, format debug or XML)?
pub fn looks_like_ui(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() || !t.chars().any(char::is_alphabetic) || t.len() > 240 {
        return false;
    }
    let first = t.chars().next().unwrap_or(' ');
    let spaced = t.contains(' ');
    if !(first.is_uppercase() || spaced) {
        return false;
    }
    if matches!(
        first,
        '<' | '/' | '.' | '#' | '{' | '[' | '&' | '%' | '$' | '@'
    ) && !spaced
    {
        return false;
    }
    if t.starts_with('<') || t.contains("://") || t.contains("::") {
        return false;
    }
    if !spaced
        && t.chars().any(|c| {
            matches!(
                c,
                '.' | '_' | ':' | '/' | '+' | '=' | '<' | '>' | '#' | '\\'
            )
        })
        && !t.ends_with('\u{2026}')
    {
        return false;
    }
    if !spaced
        && t.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return false;
    }
    true
}

fn deck_universe(root: &Path) -> Universe {
    let base = app_dir(root, "deckcraft");
    let mut u = Universe {
        app: "deckcraft".into(),
        ..Universe::default()
    };
    let srcs = sources(&base, &["crates/ui-egui/src", "crates/engine/src/cmd"]);
    let mut ui_lits = BTreeSet::new();
    let mut cmd_labels = BTreeSet::new();
    let mut p1: BTreeMap<String, String> = BTreeMap::new();
    for (rel, toks) in &srcs {
        let file = rel.rsplit('/').next().unwrap_or("");
        // Our own mechanism modules (patches 0100/0110) hold error text and registry names, not interface strings.
        let is_ui =
            rel.starts_with("crates/ui-egui") && !matches!(file, "i18n.rs" | "office_overlay.rs");
        let mut stack: Vec<String> = Vec::new();
        for (i, t) in toks.iter().enumerate() {
            if t.is_punct('(') {
                let callee = match (
                    i.checked_sub(1).map(|k| &toks[k]),
                    i.checked_sub(2).map(|k| &toks[k]),
                ) {
                    (Some(p), Some(pp)) if p.is_punct('!') => match &pp.tok {
                        super::lex::Tok::Ident(n) => n.clone(),
                        _ => String::new(),
                    },
                    (Some(p), _) => match &p.tok {
                        super::lex::Tok::Ident(n) => n.clone(),
                        _ => String::new(),
                    },
                    _ => String::new(),
                };
                stack.push(callee);
            } else if t.is_punct(')') {
                stack.pop();
            } else if let Some(l) = t.lit()
                && is_ui
                && looks_like_ui(l)
                && !stack.iter().any(|c| {
                    SKIP_CALLEES.contains(&c.as_str()) || SKIP_CALLEES2.contains(&c.as_str())
                })
            {
                let at = format!("{rel}:{}", t.line);
                ui_lits.insert((l.to_string(), at.clone()));
                if matches!(file, "backstage.rs" | "credits.rs") {
                    p1.entry(l.to_string()).or_insert(at);
                }
            }
        }
        // cmd!( [query|noundo] "id", "Label", [ "Menu", ... ], ... )
        for i in 0..toks.len().saturating_sub(3) {
            if toks[i].is_ident("cmd") && toks[i + 1].is_punct('!') && toks[i + 2].is_punct('(') {
                let l = lits_in_call(toks, i + 2);
                if let Some(label) = l.get(1) {
                    cmd_labels.insert(((*label).to_string(), format!("{rel}:{}", toks[i].line)));
                }
            }
        }
        if file == "ribbon.rs" && is_ui {
            for l in tokens_after_ident_until_semicolon(toks, "TABS") {
                p1.entry(l.to_string())
                    .or_insert_with(|| format!("{rel}:154"));
            }
        }
    }
    for (l, at) in ui_lits.iter().chain(cmd_labels.iter()) {
        let e = u.keys.entry((String::new(), l.clone())).or_default();
        if e.where_.is_empty() {
            e.where_ = at.clone();
        }
    }
    for l in [
        "OK", "Cancel", "Close", "Apply", "Yes", "No", "Save", "Open", "Delete", "Help", "Undo",
        "Redo", "Print", "Export",
    ] {
        p1.entry(l.to_string()).or_default();
    }
    mark_p1(&mut u, &p1);
    let ui_n = ui_lits
        .iter()
        .map(|(l, _)| l)
        .collect::<BTreeSet<_>>()
        .len();
    let cmd_n = cmd_labels
        .iter()
        .map(|(l, _)| l)
        .collect::<BTreeSet<_>>()
        .len();
    let both = cmd_labels
        .iter()
        .filter(|(l, _)| ui_lits.iter().any(|(u, _)| u == l))
        .count();
    u.notes.push(format!("heuristic: {ui_n} distinct UI literals + {cmd_n} cmd! labels, {both} in both; no upstream catalog, so no ru"));
    u
}

// ---------------------------------------------------------------- merge

/// UI strings that our carried UI patches (`patches/*/0121`) add. Upstream's catalogs do not hold them, so without this list
/// every extract would mark their rows obsolete.
const OWN: [(&str, &[&str]); 9] = [
    ("Transliterate", &["wordcraft", "gridcraft", "deckcraft"]),
    (
        "Latin to Cyrillic",
        &["wordcraft", "gridcraft", "deckcraft"],
    ),
    (
        "Cyrillic to Latin",
        &["wordcraft", "gridcraft", "deckcraft"],
    ),
    ("Auto-detect", &["wordcraft", "gridcraft", "deckcraft"]),
    (
        "Nothing to convert",
        &["wordcraft", "gridcraft", "deckcraft"],
    ),
    (
        "{n} items skipped (other language, locked or field)",
        &["wordcraft", "gridcraft", "deckcraft"],
    ),
    ("{n} paragraphs converted", &["wordcraft", "deckcraft"]),
    ("{n} cells converted", &["gridcraft"]),
    (
        "{n} formulas contain text and were not changed",
        &["gridcraft"],
    ),
];

fn add_own(unis: &mut [Universe]) {
    for u in unis {
        for (en, apps) in OWN {
            if apps.contains(&u.app.as_str()) {
                let e = u.keys.entry((String::new(), en.to_string())).or_default();
                e.p1 = true;
                if e.where_.is_empty() {
                    e.where_ = format!("patches/{}/0121", u.app);
                }
            }
        }
    }
}

pub fn extract_all(root: &Path) -> Vec<Universe> {
    let mut unis = vec![
        tsv_universe(root, "wordcraft"),
        grid_universe(root),
        tsv_universe(root, "pdfcraft"),
        deck_universe(root),
    ];
    add_own(&mut unis);
    // wordcraft chrome: strings that are P1 in another app are common UI words; they join P1 when pl.tsv covers them.
    let others: BTreeSet<(String, String)> = unis[1..]
        .iter()
        .flat_map(|u| u.keys.iter().filter(|(_, i)| i.p1).map(|(k, _)| k.clone()))
        .collect();
    let before = unis[0].p1();
    let pl = std::mem::take(&mut unis[0].pl_keys);
    for (k, info) in unis[0].keys.iter_mut() {
        if !info.p1 && k.0.is_empty() && others.contains(k) && pl.contains(k) {
            info.p1 = true;
        }
    }
    let added = unis[0].p1() - before;
    unis[0].notes.push(format!("P1 = {before} command/tab/location labels (pl.tsv-covered) + {added} chrome strings that are P1 in another app"));
    unis
}

/// What a merge changed, per key. `where_` (file:line) churn is not a change: it never makes a catalog stale.
#[derive(Debug, Default)]
pub struct MergeReport {
    /// (scope, id, English) of rows that did not exist.
    pub added: Vec<(String, String, String)>,
    /// (scope, id, English) of rows that no longer exist upstream (kept, `priority = obsolete`).
    pub obsolete: Vec<(String, String, String)>,
    /// (scope, old id, new id, old English, new English): the English changed, so the old Uzbek was carried to the new row as a
    /// `draft:carried:<date>` (never reviewed; release exports skip it) and flagged in `note`.
    pub carried: Vec<(String, String, String, String, String)>,
    /// (scope, id, English, old ru, new ru): upstream's own Russian changed under an unchanged English key.
    pub ru_changed: Vec<(String, String, String, String, String)>,
}

impl MergeReport {
    pub fn is_clean(&self) -> bool {
        self.added.is_empty() && self.obsolete.is_empty() && self.ru_changed.is_empty()
    }

    /// The report, at most `LIMIT` keys per category.
    pub fn text(&self) -> String {
        const LIMIT: usize = 40;
        let short = |s: &str| {
            let one: String = s.replace('\n', " ");
            if one.chars().count() > 70 {
                format!("{}...", one.chars().take(67).collect::<String>())
            } else {
                one
            }
        };
        let mut out = format!(
            "keys added: {}, removed (now obsolete): {}, English changed (carried as draft): {}, upstream ru changed: {}\n",
            self.added.len(),
            self.obsolete.len(),
            self.carried.len(),
            self.ru_changed.len()
        );
        let mut section = |title: &str, lines: Vec<String>| {
            for l in lines.iter().take(LIMIT) {
                out.push_str(&format!("  {title} {l}\n"));
            }
            if lines.len() > LIMIT {
                out.push_str(&format!("  {title} ... and {} more\n", lines.len() - LIMIT));
            }
        };
        section(
            "added  ",
            self.added
                .iter()
                .map(|(sc, id, en)| format!("{sc}/{id} {:?}", short(en)))
                .collect(),
        );
        section(
            "removed",
            self.obsolete
                .iter()
                .map(|(sc, id, en)| format!("{sc}/{id} {:?}", short(en)))
                .collect(),
        );
        section(
            "changed",
            self.carried
                .iter()
                .map(|(sc, o, n, oe, ne)| {
                    format!("{sc}/{o} -> {n} {:?} -> {:?}", short(oe), short(ne))
                })
                .collect(),
        );
        section(
            "ru     ",
            self.ru_changed
                .iter()
                .map(|(sc, id, en, o, n)| {
                    format!(
                        "{sc}/{id} {:?}: {:?} -> {:?}",
                        short(en),
                        short(o),
                        short(n)
                    )
                })
                .collect(),
        );
        out
    }
}

/// Levenshtein distance in chars.
fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cur.push(
                (prev[j] + usize::from(ca != cb))
                    .min(prev[j + 1] + 1)
                    .min(cur[j] + 1),
            );
        }
        prev = cur;
    }
    prev[b.len()]
}

/// [`merge_report`], counts only: (new rows, rows turned obsolete).
#[cfg(test)]
fn merge(cat: &mut Catalog, unis: &[Universe]) -> (usize, usize) {
    let r = merge_report(cat, unis, "1970-01-01", true);
    (r.added.len(), r.obsolete.len())
}

/// Merge the universes into the catalog. Existing rows keep their Uzbek values, statuses and placement.
/// A key whose English changed (an obsolete row and a new row of one scope and context within 20% edit distance) hands its
/// Uzbek to the new row as `draft:carried:<date>`, with a note naming the old row and both English strings (design 6.3).
///
/// `where_` (file:line) is a locator that drifts with every upstream edit. Unless `refresh_where` is set, an existing row keeps
/// its `where_` while the file part is unchanged: a weekly sync then changes the TSVs only where a key or a value changed,
/// not on every row whose line number moved. A new file part, or a row without one, always takes the new value.
pub fn merge_report(
    cat: &mut Catalog,
    unis: &[Universe],
    date: &str,
    refresh_where: bool,
) -> MergeReport {
    let mut rep = MergeReport::default();
    type Key = (String, String);
    // key -> app -> info (UI keys and date keys alike)
    let mut by_key: BTreeMap<Key, BTreeMap<&str, &KeyInfo>> = BTreeMap::new();
    for u in unis {
        for (k, info) in u.keys.iter().chain(u.dates.iter()) {
            by_key.entry(k.clone()).or_default().insert(&u.app, info);
        }
    }
    // existing placement
    let mut existing: BTreeMap<Key, BTreeSet<String>> = BTreeMap::new();
    for (scope, rows) in &cat.scopes {
        if scope == "glossary" {
            continue;
        }
        for r in rows {
            existing
                .entry((r.context.clone(), r.source.clone()))
                .or_default()
                .insert(scope.clone());
        }
    }
    let mut seen: BTreeSet<(String, Key)> = BTreeSet::new();
    for (key, per_app) in &by_key {
        let apps: Vec<&str> = per_app.keys().copied().collect();
        let ru_vals: BTreeSet<&String> = per_app.values().filter_map(|i| i.ru.as_ref()).collect();
        let shared_ok = apps.len() >= 2 && ru_vals.len() <= 1;
        let places = existing.get(key).cloned().unwrap_or_default();
        let mut targets: Vec<(String, Vec<&str>)> = Vec::new();
        if places.contains("shared") {
            targets.push(("shared".into(), apps.clone()));
            for a in &apps {
                if places.contains(*a) {
                    targets.push((a.to_string(), vec![*a]));
                }
            }
        } else {
            let unplaced: Vec<&str> = apps
                .iter()
                .copied()
                .filter(|a| !places.contains(*a))
                .collect();
            for a in &apps {
                if places.contains(*a) {
                    targets.push((a.to_string(), vec![*a]));
                }
            }
            if shared_ok && places.is_empty() {
                targets.push(("shared".into(), apps.clone()));
            } else {
                for a in unplaced {
                    targets.push((a.to_string(), vec![a]));
                }
            }
        }
        for (scope, tapps) in targets {
            seen.insert((scope.clone(), key.clone()));
            let rows = cat.scopes.entry(scope.clone()).or_default();
            let pos = rows
                .iter()
                .position(|r| r.context == key.0 && r.source == key.1);
            let infos: Vec<&&KeyInfo> = tapps.iter().filter_map(|a| per_app.get(a)).collect();
            let ru = infos.iter().find_map(|i| i.ru.clone()).unwrap_or_default();
            let ru_apps: Vec<&str> = tapps
                .iter()
                .copied()
                .filter(|a| per_app.get(a).is_some_and(|i| i.ru.is_some()))
                .collect();
            let ru_apps = if scope == "shared" && ru_apps.len() != tapps.len() {
                ru_apps.join(",")
            } else {
                String::new()
            };
            let p1_apps: Vec<&str> = tapps
                .iter()
                .copied()
                .filter(|a| per_app.get(a).is_some_and(|i| i.p1))
                .collect();
            let priority = if p1_apps.is_empty() {
                "P2".to_string()
            } else if p1_apps.len() == tapps.len() {
                "P1".to_string()
            } else {
                let mut v = p1_apps.clone();
                v.sort_unstable();
                format!("P1:{}", v.join("+"))
            };
            let where_ = infos
                .iter()
                .map(|i| i.where_.clone())
                .find(|w| !w.is_empty())
                .unwrap_or_default();
            let mut apps_sorted: Vec<&str> = tapps.clone();
            apps_sorted.sort_unstable();
            let apps_s = apps_sorted.join(",");
            match pos {
                Some(p) => {
                    let r = &mut rows[p];
                    r.apps = apps_s;
                    r.ru_apps = ru_apps;
                    r.priority = priority;
                    let file_of = |w: &str| {
                        w.rsplit_once(':')
                            .map_or(w.to_string(), |(f, _)| f.to_string())
                    };
                    if refresh_where
                        || r.where_.is_empty()
                        || file_of(&r.where_) != file_of(&where_)
                    {
                        r.where_ = where_;
                    }
                    if !ru.is_empty()
                        && (r.ru_status == "upstream" || r.ru.is_empty() || r.ru_status == "todo")
                    {
                        if !r.ru.is_empty() && r.ru != ru {
                            rep.ru_changed.push((
                                scope.clone(),
                                r.id.clone(),
                                r.source.clone(),
                                r.ru.clone(),
                                ru.clone(),
                            ));
                        }
                        r.ru = ru;
                        r.ru_status = "upstream".into();
                    }
                }
                None => {
                    let id = row_id(&scope, &key.0, &key.1);
                    rep.added.push((scope.clone(), id.clone(), key.1.clone()));
                    rows.push(Row {
                        id,
                        context: key.0.clone(),
                        source: key.1.clone(),
                        uz_latn_status: "todo".into(),
                        uz_cyrl_status: "todo".into(),
                        ru_status: if ru.is_empty() {
                            "todo".into()
                        } else {
                            "upstream".into()
                        },
                        ru,
                        ru_apps,
                        priority,
                        apps: apps_s,
                        where_,
                        ..Row::default()
                    });
                }
            }
        }
    }
    for (scope, rows) in cat.scopes.iter_mut() {
        if scope == "glossary" {
            continue;
        }
        for r in rows.iter_mut() {
            if !seen.contains(&(scope.clone(), (r.context.clone(), r.source.clone())))
                && r.priority != "obsolete"
            {
                r.priority = "obsolete".into();
                rep.obsolete
                    .push((scope.clone(), r.id.clone(), r.source.clone()));
            }
        }
    }
    carry_changed_english(cat, &mut rep, date);
    rep
}

/// Pair each newly obsolete row that has Uzbek with the closest new row of its scope and context (within 20% edit distance)
/// and carry the Uzbek over as an unreviewed draft. An old `reviewed:owner` status does not survive: the English changed.
fn carry_changed_english(cat: &mut Catalog, rep: &mut MergeReport, date: &str) {
    let mut taken: BTreeSet<String> = BTreeSet::new();
    for (scope, old_id, old_en) in rep.obsolete.clone() {
        let Some(rows) = cat.scopes.get_mut(&scope) else {
            continue;
        };
        let Some(old) = rows.iter().find(|r| r.id == old_id).cloned() else {
            continue;
        };
        if old.uz_latn.is_empty() && old.uz_cyrl.is_empty() {
            continue;
        }
        let best = rep
            .added
            .iter()
            .filter(|(sc, id, _)| *sc == scope && !taken.contains(id))
            .filter_map(|(_, id, en)| {
                let ctx = rows
                    .iter()
                    .find(|r| r.id == *id)
                    .map(|r| r.context.clone())?;
                let d = edit_distance(&old_en, en);
                (ctx == old.context && d * 5 <= old_en.chars().count().max(en.chars().count()))
                    .then(|| (d, id.clone(), en.clone()))
            })
            .min();
        let Some((_, new_id, new_en)) = best else {
            continue;
        };
        taken.insert(new_id.clone());
        if let Some(row) = rows.iter_mut().find(|r| r.id == new_id) {
            let carried = format!("draft:carried:{date}");
            if !old.uz_latn.is_empty() {
                row.uz_latn = old.uz_latn.clone();
                row.uz_latn_status = carried.clone();
            }
            if !old.uz_cyrl.is_empty() {
                row.uz_cyrl = old.uz_cyrl.clone();
                row.uz_cyrl_status = carried;
            }
            row.note = format!("carried from {old_id}; English changed: {old_en:?} -> {new_en:?}");
        }
        rep.carried.push((scope, old_id, new_id, old_en, new_en));
    }
}

/// The per-app report: extracted vs design numbers.
pub fn report(unis: &[Universe], cat: Option<&Catalog>) -> String {
    let mut out = String::from("app        keys(extracted)  design   ru   P1   dates  notes\n");
    for u in unis {
        let design = DESIGN_COUNTS
            .iter()
            .find(|(a, _)| *a == u.app)
            .map_or(0, |d| d.1);
        let ctxs: BTreeMap<&str, usize> = u.keys.keys().fold(BTreeMap::new(), |mut m, (c, _)| {
            *m.entry(if c.is_empty() { "" } else { c.as_str() })
                .or_default() += 1;
            m
        });
        let mut line = format!(
            "{:<10} {:>8} {:>16} {:>6} {:>4} {:>7}",
            u.app,
            u.keys.len(),
            design,
            u.with_ru(),
            u.p1(),
            u.dates.len()
        );
        let special = ctxs
            .iter()
            .filter(|(c, _)| !c.is_empty())
            .map(|(c, n)| format!("{c}:{n}"))
            .collect::<Vec<_>>();
        if !special.is_empty() {
            let shown: Vec<_> = special.iter().take(6).cloned().collect();
            line.push_str(&format!(
                "  contexts {} ({} kinds)",
                shown.join(" "),
                special.len()
            ));
        }
        out.push_str(&line);
        out.push('\n');
        for n in &u.notes {
            out.push_str(&format!("           - {n}\n"));
        }
    }
    if let Some(cat) = cat {
        for scope in ["shared"].iter().chain(APPS.iter()) {
            out.push_str(&format!(
                "rows in {scope}.tsv: {}\n",
                cat.scopes.get(*scope).map_or(0, Vec::len)
            ));
        }
        for a in APPS {
            out.push_str(&format!("rows visible to {a}: {}\n", cat.app_rows(a).len()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_tsv_parse_skips_comments_and_first_duplicate_wins() {
        let (e, bad) = read_upstream_tsv(
            "# c\n\n\tPaste\tВставить\n\tPaste\tX\nbroken line\npreview\tAa\tАа\n",
        );
        assert_eq!(
            e,
            vec![
                ("".into(), "Paste".into(), "Вставить".into()),
                ("preview".into(), "Aa".into(), "Аа".into())
            ]
        );
        assert_eq!(bad, 1);
    }

    #[test]
    fn rust_table_arms_and_tuples() {
        let src = "pub fn t(x:&str)->Option<&'static str>{ Some(match x { \"Paste\" => \"A\", \"Page Layout|Orientation\" => \"B\\n\", _ => return None }) }";
        let (v, a) = parse_rust_table(src, None);
        assert_eq!(
            v,
            vec![
                ("Paste".to_string(), "A".to_string()),
                ("Page Layout|Orientation".to_string(), "B\n".to_string())
            ]
        );
        assert_eq!(a, 0);
        let (v, _) = parse_rust_table(
            "const JAPANESE: &[(&str,&str)] = &[(\"Paste\", \"x\"), (\"Cut\", \"y\")]; const OTHER: &[(&str,&str)] = &[(\"No\", \"no\")];",
            Some("JAPANESE"),
        );
        assert_eq!(v.len(), 2); // stops at the first `;` after the slice name
        assert_eq!(
            split_grid_key("Page Layout|Orientation"),
            ("Page Layout".to_string(), "Orientation".to_string())
        );
        assert_eq!(split_grid_key("Cut"), (String::new(), "Cut".to_string()));
    }

    #[test]
    fn ui_literal_heuristic() {
        for ok in [
            "Save",
            "Insert Table\u{2026}",
            "Slide {n} of {total}",
            "OK",
            "a sentence with spaces",
        ] {
            assert!(looks_like_ui(ok), "{ok}");
        }
        for no in [
            "insert.dateTime",
            "ppt/slides/slide1.xml",
            "Ctrl+Z",
            "#ff00aa",
            "<p:sp x>",
            "100%",
            "left",
            "std::io",
        ] {
            assert!(!looks_like_ui(no), "{no}");
        }
    }

    #[test]
    fn merge_shares_keys_and_keeps_uzbek_on_rerun() {
        let mk = |app: &str, ru: Option<&str>| {
            let mut u = Universe {
                app: app.into(),
                ..Universe::default()
            };
            u.keys.insert(
                (String::new(), "Paste".into()),
                KeyInfo {
                    ru: ru.map(String::from),
                    p1: true,
                    where_: "x.rs:1".into(),
                },
            );
            u.keys
                .insert((String::new(), format!("Only {app}")), KeyInfo::default());
            u
        };
        let unis = vec![mk("wordcraft", Some("Вставить")), mk("gridcraft", None)];
        let mut cat = Catalog::default();
        let (added, _) = merge(&mut cat, &unis);
        assert_eq!(added, 3);
        let shared = &cat.scopes["shared"][0];
        assert_eq!(
            (
                shared.source.as_str(),
                shared.apps.as_str(),
                shared.ru_apps.as_str(),
                shared.priority.as_str()
            ),
            ("Paste", "gridcraft,wordcraft", "wordcraft", "P1")
        );
        assert!(shared.ru_in_app("wordcraft") && !shared.ru_in_app("gridcraft"));
        cat.scopes.get_mut("shared").unwrap()[0].uz_latn = "Qo\u{2018}yish".into();
        let (added, obsolete) = merge(&mut cat, &unis);
        assert_eq!((added, obsolete), (0, 0));
        assert_eq!(cat.scopes["shared"][0].uz_latn, "Qo\u{2018}yish");
        // a key that vanishes becomes obsolete, not deleted
        let (_, obsolete) = merge(&mut cat, &unis[..1]);
        assert!(obsolete >= 1);
    }

    #[test]
    fn priority_is_scoped_to_the_apps_where_the_key_is_p1() {
        let mk = |app: &str, p1: bool| {
            let mut u = Universe {
                app: app.into(),
                ..Universe::default()
            };
            u.keys.insert(
                (String::new(), "Cut".into()),
                KeyInfo {
                    p1,
                    ..KeyInfo::default()
                },
            );
            u
        };
        let mut cat = Catalog::default();
        merge(&mut cat, &[mk("wordcraft", true), mk("pdfcraft", false)]);
        let r = &cat.scopes["shared"][0];
        assert_eq!(r.priority, "P1:wordcraft");
        assert!(r.is_p1("wordcraft") && !r.is_p1("pdfcraft"));
    }

    #[test]
    fn conflicting_ru_keeps_rows_per_app() {
        let mk = |app: &str, ru: &str| {
            let mut u = Universe {
                app: app.into(),
                ..Universe::default()
            };
            u.keys.insert(
                (String::new(), "About".into()),
                KeyInfo {
                    ru: Some(ru.into()),
                    ..KeyInfo::default()
                },
            );
            u
        };
        let mut cat = Catalog::default();
        merge(&mut cat, &[mk("wordcraft", "A"), mk("pdfcraft", "B")]);
        assert!(!cat.scopes.contains_key("shared"));
        assert_eq!(cat.scopes["wordcraft"][0].ru, "A");
        assert_eq!(cat.scopes["pdfcraft"][0].ru, "B");
    }

    #[test]
    fn changed_english_carries_the_uzbek_as_an_unreviewed_draft() {
        let mk = |en: &str, ru: &str| {
            let mut u = Universe {
                app: "pdfcraft".into(),
                ..Universe::default()
            };
            u.keys.insert(
                (String::new(), en.into()),
                KeyInfo {
                    ru: Some(ru.into()),
                    ..KeyInfo::default()
                },
            );
            u
        };
        let mut cat = Catalog::default();
        merge(
            &mut cat,
            &[mk(
                "You are viewing revision {n}",
                "Вы просматриваете версию {n}",
            )],
        );
        {
            let r = &mut cat.scopes.get_mut("pdfcraft").unwrap()[0];
            r.uz_latn = "Siz {n}-tahrirni koʻryapsiz".into();
            r.uz_latn_status = "reviewed:owner:2026-10-21".into();
        }
        let rep = merge_report(
            &mut cat,
            &[mk(
                "You are viewing revision {n}.",
                "Вы просматриваете версию {n}.",
            )],
            "2026-10-18",
            true,
        );
        assert_eq!(
            (rep.added.len(), rep.obsolete.len(), rep.carried.len()),
            (1, 1, 1)
        );
        assert!(!rep.is_clean());
        let new = cat.scopes["pdfcraft"]
            .iter()
            .find(|r| r.priority != "obsolete")
            .unwrap();
        assert_eq!(new.uz_latn, "Siz {n}-tahrirni koʻryapsiz");
        assert_eq!(
            new.uz_latn_status, "draft:carried:2026-10-18",
            "a reviewed status never survives an English change"
        );
        assert!(new.note.starts_with("carried from "), "{}", new.note);
        // an unrelated new key is not a carry
        let rep = merge_report(
            &mut cat,
            &[mk("Completely different text here", "Совсем другое")],
            "2026-10-18",
            true,
        );
        assert!(rep.carried.is_empty());
    }
}
