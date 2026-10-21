//! The input scanner (PLAN 2.6 source 2, 5.5): reads the zip central directory only, never extracts and never parses XML.
//! It does two jobs from one pass. It rejects zip bombs and inconsistent archives (exit 9), and it counts the features that
//! the loss table (`data/losses.toml`) asks about. Most features are counted from part names. The few that live inside a part
//! (tables, tracked changes, footnote references, merged cells, ODF annotations and notes, the text of headers and footers) are
//! counted by a byte search over that one part while the scan inflates it anyway; a part over `PART_CAP` counts as "has it".
//! RTF and HTML (not zip packages) get the same byte search over the file. `shortcut: byte search, not an XML parse: a tag name in
//! a CDATA section or an attribute value is counted too, which errs on the side of a warning; upgrade if that shows up.`
use crate::exec::{Exit, Failure, fail};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub const MAX_TOTAL_BYTES: u64 = 1 << 30;
pub const MAX_ENTRIES: usize = 10_000;
pub const MAX_RATIO: u64 = 100;
/// The ratio check applies to entries above this size only (small text parts compress very well and are harmless). The floor is
/// small on purpose (WP7 review M1): with a 10 MB floor, 100 entries of 9.9 MB of zeros cost a megabyte of upload and a gigabyte of
/// inflate. Entries under the floor are bounded by the entry count and by the caller's inflate budget.
pub const RATIO_FROM_BYTES: u64 = 1 << 20;
/// The most bytes of one part (or of an RTF / HTML file) the feature search reads. Over it the features of that part count as present.
pub const PART_CAP: u64 = 64 << 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub size: u64,
    pub compressed: u64,
    /// where the entry's data starts, to catch overlapping or out-of-file entries
    pub start: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scan {
    /// false when the input is not a zip package (the loss table then treats every feature as possibly present)
    pub inspected: bool,
    pub features: BTreeMap<&'static str, u32>,
}

impl Scan {
    pub fn count(&self, feature: &str) -> u32 {
        self.features.get(feature).copied().unwrap_or(0)
    }
}

/// Which feature (if any) a package part name stands for.
fn feature_of(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    if n.ends_with('/') {
        return None;
    }
    let base = n.rsplit('/').next().unwrap_or("");
    Some(if n.contains("/charts/chart") && n.ends_with(".xml") {
        "charts"
    } else if n.contains("/diagrams/data") {
        "diagrams"
    } else if n.contains("/embeddings/") {
        "embeddings"
    } else if (base.starts_with("comments") && base.ends_with(".xml")) || n.starts_with("ppt/comments/") {
        "comments"
    } else if base == "vbaproject.bin" {
        "macros"
    } else if (n.starts_with("word/header") || n.starts_with("word/footer")) && n.ends_with(".xml") {
        "headers_footers"
    } else if n.contains("/media/") {
        "images"
    } else {
        return None;
    })
}

/// The pure core: limits first, then consistency, then the feature counts.
pub fn scan_entries(entries: &[Entry], file_len: u64) -> Result<Scan, String> {
    if entries.len() > MAX_ENTRIES {
        return Err(format!("the package holds {} parts, over the limit of {MAX_ENTRIES}", entries.len()));
    }
    let total: u64 = entries.iter().map(|e| e.size).sum();
    if total > MAX_TOTAL_BYTES {
        return Err(format!("the package expands to {} MB, over the limit of {} MB", total >> 20, MAX_TOTAL_BYTES >> 20));
    }
    for e in entries {
        if e.size > RATIO_FROM_BYTES && e.size / e.compressed.max(1) > MAX_RATIO {
            return Err(format!("part {} expands {}:1 (over {MAX_RATIO}:1), which looks like a zip bomb", e.name, e.size / e.compressed.max(1)));
        }
        if e.start.saturating_add(e.compressed) > file_len {
            return Err(format!("part {} lies beyond the end of the file (inconsistent central directory)", e.name));
        }
    }
    let mut spans: Vec<(u64, u64)> = entries.iter().filter(|e| e.compressed > 0).map(|e| (e.start, e.start + e.compressed)).collect();
    spans.sort_unstable();
    if spans.windows(2).any(|w| w[1].0 < w[0].1) {
        return Err("parts overlap in the file (inconsistent central directory)".into());
    }
    let mut scan = Scan { inspected: true, features: BTreeMap::new() };
    for e in entries {
        if let Some(f) = feature_of(&e.name) {
            *scan.features.entry(f).or_insert(0) += 1;
        }
    }
    Ok(scan)
}

