//! The report (PLAN 2.6): the per-file record that `--json` prints and the API returns, and the warning texts.
//! Warning codes are stable. Their English text lives in the i18n catalog (`i18n/converter.tsv`, context `warning.<code>`),
//! the single source: this module reads it at build time, so the Rust side never holds a second copy.
//! Schema: `data/report.schema.json` (version `SCHEMA`).
use crate::exec::{Exit, StepReport};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Bumped when a field changes meaning or goes away; adding an optional field does not bump it.
pub const SCHEMA: u8 = 1;

#[derive(Debug, Clone, Serialize)]
pub struct Warning {
    pub code: String,
    pub count: u32,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileReport {
    pub schema: u8,
    pub input: String,
    pub kind: Option<String>,
    pub target: String,
    pub status: &'static str,
    pub code: u8,
    pub message: String,
    pub steps: Vec<StepReport>,
    pub warnings: Vec<Warning>,
    /// the result JSON of the transliteration command (paragraphs, runs, skipped, rulesVersion), when the job ran one
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translit: Option<serde_json::Value>,
    /// `--verify` ran and every output re-opened with its own CLI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified: Option<bool>,
    /// WP15: present only when the XDFU hook is on and found a grif: `{detected, mode, copy, refused, evidence:[{place, part, marker}]}`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xdfu: Option<serde_json::Value>,
    pub outputs: Vec<String>,
}

impl FileReport {
    /// `--strict`: a successful job with warnings becomes exit 10.
    pub fn apply_strict(&mut self) {
        if self.code == 0 && !self.warnings.is_empty() {
            self.code = Exit::Warnings.code();
            self.message = format!("done, but with {} warning(s) and --strict is on", self.warnings.len());
        }
    }
}

const CATALOG: &str = include_str!("../../../i18n/converter.tsv");

/// `warning.<code>` rows of the catalog: code -> English text (TSV escapes undone).
fn messages() -> &'static HashMap<String, String> {
    static M: OnceLock<HashMap<String, String>> = OnceLock::new();
    M.get_or_init(|| {
        CATALOG
            .lines()
            .skip(1)
            .filter_map(|l| {
                let c: Vec<&str> = l.split('\t').collect();
                let code = c.get(1)?.strip_prefix("warning.")?;
                Some((code.to_string(), c.get(2)?.replace("\\n", "\n").replace("\\t", "\t").replace("\\\\", "\\")))
            })
            .collect()
    })
}

/// The English text of a warning code. A code without a catalog row is a bug that the test `every_code_has_text` catches;
/// the code itself is returned so that a report is never empty-handed.
pub fn text(code: &str) -> String {
    messages().get(code).cloned().unwrap_or_else(|| code.to_string())
}

/// `text(code)` with `{name}` placeholders filled.
pub fn text_with(code: &str, fill: &[(&str, &str)]) -> String {
    fill.iter().fold(text(code), |t, (k, v)| t.replace(&format!("{{{k}}}"), v))
}

pub fn warn(code: &str, count: u32) -> Warning {
    Warning { code: code.into(), count, text: text(code) }
}

pub fn codes() -> Vec<String> {
    let mut v: Vec<String> = messages().keys().cloned().collect();
    v.sort();
    v
}
