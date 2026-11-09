//! The checks on every document that enters the server, fetched or saved (PLAN phase 5, S9): the size cap, the magic bytes, the zip
//! central directory against zip bombs (the converter's own limits, `convert::scan`), and that the package's main part is the one
//! `fileType` names, so a renamed spreadsheet is not stored as a text document. Nothing is inflated and no XML is parsed here.
use convert::scan::{Entry, scan_entries};
use std::io::Cursor;

#[derive(Debug, PartialEq, Eq)]
pub enum DocErr {
    TooLarge,
    Bad(String),
}

/// The part that must exist for each package type.
fn main_part(file_type: &str) -> Option<&'static str> {
    match file_type {
        "docx" => Some("word/document.xml"),
        "xlsx" => Some("xl/workbook.xml"),
        "pptx" => Some("ppt/presentation.xml"),
        _ => None,
    }
}

pub fn check(bytes: &[u8], file_type: &str, max_bytes: u64) -> Result<(), DocErr> {
    let bad = |m: &str| Err(DocErr::Bad(m.into()));
    if bytes.len() as u64 > max_bytes {
        return Err(DocErr::TooLarge);
    }
    if file_type == "pdf" {
        return if bytes.starts_with(b"%PDF-") {
            Ok(())
        } else {
            bad("not a PDF document")
        };
    }
    let Some(main) = main_part(file_type) else {
        return bad("not a supported type");
    };
    if !bytes.starts_with(b"PK\x03\x04") {
        return bad("not an Office Open XML package");
    }
    let mut z = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| DocErr::Bad("the zip directory is unreadable or inconsistent".into()))?;
    if z.len() > convert::scan::MAX_ENTRIES {
        return bad("the package holds too many parts");
    }
    let mut entries = Vec::with_capacity(z.len());
    let (mut has_main, mut has_types) = (false, false);
    for i in 0..z.len() {
        let e = z
            .by_index_raw(i)
            .map_err(|_| DocErr::Bad("the zip directory is unreadable or inconsistent".into()))?;
        let name = e.name().map(|n| n.into_owned()).unwrap_or_default();
        has_main |= name == main;
        has_types |= name == "[Content_Types].xml";
        entries.push(Entry {
            name,
            size: e.size(),
            compressed: e.compressed_size(),
            start: e.data_start().unwrap_or(u64::MAX),
        });
    }
    scan_entries(&entries, bytes.len() as u64).map_err(DocErr::Bad)?;
    if !(has_main && has_types) {
        return bad("the package is not a document of this type");
    }
    Ok(())
}

/// Generator names that stand for "nobody": what the host's server-side blank-file makers write as `dc:creator`.
const GENERATORS: [&str; 3] = ["python-docx", "openpyxl", "python-pptx"];

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Sets `<tag>` to `name` in core.xml text when `replace(current)` says so, or adds it when absent. `None` = nothing to change.
/// shortcut: standard `dc:`/`cp:` prefixes and plain-text elements only (what every writer we meet emits), an exotic prefix is left alone.
fn set_prop(xml: &str, tag: &str, name: &str, replace: impl Fn(&str) -> bool) -> Option<String> {
    let new = format!("<{tag}>{}</{tag}>", esc(name));
    let open = format!("<{tag}");
    if let Some(a) = xml.find(&open) {
        let rest = &xml[a + open.len()..];
        // `<tag>text</tag>`, `<tag/>` or `<tag />`; a longer tag name sharing the prefix is not ours
        let (cur, end) = if let Some(r) = rest.strip_prefix('>') {
            let e = r.find(&format!("</{tag}>"))?;
            (&r[..e], a + open.len() + 1 + e + tag.len() + 3)
        } else if let Some(i) = rest.find("/>").filter(|i| rest[..*i].trim().is_empty()) {
            ("", a + open.len() + i + 2)
        } else {
            return None;
        };
        if !replace(cur.trim()) || cur == esc(name) {
            return None;
        }
        return Some(format!("{}{new}{}", &xml[..a], &xml[end..]));
    }
    let close = xml.rfind("</cp:coreProperties>")?;
    Some(format!("{}{new}{}", &xml[..close], &xml[close..]))
}