// ---- the features that live inside a part ---------------------------------------------------------------------------------

/// Parts whose bytes `scan_with` keeps while it inflates them.
fn wants_content(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n == "word/document.xml" || n == "content.xml" || n == "styles.xml" || ((n.starts_with("word/header") || n.starts_with("word/footer")) && n.ends_with(".xml"))
}

/// How many times `needle` occurs in `hay` (non-overlapping).
fn count(hay: &[u8], needle: &[u8]) -> u32 {
    let (mut n, mut i) = (0, 0);
    while let Some(p) = hay[i..].windows(needle.len()).position(|w| w == needle) {
        n += 1;
        i += p + needle.len();
    }
    n
}

fn add(scan: &mut Scan, feature: &'static str, n: u32) {
    if n > 0 {
        *scan.features.entry(feature).or_insert(0) += n;
    }
}

/// `attr="N"` occurrences in `hay` with N above 1 (a colspan of 1 merges nothing).
fn span_gt1(hay: &[u8], attr: &[u8]) -> u32 {
    let (mut n, mut i) = (0, 0);
    while let Some(p) = hay[i..].windows(attr.len()).position(|w| w == attr) {
        i += p + attr.len();
        let digits: Vec<u8> = hay[i..].iter().copied().skip_while(|b| *b == b'"' || *b == b'\'').take_while(u8::is_ascii_digit).collect();
        if std::str::from_utf8(&digits).ok().and_then(|d| d.parse::<u32>().ok()).is_some_and(|v| v > 1) {
            n += 1;
        }
    }
    n
}

/// A header or footer part that holds something to show (text, a field such as a page number, or a picture). Word writes empty
/// ones for the "first page" and "even page" variants, and a plain letter must not be warned about those.
fn xml_has_content(xml: &[u8]) -> bool {
    [&b"<w:t>"[..], b"<w:t ", b"<w:fldSimple", b"<w:instrText", b"<w:drawing", b"<w:pict"].iter().any(|n| count(xml, n) > 0)
}

/// Add the features found inside the kept parts. `kept` = (part name, bytes, over the cap).
fn content_features(scan: &mut Scan, kept: &[(String, Vec<u8>, bool)]) {
    let mut header_parts = 0;
    let mut saw_header_part = false;
    for (name, b, over) in kept {
        let n = name.to_ascii_lowercase();
        // over the cap: assume the part has every feature it could have
        let c = |needles: &[&[u8]]| if *over { 1 } else { needles.iter().map(|x| count(b, x)).sum() };
        match n.as_str() {
            "word/document.xml" => {
                add(scan, "tables", c(&[b"<w:tbl>"]));
                add(scan, "revisions", c(&[b"<w:ins ", b"<w:del "]));
                add(scan, "footnotes", c(&[b"<w:footnoteReference", b"<w:endnoteReference"]));
                add(scan, "merged", c(&[b"<w:gridSpan", b"<w:vMerge"]));
            }
            "content.xml" => {
                add(scan, "tables", c(&[b"<table:table "]));
                add(scan, "comments", c(&[b"<office:annotation ", b"<office:annotation>"]));
                add(scan, "footnotes", c(&[b"<text:note "]));
                add(scan, "revisions", c(&[b"<text:changed-region"]));
                add(scan, "merged", c(&[b"table:number-columns-spanned", b"table:number-rows-spanned"]));
                add(scan, "images", c(&[b"<draw:image"]));
            }
            "styles.xml" => add(
                scan,
                "headers_footers",
                c(&[b"<style:header>", b"<style:footer>", b"<style:header-left>", b"<style:footer-left>", b"<style:header-first>", b"<style:footer-first>"]),
            ),
            _ => {
                saw_header_part = true;
                header_parts += u32::from(*over || xml_has_content(b));
            }
        }
    }
    if saw_header_part {
        // replaces the name-based count (empty variants do not count)
        scan.features.remove("headers_footers");
        add(scan, "headers_footers", header_parts);
    }
}

