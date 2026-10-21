//! The static loss table (PLAN 2.6 source 3). The rows are data (`data/losses.toml`, each citing its evidence); this module
//! only selects them. A row fires when the input kind and the target match **and** the scanner finds the feature the row
//! needs, so a plain letter to pdf stays quiet.
use crate::kind::Kind;
use crate::plan::Target;
use crate::report::{Warning, text};
use crate::scan::Scan;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
pub struct Row {
    pub code: String,
    #[serde(default)]
    pub from: Vec<String>,
    #[serde(default)]
    pub to: Vec<String>,
    #[serde(default)]
    pub requires: Vec<String>,
    /// does not apply when the target is the input's own format
    #[serde(default)]
    pub cross: bool,
    pub evidence: String,
    #[serde(default)]
    pub until: Option<String>,
}

#[derive(Deserialize)]
struct File {
    row: Vec<Row>,
}

pub fn rows() -> &'static [Row] {
    static R: OnceLock<Vec<Row>> = OnceLock::new();
    R.get_or_init(|| toml::from_str::<File>(include_str!("../data/losses.toml")).expect("data/losses.toml is valid (a test checks it)").row)
}

fn hit(r: &Row, k: Kind, t: Target, scan: &Scan) -> Option<u32> {
    if !(r.from.is_empty() || r.from.iter().any(|f| f == k.ext())) || !(r.to.is_empty() || r.to.iter().any(|x| x == t.ext())) {
        return None;
    }
    if r.cross && k.ext() == t.ext() {
        return None;
    }
    if r.requires.is_empty() {
        return Some(1);
    }
    if !scan.inspected {
        return Some(1);
    }
    let n: u32 = r.requires.iter().map(|f| scan.count(f)).sum();
    (n > 0).then_some(n)
}