/// The person at the keyboard becomes the document's creator when `dc:creator` is empty or a generator name (python-docx and its
/// kin); a real name is kept. With `saved`, `cp:lastModifiedBy` is the person as well. Only `docProps/core.xml` changes: every other
/// entry is copied raw. `None` when nothing changes (so a second pass over the result is a no-op and its digest stays) or the part is
/// missing or odd. `name` is the host's text: trimmed, capped, escaped.
pub fn stamp_author(bytes: &[u8], file_type: &str, name: &str, saved: bool) -> Option<Vec<u8>> {
    let name: String = name.trim().chars().take(255).collect();
    if name.is_empty() || main_part(file_type).is_none() {
        return None;
    }
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
    let idx = z.index_for_name("docProps/core.xml")?;
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut std::io::Read::take(z.by_index(idx).ok()?, 1 << 20), &mut xml).ok()?;
    let mut out = xml.clone();
    let creator = |c: &str| c.is_empty() || GENERATORS.iter().any(|g| g.eq_ignore_ascii_case(c));
    if let Some(x) = set_prop(&out, "dc:creator", &name, creator) {
        out = x;
    }
    if saved && let Some(x) = set_prop(&out, "cp:lastModifiedBy", &name, |_| true) {
        out = x;
    }
    if out == xml {
        return None;
    }
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::with_capacity(bytes.len() + 256)));
    for i in 0..z.len() {
        if i == idx {
            let o = zip::write::SimpleFileOptions::default();
            w.start_file("docProps/core.xml", o).ok()?;
            std::io::Write::write_all(&mut w, out.as_bytes()).ok()?;
        } else {
            w.raw_copy_file(z.by_index_raw(i).ok()?).ok()?;
        }
    }
    Some(w.finish().ok()?.into_inner())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;

    /// A package with the given parts, each part `(name, bytes)`.
    pub fn package(parts: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(vec![]));
        for (n, b) in parts {
            w.start_file(*n, zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(b).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn a_main_part_of_the_right_type_and_nothing_else() {
        let docx = package(&[("[Content_Types].xml", b"x"), ("word/document.xml", b"x")]);
        assert_eq!(check(&docx, "docx", 1 << 20), Ok(()));
        assert!(matches!(check(&docx, "xlsx", 1 << 20), Err(DocErr::Bad(_))));
        assert_eq!(check(&docx, "docx", 10), Err(DocErr::TooLarge));
        let no_types = package(&[("word/document.xml", b"x")]);
        assert!(matches!(
            check(&no_types, "docx", 1 << 20),
            Err(DocErr::Bad(_))
        ));
        assert!(matches!(
            check(b"PK\x03\x04junk", "docx", 1 << 20),
            Err(DocErr::Bad(_))
        ));
        assert_eq!(check(b"%PDF-1.7 x", "pdf", 1 << 20), Ok(()));
        assert!(matches!(check(&docx, "pdf", 1 << 20), Err(DocErr::Bad(_))));
    }

    #[test]
    fn a_small_zip_that_inflates_far_is_refused() {
        let zeros = vec![0u8; 40 << 20];
        let bomb = package(&[
            ("[Content_Types].xml", b"x"),
            ("word/document.xml", b"x"),
            ("word/media/a.bin", &zeros),
        ]);
        assert!(bomb.len() < 1 << 20, "the bomb is {} bytes", bomb.len());
        match check(&bomb, "docx", 50 << 20) {
            Err(DocErr::Bad(m)) => assert!(m.contains("zip bomb"), "{m}"),
            r => panic!("{r:?}"),
        }
    }
}