/// The non-zip inputs the loss table asks about: RTF and HTML, by a byte search over the file.
fn scan_text(path: &Path) -> Scan {
    let mut bytes = Vec::new();
    let Ok(f) = File::open(path) else { return Scan::default() };
    if f.take(PART_CAP + 1).read_to_end(&mut bytes).is_err() || bytes.len() as u64 > PART_CAP {
        return Scan::default();
    }
    let head: Vec<u8> = bytes.iter().take(1024).map(u8::to_ascii_lowercase).collect();
    let head = String::from_utf8_lossy(&head);
    let head = head.trim_start_matches('\u{feff}').trim_start();
    let mut s = Scan { inspected: true, features: BTreeMap::new() };
    if bytes.starts_with(b"{\\rtf") {
        let word = |w: &str| rtf_words(&bytes, w);
        add(&mut s, "tables", word("trowd").min(1));
        add(&mut s, "comments", word("annotation"));
        add(&mut s, "footnotes", word("footnote"));
        add(&mut s, "revisions", word("revised") + word("deleted"));
        // Word writes every picture twice (`shppict` and `nonshppict`)
        add(&mut s, "images", if word("shppict") > 0 { word("shppict") } else { word("pict") });
        add(&mut s, "merged", word("clmgf") + word("clmrg") + word("clvmgf") + word("clvmrg"));
        add(&mut s, "headers_footers", rtf_headers_with_text(&bytes));
        s
    } else if head.starts_with("<!doctype html") || head.starts_with("<html") {
        let low = bytes.to_ascii_lowercase();
        add(&mut s, "tables", count(&low, b"<table"));
        add(&mut s, "images", count(&low, b"<img"));
        add(&mut s, "linked_images", html_linked_images(&low));
        add(&mut s, "merged", span_gt1(&low, b"colspan=") + span_gt1(&low, b"rowspan="));
        s
    } else {
        Scan::default()
    }
}

/// `<img` tags whose source is not an inline `data:` URI: the picture lives in another file that a one-file conversion never sees.
fn html_linked_images(low: &[u8]) -> u32 {
    let (mut n, mut i) = (0, 0);
    while let Some(p) = low[i..].windows(4).position(|w| w == b"<img") {
        let start = i + p;
        let end = low[start..].iter().position(|b| *b == b'>').map_or(low.len(), |e| start + e);
        let tag = &low[start..end];
        if count(tag, b"src=\"data:") + count(tag, b"src='data:") + count(tag, b"src=data:") == 0 {
            n += 1;
        }
        i = end.max(start + 4);
    }
    n
}

/// Occurrences of the RTF control word `\word` (not followed by another letter).
fn rtf_words(b: &[u8], word: &str) -> u32 {
    let pat = format!("\\{word}");
    let (mut n, mut i) = (0, 0);
    while let Some(p) = b[i..].windows(pat.len()).position(|w| w == pat.as_bytes()) {
        i += p + pat.len();
        if !b.get(i).is_some_and(u8::is_ascii_alphabetic) {
            n += 1;
        }
    }
    n
}

/// The header and footer groups (`{\header`, `{\headerl`, `{\footerf`, ...) that hold text. An empty first-page variant is not one.
fn rtf_headers_with_text(b: &[u8]) -> u32 {
    let mut n = 0;
    for kind in [&b"{\\header"[..], b"{\\footer"] {
        let mut i = 0;
        while let Some(p) = b[i..].windows(kind.len()).position(|w| w == kind) {
            let start = i + p;
            i = start + kind.len();
            if rtf_group_has_text(&b[start..]) {
                n += 1;
            }
        }
    }
    n
}

/// Does the group that opens at `b[0]` contain visible characters (outside control words and `{\*...}` destinations)?
fn rtf_group_has_text(b: &[u8]) -> bool {
    let (mut depth, mut i, mut skip_to) = (0i32, 0, None::<i32>);
    while i < b.len() {
        match b[i] {
            b'{' => {
                depth += 1;
                if skip_to.is_none() && b[i..].starts_with(b"{\\*") {
                    skip_to = Some(depth - 1);
                }
            }
            b'}' => {
                depth -= 1;
                if skip_to == Some(depth) {
                    skip_to = None;
                }
                if depth == 0 {
                    return false;
                }
            }
            b'\\' => {
                i += 1;
                match b.get(i) {
                    Some(b'\'') if skip_to.is_none() => return true,
                    Some(c) if c.is_ascii_alphabetic() => {
                        let start = i;
                        while b.get(i).is_some_and(u8::is_ascii_alphabetic) {
                            i += 1;
                        }
                        if skip_to.is_none() && &b[start..i] == b"u" && b.get(i).is_some_and(|d| d.is_ascii_digit() || *d == b'-') {
                            return true;
                        }
                        while b.get(i).is_some_and(|d| d.is_ascii_digit() || *d == b'-') {
                            i += 1;
                        }
                        continue;
                    }
                    _ => {}
                }
            }
            c if skip_to.is_none() && c.is_ascii_alphanumeric() => return true,
            _ => {}
        }
        i += 1;
    }
    false
}

