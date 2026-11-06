//! The command allowlist as data (PLAN phase 7, section 5.2). Deny by default.
//!
//! An id is allowed when it starts with an allowed prefix and is not in [`EXCEPTIONS`], or is one of [`READS`]. The tests in
//! `docs-office` (which can see the real Docs registry) check that every exception and read exists and that no allowed spec's
//! `params` description mentions a path or URL; this crate cannot depend on an engine.

use serde_json::Value;

use crate::manifest::DocAccess;

/// Families a plugin may run. Everything else (`file.`, `edit.`, `tools.`, `mailings.`, `view.`, `readAloud.`, `ui.`,
/// `office.`, `plugin.`, `link.`, `document.setText`, ...) is refused because no prefix matches.
pub const PREFIXES: &[&str] = &[
    "caret.",
    "select.",
    "text.",
    "format.",
    "font.",
    "para.",
    "styles.",
    "list.",
    "table.",
    "fields.",
    "developer.",
    "insert.",
    "review.",
];

/// Pure reads of the document; they need `document = "read"`.
pub const READS: &[&str] = &[
    "document.text",
    "document.paragraph",
    "document.selection",
    "document.inspect",
];

/// Ids inside an allowed prefix that are refused anyway, with the reason (shown in `docs/plugins.md`).
pub const EXCEPTIONS: &[(&str, &str)] = &[
    ("insert.textFromFile", "reads a file"),
    ("insert.picture", "reads a file or arbitrary bytes"),
    ("review.acceptAll", "accept all is a user decision"),
    ("review.rejectAll", "reject all is a user decision"),
    ("font.add", "installs a font from a file or bytes"),
    ("font.loadSystem", "reads the machine's font folders"),
    ("font.remove", "changes the installed fonts"),
    ("insert.link", "a link target can be a file or a script URL"),
    ("insert.object", "embeds a file"),
    ("review.compare", "reads a second document by path"),
    ("review.combine", "reads a second document by path"),
    ("styles.organizer", "reads styles from a file"),
    ("styles.copyFrom", "reads styles from a file"),
    (
        "insert.field",
        "an instruction can be HYPERLINK, INCLUDEPICTURE, INCLUDETEXT or DDEAUTO",
    ),
    (
        "fields.setCode",
        "an instruction can be HYPERLINK, INCLUDEPICTURE, INCLUDETEXT or DDEAUTO",
    ),
    (
        "review.addToDictionary",
        "writes the user's dictionary file, outside the undo step",
    ),
    (
        "review.accept",
        "a loop of accepts is accept all, which is a user decision",
    ),
    (
        "review.reject",
        "a loop of rejects is reject all, which is a user decision",
    ),
    (
        "review.restrict",
        "a plugin must not lock the document or set a password",
    ),
];

/// A `params` key with one of these names (any depth, any case) refuses the `run` action, unless its value is an array of
/// non-negative integers (a node path such as `document.paragraph {"path": [3]}`, which cannot name a file). `link` is a hyperlink
/// target in `CharProps` (also inside `styles.*`); `ins`, `del`, `fmtChange` and `numChange` forge tracked-change attribution.
pub const DENIED_KEYS: &[&str] = &[
    "path",
    "dir",
    "file",
    "url",
    "uri",
    "href",
    "src",
    "link",
    "ins",
    "del",
    "fmtChange",
    "fmt_change",
    "numChange",
    "num_change",
];

const MAX_DEPTH: usize = 32;

/// One app's allowlist (Docs above; Sheets and Slides added in Phase 7 WP13). Deny by default, as for Docs.
pub struct List {
    pub prefixes: &'static [&'static str],
    pub reads: &'static [&'static str],
    pub exceptions: &'static [(&'static str, &'static str)],
    /// Keys refused on top of [`DENIED_KEYS`] (any depth, any case).
    pub denied_keys: &'static [&'static str],
    /// Refuse a params string that the app would read as a formula (Sheets: `=...`, `+A1`, `-SUM(...)`, `@...`).
    pub no_formulas: bool,
}

pub const DOCS: List = List {
    prefixes: PREFIXES,
    reads: READS,
    exceptions: EXCEPTIONS,
    denied_keys: &[],
    no_formulas: false,
};

