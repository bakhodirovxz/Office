//! Input kind detection: magic bytes first, the extension only as a tie-break (PLAN 2.2).
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Docx,
    Doc,
    Odt,
    Rtf,
    Html,
    Md,
    Txt,
    Xlsx,
    Xls,
    Xlsb,
    Ods,
    Csv,
    Tsv,
    Pptx,
    Ppt,
    Odp,
    Pdf,
}

/// Which domain CLI opens the kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Docs,
    Sheets,
    Slides,
    Pdf,
}

impl Kind {
    pub const ALL: [Kind; 17] = [
        Kind::Docx, Kind::Doc, Kind::Odt, Kind::Rtf, Kind::Html, Kind::Md, Kind::Txt, Kind::Xlsx, Kind::Xls,
        Kind::Xlsb, Kind::Ods, Kind::Csv, Kind::Tsv, Kind::Pptx, Kind::Ppt, Kind::Odp, Kind::Pdf,
    ];

    pub fn ext(self) -> &'static str {
        match self {
            Kind::Docx => "docx", Kind::Doc => "doc", Kind::Odt => "odt", Kind::Rtf => "rtf", Kind::Html => "html",
            Kind::Md => "md", Kind::Txt => "txt", Kind::Xlsx => "xlsx", Kind::Xls => "xls", Kind::Xlsb => "xlsb",
            Kind::Ods => "ods", Kind::Csv => "csv", Kind::Tsv => "tsv", Kind::Pptx => "pptx", Kind::Ppt => "ppt",
            Kind::Odp => "odp", Kind::Pdf => "pdf",
        }
    }

    pub fn family(self) -> Family {
        match self {
            Kind::Docx | Kind::Doc | Kind::Odt | Kind::Rtf | Kind::Html | Kind::Md | Kind::Txt => Family::Docs,
            Kind::Xlsx | Kind::Xls | Kind::Xlsb | Kind::Ods | Kind::Csv | Kind::Tsv => Family::Sheets,
            Kind::Pptx | Kind::Ppt | Kind::Odp => Family::Slides,
            Kind::Pdf => Family::Pdf,
        }
    }

    fn from_ext(ext: &str) -> Option<Kind> {
        let e = ext.to_ascii_lowercase();
        Some(match e.as_str() {
            "docx" | "docm" | "dotx" | "dotm" => Kind::Docx,
            "doc" | "dot" => Kind::Doc,
            "odt" => Kind::Odt,
            "rtf" => Kind::Rtf,
            "html" | "htm" => Kind::Html,
            "md" | "markdown" => Kind::Md,
            "txt" => Kind::Txt,
            "xlsx" | "xlsm" | "xltx" | "xltm" => Kind::Xlsx,
            "xls" | "xlt" => Kind::Xls,
            "xlsb" => Kind::Xlsb,
            "ods" => Kind::Ods,
            "csv" => Kind::Csv,
            "tsv" | "tab" => Kind::Tsv,
            "pptx" | "potx" | "ppsx" => Kind::Pptx,
            "ppt" | "pps" => Kind::Ppt,
            "odp" => Kind::Odp,
            "pdf" => Kind::Pdf,
            _ => return None,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Detected {
    pub kind: Kind,
    /// The extension names a different kind than the content (a warning, never an error).
    pub ext_mismatch: bool,
    /// An encrypted OOXML package (an OLE container holding `EncryptedPackage`): needs a password.
    pub password_protected: bool,
}

#[derive(Debug)]
pub enum DetectError {
    Io(String),
    Unknown,
}

const OLE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// Detect from the file on disk. `shortcut: an OLE container is scanned whole for its UTF-16 stream names instead of walking
/// the directory chain; upgrade when a file with a misleading name inside a data stream shows up.`
pub fn detect(path: &Path) -> Result<Detected, DetectError> {
    let mut f = File::open(path).map_err(|e| DetectError::Io(e.to_string()))?;
    let mut head = vec![0u8; 64 * 1024];
    let n = f.read(&mut head).map_err(|e| DetectError::Io(e.to_string()))?;
    head.truncate(n);
    let ext = path.extension().and_then(|e| e.to_str()).and_then(Kind::from_ext);
    let ole_bytes = if head.starts_with(&OLE) {
        let mut all = head.clone();
        f.read_to_end(&mut all).map_err(|e| DetectError::Io(e.to_string()))?;
        Some(all)
    } else {
        None
    };
    let zip_names = if head.starts_with(b"PK\x03\x04") { zip_names(path) } else { None };
    detect_bytes(&head, ole_bytes.as_deref(), zip_names.as_deref(), ext).ok_or(DetectError::Unknown)
}

fn zip_names(path: &Path) -> Option<Vec<String>> {
    let z = zip::ZipArchive::new(File::open(path).ok()?).ok()?;
    Some(z.file_names().filter_map(Result::ok).map(|n| n.into_owned()).collect())
}

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(|c| c.to_le_bytes()).collect()
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

/// Pure core, unit-tested. `head` = first 64 KB, `ole` = the whole file when it is an OLE container, `zip` = entry names.
pub fn detect_bytes(head: &[u8], ole: Option<&[u8]>, zip: Option<&[String]>, ext: Option<Kind>) -> Option<Detected> {
    let found = |kind: Kind| Detected { kind, ext_mismatch: ext.is_some_and(|e| e != kind), password_protected: false };
    if head.starts_with(b"%PDF-") {
        return Some(found(Kind::Pdf));
    }
    if let Some(all) = ole {
        if contains(all, &utf16("EncryptedPackage")) {
            // which app it was is unknowable without the password; the extension decides only the label
            let kind = ext.unwrap_or(Kind::Docx);
            return Some(Detected { kind, ext_mismatch: false, password_protected: true });
        }
        let kind = if contains(all, &utf16("WordDocument")) {
            Kind::Doc
        } else if contains(all, &utf16("PowerPoint Document")) {
            Kind::Ppt
        } else if contains(all, &utf16("Workbook")) || contains(all, &utf16("Book")) {
            Kind::Xls
        } else {
            return None;
        };
        return Some(found(kind));
    }
    if head.starts_with(b"PK\x03\x04") {
        let names = zip?;
        let has = |p: &str| names.iter().any(|n| n == p);
        let kind = if has("word/document.xml") {
            Kind::Docx
        } else if has("xl/workbook.bin") {
            Kind::Xlsb
        } else if has("xl/workbook.xml") {
            Kind::Xlsx
        } else if has("ppt/presentation.xml") {
            Kind::Pptx
        } else if has("mimetype") {
            // ODF: the mimetype entry is stored first and uncompressed, so it is readable in `head` at a fixed place
            let s = String::from_utf8_lossy(&head[..head.len().min(300)]);
            if s.contains("opendocument.text") {
                Kind::Odt
            } else if s.contains("opendocument.spreadsheet") {
                Kind::Ods
            } else if s.contains("opendocument.presentation") {
                Kind::Odp
            } else {
                return None;
            }
        } else {
            return None;
        };
        return Some(found(kind));
    }
    if head.starts_with(b"{\\rtf") {
        return Some(found(Kind::Rtf));
    }
    // text: a NUL byte or invalid UTF-8 (other than a cut at the end of the window) means binary
    let text = match std::str::from_utf8(head) {
        Ok(t) => t,
        Err(e) if e.error_len().is_none() => std::str::from_utf8(&head[..e.valid_up_to()]).ok()?,
        Err(_) => return None,
    };
    if text.contains('\0') {
        return None;
    }
    let lower = text.trim_start_matches('\u{feff}').trim_start().to_ascii_lowercase();
    let kind = if lower.starts_with("<!doctype html") || lower.starts_with("<html") {
        Kind::Html
    } else {
        match ext {
            Some(k @ (Kind::Csv | Kind::Tsv | Kind::Md | Kind::Html | Kind::Txt)) => k,
            _ => Kind::Txt,
        }
    };
    Some(found(kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(head: &[u8], ole: Option<&[u8]>, zip: Option<&[&str]>, ext: Option<Kind>) -> Option<Detected> {
        let z: Option<Vec<String>> = zip.map(|n| n.iter().map(|s| s.to_string()).collect());
        detect_bytes(head, ole, z.as_deref(), ext)
    }
    fn kind(head: &[u8], ole: Option<&[u8]>, zip: Option<&[&str]>, ext: Option<Kind>) -> Option<Kind> {
        d(head, ole, zip, ext).map(|x| x.kind)
    }
    const PK: &[u8] = b"PK\x03\x04rest";

    #[test]
    fn every_kind() {
        assert_eq!(kind(b"%PDF-1.7\n", None, None, None), Some(Kind::Pdf));
        assert_eq!(kind(PK, None, Some(&["[Content_Types].xml", "word/document.xml"]), None), Some(Kind::Docx));
        assert_eq!(kind(PK, None, Some(&["xl/workbook.xml"]), None), Some(Kind::Xlsx));
        assert_eq!(kind(PK, None, Some(&["xl/workbook.bin"]), None), Some(Kind::Xlsb));
        assert_eq!(kind(PK, None, Some(&["ppt/presentation.xml"]), None), Some(Kind::Pptx));
        let odf = |m: &str| [&b"PK\x03\x04"[..], b"....mimetype", m.as_bytes()].concat();
        assert_eq!(kind(&odf("application/vnd.oasis.opendocument.text"), None, Some(&["mimetype"]), None), Some(Kind::Odt));
        assert_eq!(kind(&odf("application/vnd.oasis.opendocument.spreadsheet"), None, Some(&["mimetype"]), None), Some(Kind::Ods));
        assert_eq!(kind(&odf("application/vnd.oasis.opendocument.presentation"), None, Some(&["mimetype"]), None), Some(Kind::Odp));
        assert_eq!(kind(b"{\\rtf1\\ansi", None, None, None), Some(Kind::Rtf));
        let ole = |s: &str| [&OLE[..], &utf16(s)].concat();
        assert_eq!(kind(&OLE, Some(&ole("WordDocument")), None, None), Some(Kind::Doc));
        assert_eq!(kind(&OLE, Some(&ole("Workbook")), None, None), Some(Kind::Xls));
        assert_eq!(kind(&OLE, Some(&ole("PowerPoint Document")), None, None), Some(Kind::Ppt));
        assert_eq!(kind(b"<!DOCTYPE html><p>x", None, None, None), Some(Kind::Html));
        assert_eq!(kind(b"  <html>", None, None, Some(Kind::Txt)), Some(Kind::Html));
        assert_eq!(kind(b"a,b\n1,2\n", None, None, Some(Kind::Csv)), Some(Kind::Csv));
        assert_eq!(kind(b"a\tb\n", None, None, Some(Kind::Tsv)), Some(Kind::Tsv));
        assert_eq!(kind(b"# Title\n", None, None, Some(Kind::Md)), Some(Kind::Md));
        assert_eq!(kind("Salom dunyo \u{43f}".as_bytes(), None, None, None), Some(Kind::Txt));
    }

    #[test]
    fn mislabelled_file_is_routed_by_content() {
        let r = d(PK, None, Some(&["xl/workbook.xml"]), Some(Kind::Docx)).unwrap();
        assert_eq!((r.kind, r.ext_mismatch), (Kind::Xlsx, true));
        let r = d(b"%PDF-1.4", None, None, Some(Kind::Docx)).unwrap();
        assert_eq!((r.kind, r.ext_mismatch), (Kind::Pdf, true));
        assert!(!d(PK, None, Some(&["word/document.xml"]), Some(Kind::Docx)).unwrap().ext_mismatch);
    }

    #[test]
    fn encrypted_package_and_garbage() {
        let enc = [&OLE[..], &utf16("EncryptedPackage")].concat();
        assert!(d(&OLE, Some(&enc), None, Some(Kind::Xlsx)).unwrap().password_protected);
        assert_eq!(kind(&[0, 1, 2, 0xff, 0xfe], None, None, None), None);
        assert_eq!(kind(PK, None, Some(&["a.txt"]), None), None);
        assert_eq!(kind(&OLE, Some(&OLE), None, None), None);
    }
}