/// A scan result bound to the content it was made from: the API scans an upload once at `POST` and hands the verdict to the job,
/// which re-hashes its staged copy and skips the second inflate when the hash is the same (WP7 review M1).
#[derive(Debug, Clone)]
pub struct Verdict {
    pub sha256: String,
    pub scan: Scan,
}

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Scan a file by content: anything that is not a zip package comes back as `inspected: false`.
pub fn scan(path: &Path) -> Result<Scan, Failure> {
    let mut unlimited = u64::MAX;
    scan_with(path, &mut unlimited)
}

/// `scan` that charges every measured inflated byte to `budget` (the API gives one budget to a whole upload). A file that would
/// overdraw it is rejected like a bomb, and so is every later file of the same upload.
pub fn scan_with(path: &Path, budget: &mut u64) -> Result<Scan, Failure> {
    let reject = |m: String| fail(Exit::Rejected, m);
    let Ok(mut f) = File::open(path) else { return Ok(Scan::default()) };
    let mut magic = [0u8; 4];
    if f.read_exact(&mut magic).is_err() || &magic != b"PK\x03\x04" {
        return Ok(scan_text(path));
    }
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let mut z = match zip::ZipArchive::new(f) {
        Ok(z) => z,
        Err(e) => return reject(format!("the zip directory is unreadable or inconsistent: {e}")),
    };
    if z.len() > MAX_ENTRIES {
        return reject(format!("the package holds {} parts, over the limit of {MAX_ENTRIES}", z.len()));
    }
    let mut entries = Vec::with_capacity(z.len());
    for i in 0..z.len() {
        match z.by_index_raw(i) {
            Ok(e) => entries.push(Entry { name: e.name().map(|n| n.into_owned()).unwrap_or_default(), size: e.size(), compressed: e.compressed_size(), start: e.data_start().unwrap_or(u64::MAX) }),
            Err(e) => return reject(format!("part {i} of the zip directory is inconsistent: {e}")),
        }
    }
    // What the headers declare is checked first and costs nothing: a bomb that declares its size is refused before one byte is inflated.
    scan_entries(&entries, len).or_else(reject)?;
    // The headers are attacker-controlled, so the sizes that count are the ones measured while inflating (fail closed:
    // a part that cannot be inflated, fails its CRC, or outgrows its declared size rejects the file).
    let mut used = 0u64;
    let mut buf = vec![0u8; 64 * 1024];
    let mut kept: Vec<(String, Vec<u8>, bool)> = vec![];
    for (i, e) in entries.iter_mut().enumerate() {
        if e.name.ends_with('/') {
            continue;
        }
        let mut r = match z.by_index(i) {
            Ok(r) => r,
            Err(err) => return reject(format!("part {} cannot be opened: {err}", e.name)),
        };
        let mut n = 0u64;
        let mut part = wants_content(&e.name).then(|| (e.name.clone(), Vec::new(), false));
        loop {
            let got = match r.read(&mut buf) {
                Ok(0) => break,
                Ok(g) => g as u64,
                Err(err) => return reject(format!("part {} is corrupt or its sizes lie: {err}", e.name)),
            };
            n += got;
            if n > e.size || used + n > MAX_TOTAL_BYTES {
                return reject(format!("part {} inflates beyond its declared size or the {} MB limit", e.name, MAX_TOTAL_BYTES >> 20));
            }
            if *budget < got {
                *budget = 0;
                return reject("the upload expands more than this server allows for one request (inflate budget used up)".into());
            }
            *budget -= got;
            if let Some((_, bytes, over)) = part.as_mut() {
                if n <= PART_CAP {
                    bytes.extend_from_slice(&buf[..got as usize]);
                } else {
                    *over = true;
                }
            }
        }
        used += n;
        e.size = n;
        kept.extend(part);
    }
    let mut scan = scan_entries(&entries, len).or_else(reject)?;
    content_features(&mut scan, &kept);
    Ok(scan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn e(name: &str, size: u64, compressed: u64, start: u64) -> Entry {
        Entry { name: name.into(), size, compressed, start }
    }

    #[test]
    fn limits_are_exit_9_material() {
        let ok = [e("word/document.xml", 1000, 300, 0)];
        assert!(scan_entries(&ok, 1000).is_ok());
        assert!(scan_entries(&[e("a", 600 << 20, 100 << 20, 0), e("b", 600 << 20, 100 << 20, 100 << 20)], 300 << 20).unwrap_err().contains("expands"));
        assert!(scan_entries(&[e("big.xml", 20 << 20, 100_000, 0)], 100_000).unwrap_err().contains("zip bomb"));
        // a small part with a huge ratio is harmless, one over the floor is not
        assert!(scan_entries(&[e("small.xml", 512 << 10, 100, 0)], 100).is_ok());
        assert!(scan_entries(&[e("mid.xml", 5 << 20, 100, 0)], 100).unwrap_err().contains("zip bomb"));
        let many: Vec<Entry> = (0..=MAX_ENTRIES as u64).map(|i| e(&format!("p{i}"), 1, 1, i)).collect();
        assert!(scan_entries(&many, 20_000).unwrap_err().contains("parts"));
        assert!(scan_entries(&[e("a", 10, 10, 0), e("b", 10, 10, 5)], 100).unwrap_err().contains("overlap"));
        assert!(scan_entries(&[e("a", 10, 10, 95)], 100).unwrap_err().contains("beyond"));
    }

    #[test]
    fn features_are_counted_by_part_name() {
        let names = [
            "word/document.xml", "word/comments.xml", "xl/comments1.xml", "ppt/comments/comment1.xml", "xl/charts/chart1.xml",
            "word/diagrams/data1.xml", "word/embeddings/oleObject1.bin", "word/vbaProject.bin", "word/header1.xml",
            "word/footer2.xml", "word/media/image1.png", "word/media/", "word/_rels/document.xml.rels",
        ];
        let v: Vec<Entry> = names.iter().enumerate().map(|(i, n)| e(n, 10, 10, i as u64 * 10)).collect();
        let s = scan_entries(&v, 1000).unwrap();
        let c = |f| s.count(f);
        assert_eq!((c("comments"), c("charts"), c("diagrams"), c("embeddings"), c("macros"), c("headers_footers"), c("images")), (3, 1, 1, 1, 1, 2, 1));
    }

    fn zip_with(path: &Path, parts: &[(&str, &str)]) {
        let mut w = zip::ZipWriter::new(File::create(path).unwrap());
        for (n, body) in parts {
            w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(body.as_bytes()).unwrap();
        }
        w.finish().unwrap();
    }

    fn feats(s: &Scan) -> Vec<(&'static str, u32)> {
        s.features.iter().map(|(k, v)| (*k, *v)).collect()
    }

    #[test]
    fn features_inside_a_docx_are_counted_and_empty_headers_are_not_headers() {
        let dir = std::env::temp_dir().join(format!("convert-scan-docx-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let plain = dir.join("plain.docx");
        zip_with(
            &plain,
            &[
                ("word/document.xml", "<w:document><w:p/></w:document>"),
                ("word/header1.xml", "<w:hdr><w:p/></w:hdr>"),
                ("word/footer1.xml", "<w:ftr><w:p/></w:ftr>"),
            ],
        );
        // Word's empty first-page and even-page parts are not a header
        assert!(feats(&scan(&plain).unwrap()).is_empty(), "{:?}", scan(&plain).unwrap());
        let rich = dir.join("rich.docx");
        zip_with(
            &rich,
            &[
                (
                    "word/document.xml",
                    "<w:document><w:tbl><w:tr><w:tc><w:tcPr><w:gridSpan w:val=\"2\"/></w:tcPr></w:tc></w:tr></w:tbl><w:tbl>x</w:tbl>\
                     <w:ins w:id=\"1\"><w:r/></w:ins><w:footnoteReference w:id=\"2\"/></w:document>",
                ),
                ("word/header2.xml", "<w:hdr><w:p><w:r><w:t>HRM</w:t></w:r></w:p></w:hdr>"),
                ("word/footer2.xml", "<w:ftr><w:p><w:r><w:instrText>PAGE</w:instrText></w:r></w:p></w:ftr>"),
                ("word/header1.xml", "<w:hdr><w:p/></w:hdr>"),
            ],
        );
        let s = scan(&rich).unwrap();
        assert_eq!((s.count("tables"), s.count("revisions"), s.count("footnotes"), s.count("merged"), s.count("headers_footers")), (2, 1, 1, 1, 2));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn features_inside_an_odt_are_counted() {
        let dir = std::env::temp_dir().join(format!("convert-scan-odt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let odt = dir.join("a.odt");
        zip_with(
            &odt,
            &[
                (
                    "content.xml",
                    "<office:text><table:table table:name=\"a\"><table:table-cell table:number-columns-spanned=\"2\"/></table:table>\
                     <office:annotation office:name=\"c\"/><office:annotation-end/><text:note text:id=\"n\"/><text:changed-region/></office:text>",
                ),
                ("styles.xml", "<office:styles><style:header-style/><style:header><text:p/></style:header><style:footer-left/></office:styles>"),
            ],
        );
        let s = scan(&odt).unwrap();
        // `<style:header-style>` is a page-layout part, not a header; `<office:annotation-end>` is not a second comment
        assert_eq!(feats(&s), [("comments", 1), ("footnotes", 1), ("headers_footers", 1), ("merged", 1), ("revisions", 1), ("tables", 1)]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn features_inside_rtf_and_html_files_are_counted() {
        let dir = std::env::temp_dir().join(format!("convert-scan-text-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rtf = dir.join("a.rtf");
        std::fs::write(
            &rtf,
            b"{\\rtf1\\ansi{\\header\\pard HRM test\\par}{\\headerf\\pard\\plain\\par}{\\footer{\\*\\fldinst PAGE}\\par}\\trowd\\cellx100\\row\\trowd\\clmgf\\cellx100\\row\n\
              {\\*\\annotation c}\\footnote x{\\shppict{\\pict a}}{\\nonshppict{\\pict a}}\\par}",
        )
        .unwrap();
        let s = scan(&rtf).unwrap();
        // one header with text; the empty first-page header and the footer that holds only a field do not count
        assert_eq!(feats(&s), [("comments", 1), ("footnotes", 1), ("headers_footers", 1), ("images", 1), ("merged", 1), ("tables", 1)]);
        let html = dir.join("a.html");
        std::fs::write(
            &html,
            "<!DOCTYPE html><table><tr><td colspan=\"2\">a</td><td rowspan=\"1\">b</td></tr></table>\n<img\nsrc=\"order_files/a.png\"><img src='data:image/png;base64,AA'>",
        )
        .unwrap();
        let s = scan(&html).unwrap();
        assert_eq!(feats(&s), [("images", 2), ("linked_images", 1), ("merged", 1), ("tables", 1)]);
        // a plain text file and an html fragment stay opaque
        let txt = dir.join("a.txt");
        std::fs::write(&txt, "<table><tr>").unwrap();
        assert!(!scan(&txt).unwrap().inspected);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_real_zip_and_a_broken_one() {
        let dir = std::env::temp_dir().join(format!("convert-scan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let good = dir.join("good.docx");
        let mut w = zip::ZipWriter::new(File::create(&good).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        for n in ["word/document.xml", "word/comments.xml"] {
            w.start_file(n, o).unwrap();
            w.write_all(b"<x/>").unwrap();
        }
        w.finish().unwrap();
        let s = scan(&good).unwrap();
        assert!(s.inspected);
        assert_eq!(s.count("comments"), 1);
        let bad = dir.join("bad.docx");
        std::fs::write(&bad, b"PK\x03\x04\x14\x00\x00\x00\x08\x00partial").unwrap();
        assert_eq!(scan(&bad).unwrap_err().exit, Exit::Rejected);
        // a bomb, honest (11 MB of zeros in a few KB) and lying (the central directory claims 1000 bytes)
        let bomb = dir.join("bomb.docx");
        let mut w = zip::ZipWriter::new(File::create(&bomb).unwrap());
        w.start_file("word/document.xml", zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(&vec![0u8; 11 << 20]).unwrap();
        w.finish().unwrap();
        assert!(scan(&bomb).unwrap_err().msg.contains("zip bomb"));
        let mut bytes = std::fs::read(&bomb).unwrap();
        let cd = bytes.windows(4).rposition(|w| w == b"PK").unwrap();
        bytes[cd + 24..cd + 28].copy_from_slice(&1000u32.to_le_bytes());
        let lie = dir.join("lie.docx");
        std::fs::write(&lie, &bytes).unwrap();
        assert_eq!(scan(&lie).unwrap_err().exit, Exit::Rejected);
        let txt = dir.join("a.txt");
        std::fs::write(&txt, b"hello").unwrap();
        assert!(!scan(&txt).unwrap().inspected);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