/// Sheets (gridcraft). Cell values, formatting and the selection; `cell.set`, `range.setValues` and `range.fill` may write text
/// and numbers but no formula, because a formula can reach other files and programs (external references, `WEBSERVICE`,
/// `HYPERLINK`, DDE). Everything else (`file.`, `edit.`, `data.`, `review.`, `view.`, `formulas.`, `insert.`, `sheet.`,
/// `table.`, `pivot.`, `tools.`, `automate.`, `office.`, `plugin.`, ...) is refused because no prefix matches.
pub const SHEETS: List = List {
    prefixes: &["cell.", "range.", "home.", "selection."],
    reads: &[
        "document.inspect",
        "sheet.read",
        "cell.get",
        "selection.stats",
    ],
    exceptions: &[
        ("home.addIns", "add-ins load code"),
        (
            "home.lockCell",
            "a plugin must not change what the sheet protection locks",
        ),
        ("home.insertCopiedCells", "reads the clipboard"),
    ],
    denied_keys: &[],
    no_formulas: true,
};

/// Slides (deckcraft). Text, its formatting, tables and the selection. A run's `link` and `hover` are hyperlinks whose action
/// can start a program or open a file, so both keys (and `action`) are refused.
pub const SLIDES: List = List {
    prefixes: &["text.", "format.", "table.", "selection."],
    reads: &["document.inspect", "slide.inspect", "shape.inspect"],
    exceptions: &[
        ("format.fontDialog", "opens a dialog"),
        ("format.paragraphDialog", "opens a dialog"),
    ],
    denied_keys: &["hover", "action"],
    no_formulas: false,
};

/// The allowlist of `app` (`docs`, `sheets`, `slides`); `None` for an app with no plugin adapter (`pdf`).
pub fn list(app: &str) -> Option<&'static List> {
    match app {
        "docs" => Some(&DOCS),
        "sheets" => Some(&SHEETS),
        "slides" => Some(&SLIDES),
        _ => None,
    }
}

/// The document access a `run` of `id` with `params` needs in Docs, or why it is refused.
pub fn check_run(id: &str, params: &Value) -> Result<DocAccess, &'static str> {
    check_run_in(&DOCS, id, params)
}

/// The document access a `run` of `id` with `params` needs under `l`, or why it is refused.
pub fn check_run_in(l: &List, id: &str, params: &Value) -> Result<DocAccess, &'static str> {
    let access = if l.reads.contains(&id) {
        DocAccess::Read
    } else if l.prefixes.iter().any(|p| id.starts_with(p))
        && !l.exceptions.iter().any(|(e, _)| *e == id)
    {
        DocAccess::Write
    } else {
        return Err("command is not on the plugin allowlist");
    };
    if let Some(why) = bad_params(params, 0, l) {
        return Err(why);
    }
    Ok(access)
}

/// What Sheets would read as a formula (gridcraft `input_to_cell`: `=` first, or `+`/`-` before a letter or `(`), plus `@`
/// (Excel's implicit intersection); leading blanks are skipped, so the check is stricter than the app.
fn formula_like(s: &str) -> bool {
    let t = s.trim_start();
    let mut c = t.chars();
    match c.next() {
        Some('=') | Some('@') => true,
        Some('+') | Some('-') => c
            .next()
            .is_some_and(|n| n.is_alphabetic() || n == '(' || n == '$'),
        _ => false,
    }
}

fn node_path(v: &Value) -> bool {
    v.as_array().is_some_and(|a| a.iter().all(Value::is_u64))
}

fn bad_params(v: &Value, depth: usize, l: &List) -> Option<&'static str> {
    if depth > MAX_DEPTH {
        return Some("params nest too deeply");
    }
    match v {
        Value::Object(m) => m.iter().find_map(|(k, v)| {
            if DENIED_KEYS
                .iter()
                .chain(l.denied_keys)
                .any(|d| k.eq_ignore_ascii_case(d))
                && !node_path(v)
            {
                Some("params name a path, file or URL")
            } else {
                bad_params(v, depth + 1, l)
            }
        }),
        Value::Array(a) => a.iter().find_map(|v| bad_params(v, depth + 1, l)),
        Value::String(s) if l.no_formulas && formula_like(s) => {
            Some("params hold a formula, which can reach other files and programs")
        }
        _ => None,
    }
}
