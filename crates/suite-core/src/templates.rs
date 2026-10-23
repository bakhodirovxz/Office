//! The starter documents (`web/shell/templates.json` and the files in `web/shell/templates/`). Synthetic, original text written for
//! this project in Uzbek: no corpus content, no real names, numbers or organisations; every place a user fills in is a `[bracket]`.
//!
//! The catalogue is read by the web shell ("New from template") and by the desktop host. `scripts/gen-templates.py` makes the
//! files, so they are reproducible byte for byte.

use serde_json::Value;
use std::collections::BTreeMap;

/// The catalogue as shipped, embedded so the host needs no file.
pub const CATALOG_JSON: &str = include_str!("../../../web/shell/templates.json");

/// The UI languages every template name must have.
pub const NAME_LANGS: [&str; 4] = ["en", "ru", "uz-Latn", "uz-Cyrl"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    /// `docs`, `sheets` or `slides`
    pub app: String,
    /// relative to `web/shell/`, e.g. `templates/buyruq.docx`
    pub file: String,
    /// `draft:agent-<model>:<date>` or `reviewed:agent-<model>:<date>` (CLAUDE.md: the Uzbek review is the agent's, never the owner's)
    pub status: String,
    pub name: BTreeMap<String, String>,
}

fn ext_of(app: &str) -> Option<&'static str> {
    match app {
        "docs" => Some("docx"),
        "sheets" => Some("xlsx"),
        "slides" => Some("pptx"),
        _ => None,
    }
}

/// Parses and checks a catalogue: a JSON array of `{app, file, name:{en,ru,uz-Latn,uz-Cyrl}}`. The file must be a plain name under
/// `templates/` with the extension of its app. The first fault is the error.
pub fn parse(json: &str) -> Result<Vec<Template>, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let items = v.as_array().ok_or("not an array")?;
    let mut out: Vec<Template> = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let text = |k: &str| {
            it.get(k)
                .and_then(Value::as_str)
                .ok_or(format!("item {i}: `{k}` missing or not text"))
        };
        let (app, file) = (text("app")?, text("file")?);
        let ext = ext_of(app).ok_or(format!("item {i}: unknown app {app:?}"))?;
        let leaf = file.strip_prefix("templates/").unwrap_or("");
        let plain = !leaf.is_empty()
            && !leaf.starts_with('.')
            && leaf.ends_with(&format!(".{ext}"))
            && leaf
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        if !plain {
            return Err(format!("item {i}: bad file {file:?} for {app}"));
        }
        if out.iter().any(|t| t.file == file) {
            return Err(format!("item {i}: {file} listed twice"));
        }
        let status = text("status")?;
        if !(status.starts_with("draft:agent-") || status.starts_with("reviewed:agent-")) {
            return Err(format!("item {i}: bad status {status:?}"));
        }
        let names = it
            .get("name")
            .and_then(Value::as_object)
            .ok_or(format!("item {i}: `name` missing"))?;
        let mut name = BTreeMap::new();
        for l in NAME_LANGS {
            match names.get(l).and_then(Value::as_str) {
                Some(n) if !n.trim().is_empty() => {
                    name.insert(l.to_string(), n.to_string());
                }
                _ => return Err(format!("item {i}: no {l} name")),
            }
        }
        out.push(Template {
            app: app.into(),
            file: file.into(),
            status: status.into(),
            name,
        });
    }
    Ok(out)
}

