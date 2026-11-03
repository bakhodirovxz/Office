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
    ("insert.field", "an instruction can be HYPERLINK, INCLUDEPICTURE, INCLUDETEXT or DDEAUTO"),
    ("fields.setCode", "an instruction can be HYPERLINK, INCLUDEPICTURE, INCLUDETEXT or DDEAUTO"),
    ("review.addToDictionary", "writes the user's dictionary file, outside the undo step"),
    ("review.accept", "a loop of accepts is accept all, which is a user decision"),
    ("review.reject", "a loop of rejects is reject all, which is a user decision"),
    ("review.restrict", "a plugin must not lock the document or set a password"),
];

/// A `params` key with one of these names (any depth, any case) refuses the `run` action, unless its value is an array of
/// non-negative integers (a node path such as `document.paragraph {"path": [3]}`, which cannot name a file). `link` is a hyperlink
/// target in `CharProps` (also inside `styles.*`); `ins`, `del`, `fmtChange` and `numChange` forge tracked-change attribution.
pub const DENIED_KEYS: &[&str] = &[
    "path", "dir", "file", "url", "uri", "href", "src", "link", "ins", "del", "fmtChange", "fmt_change", "numChange", "num_change",
];

const MAX_DEPTH: usize = 32;

/// The document access a `run` of `id` with `params` needs, or why it is refused.
pub fn check_run(id: &str, params: &Value) -> Result<DocAccess, &'static str> {
    let access = if READS.contains(&id) {
        DocAccess::Read
    } else if PREFIXES.iter().any(|p| id.starts_with(p))
        && !EXCEPTIONS.iter().any(|(e, _)| *e == id)
    {
        DocAccess::Write
    } else {
        return Err("command is not on the plugin allowlist");
    };
    if let Some(why) = bad_params(params, 0) {
        return Err(why);
    }
    Ok(access)
}

fn node_path(v: &Value) -> bool {
    v.as_array().is_some_and(|a| a.iter().all(Value::is_u64))
}

fn bad_params(v: &Value, depth: usize) -> Option<&'static str> {
    if depth > MAX_DEPTH {
        return Some("params nest too deeply");
    }
    match v {
        Value::Object(m) => m.iter().find_map(|(k, v)| {
            if DENIED_KEYS.iter().any(|d| k.eq_ignore_ascii_case(d)) && !node_path(v) {
                Some("params name a path, file or URL")
            } else {
                bad_params(v, depth + 1)
            }
        }),
        Value::Array(a) => a.iter().find_map(|v| bad_params(v, depth + 1)),
        _ => None,
    }
}