pub fn warnings(scan: &Scan, k: Kind, t: Target) -> Vec<Warning> {
    rows().iter().filter_map(|r| hit(r, k, t, scan).map(|n| Warning { code: r.code.clone(), count: n, text: text(&r.code) })).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::Scan;

    fn with(features: &[(&'static str, u32)]) -> Scan {
        Scan { inspected: true, features: features.iter().copied().collect() }
    }
    fn codes(s: &Scan, k: Kind, t: Target) -> Vec<String> {
        warnings(s, k, t).into_iter().map(|w| w.code).collect()
    }

    #[test]
    fn the_table_is_well_formed() {
        for r in rows() {
            assert!(!r.evidence.trim().is_empty(), "{} has no evidence", r.code);
            for f in &r.from {
                assert!(Kind::ALL.iter().any(|k| k.ext() == f), "{}: unknown kind {f}", r.code);
            }
            for x in &r.to {
                assert!(Target::parse(x).is_some(), "{}: unknown target {x}", r.code);
            }
            assert_ne!(text(&r.code), r.code, "{} has no catalog text", r.code);
        }
        // the en-US rows go away with the NumLocale patch and say so
        for c in ["csv_one_sheet_en_us", "pdf_en_us"] {
            assert!(rows().iter().find(|r| r.code == c).unwrap().until.as_deref().unwrap().contains("NumLocale"));
        }
    }

    #[test]
    fn rows_fire_by_kind_target_and_feature() {
        let none = with(&[]);
        let blind = Scan::default();
        assert_eq!(codes(&blind, Kind::Doc, Target::Docx), ["doc_import_loss"]);
        assert_eq!(codes(&blind, Kind::Xls, Target::Pdf), ["xls_pdf_chart_page", "pdf_en_us"]);
        assert_eq!(codes(&none, Kind::Xlsx, Target::Csv), ["csv_one_sheet_en_us"]);
        assert_eq!(codes(&blind, Kind::Csv, Target::Csv), Vec::<String>::new());
        // feature-gated rows: quiet on a plain docx, loud when the feature is there, cautious when the input is opaque
        assert!(codes(&none, Kind::Docx, Target::Md).is_empty());
        assert_eq!(codes(&with(&[("images", 2), ("headers_footers", 1)]), Kind::Docx, Target::Txt), ["plain_text_layout"]);
        assert_eq!(warnings(&with(&[("images", 2), ("headers_footers", 1)]), Kind::Docx, Target::Txt)[0].count, 3);
        // an opaque rtf (the scanner could not open it) gets every row that could apply; a scanned one only the rows it has features for
        assert_eq!(codes(&blind, Kind::Rtf, Target::Md), ["plain_text_layout", "comments_dropped", "footnotes_dropped", "merged_cells_flattened"]);
        assert!(codes(&none, Kind::Rtf, Target::Md).is_empty());
        assert!(codes(&with(&[("images", 2)]), Kind::Docx, Target::Pdf).is_empty());
        assert_eq!(codes(&with(&[("macros", 1)]), Kind::Xlsx, Target::Xlsx), ["macros_dropped"]);
        // a plain letter to pdf: nothing
        assert!(codes(&with(&[("images", 1), ("headers_footers", 2)]), Kind::Docx, Target::Pdf).is_empty());
        // md and txt inputs do not "lose" layout going to md/txt
        assert!(codes(&blind, Kind::Md, Target::Txt).is_empty());
    }

    /// The WP9 rows: each fires for the measured kind and target when the scanner found the feature, and stays quiet without it,
    /// for another target, and (the `cross` rows) for the input's own format.
    #[test]
    fn the_wp9_rows_fire_only_for_their_feature_kind_and_target() {
        let none = with(&[]);
        let has = |f: &'static str| with(&[(f, 1)]);
        // (code, feature, kinds that warn, targets that warn, a target that must stay quiet)
        type Case<'a> = (&'a str, &'static str, &'a [Kind], &'a [Target], Target);
        let rows: [Case; 9] = [
            ("comments_dropped", "comments", &[Kind::Docx], &[Target::Odt, Target::Rtf, Target::Html, Target::Md, Target::Txt], Target::Pdf),
            ("footnotes_dropped", "footnotes", &[Kind::Docx], &[Target::Odt, Target::Rtf, Target::Html, Target::Md, Target::Txt], Target::Pdf),
            ("headers_footers_dropped", "headers_footers", &[Kind::Docx], &[Target::Odt, Target::Rtf, Target::Html], Target::Pdf),
            ("revisions_dropped", "revisions", &[Kind::Docx], &[Target::Html, Target::Md, Target::Txt], Target::Odt),
            ("merged_cells_flattened", "merged", &[Kind::Docx, Kind::Odt, Kind::Rtf, Kind::Html], &[Target::Md, Target::Txt], Target::Html),
            ("tables_to_text", "tables", &[Kind::Docx, Kind::Odt, Kind::Rtf, Kind::Html], &[Target::Txt], Target::Md),
            ("html_images_not_embedded", "linked_images", &[Kind::Html], &[Target::Docx, Target::Odt, Target::Rtf, Target::Pdf, Target::Png], Target::Md),
            ("comments_dropped", "comments", &[Kind::Odt, Kind::Rtf], &[Target::Docx, Target::Pdf, Target::Txt], Target::Odt),
            ("footnotes_dropped", "footnotes", &[Kind::Odt, Kind::Rtf], &[Target::Docx, Target::Pdf, Target::Txt], Target::Odt),
        ];
        for (code, feature, kinds, targets, quiet_target) in rows {
            for &k in kinds {
                for &t in targets {
                    assert!(codes(&has(feature), k, t).contains(&code.to_string()), "{code}: {} to {} should warn", k.ext(), t.ext());
                    assert!(!codes(&none, k, t).contains(&code.to_string()), "{code}: {} to {} warned without {feature}", k.ext(), t.ext());
                }
                let own = Target::parse(k.ext()).filter(|_| kinds.len() == 2 && code.ends_with("dropped"));
                if let Some(own) = own {
                    assert!(!codes(&has(feature), k, own).contains(&code.to_string()), "{code}: {} to itself", k.ext());
                }
                if kinds.len() == 1 {
                    assert!(!codes(&has(feature), k, quiet_target).contains(&code.to_string()), "{code}: {} to {} must stay quiet", k.ext(), quiet_target.ext());
                }
            }
        }
        // an input the scanner cannot open (opaque) gets the warning: not knowing is not "not there"
        assert!(codes(&Scan::default(), Kind::Docx, Target::Odt).contains(&"comments_dropped".to_string()));
        // a different feature does not trigger the row
        assert!(!codes(&has("images"), Kind::Docx, Target::Odt).contains(&"comments_dropped".to_string()));
        // docx keeps comments, notes and headers in docx and pdf; odt and rtf keep revisions
        for t in [Target::Docx, Target::Pdf] {
            assert!(codes(&with(&[("comments", 1), ("footnotes", 1), ("headers_footers", 1)]), Kind::Docx, t).is_empty());
        }
        assert!(codes(&has("revisions"), Kind::Docx, Target::Rtf).is_empty());
        // odt headers: lost going to docx, pdf, rtf, html (md and txt are plain_text_layout's)
        assert_eq!(codes(&has("headers_footers"), Kind::Odt, Target::Docx), ["headers_footers_dropped"]);
        assert_eq!(codes(&has("headers_footers"), Kind::Odt, Target::Txt), ["plain_text_layout"]);
        assert!(codes(&has("headers_footers"), Kind::Rtf, Target::Docx).is_empty());
    }
}