/// The shipped catalogue.
pub fn catalog() -> Vec<Template> {
    parse(CATALOG_JSON).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::path::Path;

    fn shell() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/shell")
    }

    #[test]
    fn the_shipped_catalogue_is_valid_and_lists_the_five_starters() {
        let c = parse(CATALOG_JSON).unwrap();
        let files: Vec<&str> = c.iter().map(|t| t.file.as_str()).collect();
        assert_eq!(
            files,
            [
                "templates/buyruq.docx",
                "templates/xizmat-xati.docx",
                "templates/tabel.xlsx",
                "templates/xodimlar-royxati.xlsx",
                "templates/taqdimot.pptx"
            ]
        );
        assert_eq!(c, catalog());
    }

    #[test]
    fn the_checks_refuse_bad_catalogues() {
        let ok = r#"{"app":"docs","file":"templates/a.docx","status":"draft:agent-x:2026-01-01","name":{"en":"A","ru":"B","uz-Latn":"C","uz-Cyrl":"D"}}"#;
        assert_eq!(parse(&format!("[{ok}]")).unwrap().len(), 1);
        for bad in [
            ok.replace("docs", "pdf"),
            ok.replace("templates/a.docx", "../a.docx"),
            ok.replace("templates/a.docx", "templates/a.xlsx"),
            ok.replace("templates/a.docx", "templates/x/a.docx"),
            ok.replace("templates/a.docx", "templates\\a.docx"),
            ok.replace("templates/a.docx", "templates/.docx"),
            ok.replace("templates/a.docx", "C:/a.docx"),
            ok.replace("\"ru\":\"B\",", ""),
            ok.replace("\"C\"", "\" \""),
            ok.replace("draft:agent-x:2026-01-01", "reviewed by the owner"),
            ok.replace("\"status\":\"draft:agent-x:2026-01-01\",", ""),
        ] {
            assert!(parse(&format!("[{bad}]")).is_err(), "{bad}");
        }
        assert!(parse("{}").is_err() && parse("[1]").is_err());
        assert!(parse(&format!("[{ok},{ok}]")).is_err(), "twice");
    }

    fn part(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> String {
        let mut s = String::new();
        zip.by_name(name)
            .unwrap_or_else(|_| panic!("missing part {name}"))
            .read_to_string(&mut s)
            .unwrap();
        s
    }

    #[test]
    fn every_template_is_a_well_formed_package_with_its_main_part() {
        for t in parse(CATALOG_JSON).unwrap() {
            let path = shell().join(&t.file);
            let mut z = zip::ZipArchive::new(
                std::fs::File::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
            )
            .unwrap();
            let main = match t.app.as_str() {
                "docs" => "word/document.xml",
                "sheets" => "xl/workbook.xml",
                _ => "ppt/presentation.xml",
            };
            for i in 0..z.len() {
                let name = z.by_index(i).unwrap().name().unwrap().to_string();
                if name.ends_with(".xml") || name.ends_with(".rels") {
                    let xml = part(&mut z, &name);
                    roxmltree::Document::parse(&xml)
                        .unwrap_or_else(|e| panic!("{} {name}: {e}", t.file));
                }
            }
            assert!(
                part(&mut z, "[Content_Types].xml").contains(&format!("/{main}")),
                "{}: content type of {main}",
                t.file
            );
            assert!(
                part(&mut z, "_rels/.rels").contains(main),
                "{}: the root relationship points at {main}",
                t.file
            );
        }
    }

    #[test]
    fn the_templates_hold_uzbek_text_with_bracketed_blanks_and_nothing_personal() {
        let words = [
            "Tashkilot",
            "BUYRUQ",
            "Xizmat",
            "Lavozim",
            "Taqdimot",
            "F.I.Sh",
            "TABEL",
        ];
        for t in parse(CATALOG_JSON).unwrap() {
            let mut z =
                zip::ZipArchive::new(std::fs::File::open(shell().join(&t.file)).unwrap()).unwrap();
            let mut text = String::new();
            for i in 0..z.len() {
                let mut f = z.by_index(i).unwrap();
                if f.name().unwrap().ends_with(".xml") {
                    let mut xml = String::new();
                    f.read_to_string(&mut xml).unwrap();
                    // the visible text only: ids and sizes in the markup are long numbers too
                    let doc = roxmltree::Document::parse(&xml).unwrap();
                    for t in doc
                        .descendants()
                        .filter(|n| n.is_text())
                        .filter_map(|n| n.text())
                    {
                        text.push_str(t);
                        text.push(' '); // cell values sit side by side: they are not one number
                    }
                    text.push('\n');
                }
            }
            assert!(text.contains('['), "{}: blanks are written as [..]", t.file);
            assert!(
                words.iter().any(|w| text.contains(w)),
                "{}: Uzbek text",
                t.file
            );
            // no e-mail address, and no run of nine digits (a phone or a personal id number)
            assert!(!text.contains('@'), "{}", t.file);
            let (mut run, mut longest) = (0, 0);
            for c in text.chars() {
                run = if c.is_ascii_digit() { run + 1 } else { 0 };
                longest = longest.max(run);
            }
            assert!(longest < 9, "{}: a run of {longest} digits", t.file);
        }
    }
}
