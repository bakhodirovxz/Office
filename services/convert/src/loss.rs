//! The static loss table (PLAN 2.6 source 3), first version. Rows apply by (input kind, target) alone; WP3 adds the input
//! scanner so that a row fires only when the feature is present, and moves the text to the i18n catalog.
//! Evidence: docs/phase0/fidelity-baseline.md and docs/phase3/census.md.
use crate::job::Warning;
use crate::kind::{Family, Kind};
use crate::plan::Target;

struct Row {
    code: &'static str,
    applies: fn(Kind, Target) -> bool,
    text: &'static str,
}

const ROWS: &[Row] = &[
    Row {
        code: "doc_import_loss",
        // census 1.1: .doc import prints nothing but drops comments and tracked changes (baseline: 1 -> 0 each)
        applies: |k, _| k == Kind::Doc,
        text: "legacy .doc import: comments and tracked changes are not kept, and paragraph counts can differ",
    },
    Row {
        code: "xls_pdf_chart_page",
        // census 1.2: xls -> pdf gives 3 pages where the xlsx gives 4 (the chart is lost)
        applies: |k, t| k == Kind::Xls && t == Target::Pdf,
        text: "legacy .xls import drops charts, so the PDF can have fewer pages than the same workbook saved as .xlsx",
    },
    Row {
        code: "csv_one_sheet_en_us",
        // PLAN F6 / locale-check: displayed values, one sheet per file, en-US separators
        applies: |k, t| k.family() == Family::Sheets && t == Target::Csv,
        text: "CSV holds one sheet of displayed values with en-US number and date formats (3,500,000.50, English month names)",
    },
    Row {
        code: "pdf_en_us",
        applies: |k, t| k.family() == Family::Sheets && t == Target::Pdf,
        text: "numbers and dates in the PDF use en-US formats (3,500,000.50, English month names)",
    },
    Row {
        code: "plain_text_layout",
        applies: |k, t| k.family() == Family::Docs && matches!(t, Target::Md | Target::Txt) && !matches!(k, Kind::Md | Kind::Txt),
        text: "Markdown and text drop page layout, headers, footers and images",
    },
];

pub fn warnings(k: Kind, t: Target) -> Vec<Warning> {
    ROWS.iter().filter(|r| (r.applies)(k, t)).map(|r| Warning { code: r.code.into(), count: 1, text: r.text.into() }).collect()
}
