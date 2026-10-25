//! XDFU ("Xizmat doirasida foydalanish uchun") documents (PLAN 10, WP15). Opt-in and OFF by default: with `Mode::Off` no hook is
//! installed, so nothing is detected, nothing is logged and the output is byte for byte what it was before.
//!
//! With `Watermark` or `Refuse` the hook sees every job twice (`hook.rs`):
//!
//! 0. A kind that cannot be read directly (no detected kind: the PDF operations) is checked on the tool's OUTPUT instead: `before`
//!    only notes that the check is deferred, and `after` looks for the grif in the outputs (every output kind we watermark except `png` is
//!    scannable) before they are watermarked. If neither the input nor the output can be scanned, the job is refused (exit 3,
//!    `xdfu_cannot_check`): not knowing is not the same as "not marked". An image-only PDF (no text on page 1) is the same case:
//!    refused (`xdfu_no_text_layer`). `odt` and `ods` are read natively (content, styles with the headers, meta); `doc` and `rtf` are
//!    probed through a conversion to docx, `xls` and `xlsb` through one to xlsx, in `mid/`, so that their headers are looked at too.
//! 1. `before`: look for the grif ("XDFU", "ХДФУ", "ДСП", "Для служебного пользования", and the Uzbek spellings of the phrase) in the
//!    places the Rules name: the upper part of page 1, a header, the document properties. Nothing found: the job is an ordinary job
//!    and the hook forgets it. Found: `Refuse` ends the job with exit 9; `Watermark` checks that the target can carry a watermark
//!    (else exit 3) and goes on.
//! 2. `after`: a line is entered in the audit log (`audit.rs`), whose sequence number is the copy number, and every output gets
//!    a watermark with the copy number, the client address, the user and the date and time (Rules cl. 12). The outputs are
//!    validated again by the caller.
//!
//! The watermark per output kind:
//! - `pdf`: a footer line and a diagonal text on every page, with `pdf-cli` (`doc_header_footer`, `doc_watermark`);
//! - `png`: a strip with the line, drawn by `pdf-cli` (a one-line page rendered at the png's dpi) and appended under the page, plus
//!   the line as a `tEXt` chunk named `XDFU`. PNG pixels are never covered;
//! - `docx`: a diagonal watermark and the custom properties `XDFU-copy` and `XDFU-stamp`, with `docs-cli`, then the stamp line as a
//!   paragraph at the top of the body and at the top of every header and footer part that exists. Nothing is replaced: the document's
//!   own letterhead, page numbers and grif stay (gate review M3);
//! - `xlsx`: the stamp line is merged into the centre of every sheet's header and footer (the existing text stays, first-page and
//!   even-page headers included); a header too long to take it is skipped, and a sheet where none fits fails the job;
//! - `pptx`: a text box with the stamp line at the bottom of every slide; the slides' own footers stay;
//! - `txt`, `md`, `csv`, `html`: a first line (a quoted one-field row in csv; a paragraph after `<body>` in html);
//! - `odt` and `rtf` cannot carry one, so the job is refused before any tool runs.
//!
//! `shortcut: for doc, rtf, xls and xlsb the probe conversion is what the importer keeps; a header that the importer drops is missed
//! (and so is lost from the result, which defects.md records).`
//! `shortcut: the grif is looked for by text in the top part of the first page, not by layout: the first 25 paragraphs of a docx
//! (and every paragraph of a table that has one of them: a two-column letterhead),
//! the first 5 rows of the first sheet, the top quarter of the first slide, the first 8 lines of page 1. A word that is also an
//! ordinary word (ДСП is chipboard in Russian) in those places is a false positive; that errs on the safe side.`
use crate::audit::{self, AuditLog};
use crate::exec::{Exit, Failure, fail};
use crate::hook::{JobCtx, JobHook, ToolRunner};
use crate::kind::Kind;
use crate::plan::ToolId;
use crate::report::{self, Warning};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub use xdfu_core::{
    Mode, Stamp, default_audit_path, find_grif, machine_addr, machine_name, os_user,
};

#[derive(Clone, Debug)]
pub struct Evidence {
    /// `first-page-top`, `header`, `properties` or `first-lines`
    pub place: &'static str,
    /// the package part, or the property, where it was found
    pub part: String,
    pub marker: &'static str,
}

impl Evidence {
    fn to_json(&self) -> Value {
        json!({"place": self.place, "part": self.part, "marker": self.marker})
    }
}

// ---- reading the packages ------------------------------------------------------------------------------------------------

const PART_LIMIT: u64 = 32 << 20;

struct Package<R: Read + Seek = fs::File>(zip::ZipArchive<R>);

fn open_file(path: &Path) -> Result<fs::File, String> {
    fs::File::open(path).map_err(|e| e.to_string())
}

#[cfg(test)]
impl Package {
    fn open(path: &Path) -> Result<Package, String> {
        Package::new(open_file(path)?)
    }
}

impl<R: Read + Seek> Package<R> {
    fn new(r: R) -> Result<Package<R>, String> {
        zip::ZipArchive::new(r)
            .map(Package)
            .map_err(|e| format!("the package cannot be read: {e}"))
    }
    fn names(&mut self) -> Vec<String> {
        (0..self.0.len())
            .filter_map(|i| {
                self.0
                    .by_index_raw(i)
                    .ok()
                    .and_then(|e| e.name().ok().map(|n| n.into_owned()))
            })
            .collect()
    }
    fn text(&mut self, name: &str) -> Option<String> {
        let f = self.0.by_name(name).ok()?;
        let mut b = Vec::new();
        f.take(PART_LIMIT).read_to_end(&mut b).ok()?;
        Some(String::from_utf8_lossy(&b).into_owned())
    }
}

fn decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';').filter(|e| *e <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..end];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => ent
                .strip_prefix('#')
                .and_then(|n| {
                    n.strip_prefix(['x', 'X'])
                        .map_or_else(|| n.parse().ok(), |h| u32::from_str_radix(h, 16).ok())
                })
                .and_then(char::from_u32),
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The tags of `xml` in order as `(name, is_closing, start, end)`; text between tags is `xml[end..next.start]`.
fn tags(xml: &str) -> Vec<(&str, bool, usize, usize)> {
    let b = xml.as_bytes();
    let mut v = vec![];
    let mut i = 0;
    while let Some(off) = xml[i..].find('<') {
        let start = i + off;
        let Some(len) = xml[start..].find('>') else {
            break;
        };
        let inner = &xml[start + 1..start + len];
        if !(inner.starts_with('?') || inner.starts_with('!')) {
            let closing = inner.starts_with('/');
            let body = inner.trim_start_matches('/');
            let name = body
                .split(|c: char| c.is_whitespace() || c == '/')
                .next()
                .unwrap_or("");
            v.push((name, closing, start, start + len + 1));
        }
        i = start + len + 1;
        if i >= b.len() {
            break;
        }
    }
    v
}

/// The text of the paragraphs of an OOXML part: `para` opens a paragraph (`w:p`, `a:p`), `t` holds text (`w:t`, `a:t`), and the tags
/// in `gap` (tab, break) count as a space.
fn paragraphs(xml: &str, para: &str, t: &str, gap: &[&str]) -> Vec<String> {
    let mut out = vec![String::new()];
    let tg = tags(xml);
    for (k, (name, closing, _, end)) in tg.iter().enumerate() {
        if *name == para && !closing {
            out.push(String::new());
        } else if *name == t && !closing {
            let next = tg.get(k + 1).map_or(xml.len(), |n| n.2);
            out.last_mut()
                .expect("never empty")
                .push_str(&decode(&xml[*end..next]));
        } else if gap.contains(name) && !closing {
            out.last_mut().expect("never empty").push(' ');
        }
    }
    out.retain(|p| !p.trim().is_empty());
    out
}

/// All text of an XML part, tags removed.
fn plain(xml: &str) -> String {
    let mut out = String::new();
    let mut last = 0;
    for (_, _, start, end) in tags(xml) {
        out.push_str(&xml[last..start]);
        out.push(' ');
        last = end;
    }
    out.push_str(&xml[last..]);
    decode(&out)
}

/// The text of an HTML file as lines: one per run of text between tags, without `<head>`, `<style>` and `<script>` and with entities decoded.
fn html_lines(raw: &str) -> Vec<String> {
    let tg = tags(raw);
    let (mut out, mut skip) = (vec![], None::<&str>);
    let lead = decode(&raw[..tg.first().map_or(raw.len(), |t| t.2)]);
    if !lead.trim().is_empty() {
        out.push(lead.trim().to_string());
    }
    for (k, (name, closing, _, end)) in tg.iter().enumerate() {
        let lname = name.to_ascii_lowercase();
        match skip {
            Some(s) if *closing && lname == s => skip = None,
            Some(_) => continue,
            None if !*closing && matches!(lname.as_str(), "head" | "style" | "script") => {
                skip = Some(
                    ["head", "style", "script"]
                        .into_iter()
                        .find(|n| *n == lname)
                        .expect("matched above"),
                );
                continue;
            }
            None => {}
        }
        let next = tg.get(k + 1).map_or(raw.len(), |n| n.2);
        let text = decode(&raw[*end..next]);
        if !text.trim().is_empty() {
            out.push(text.trim().to_string());
        }
    }
    out
}

/// A spreadsheet header or footer without its `&L` `&C` `&R` section codes, font and size codes.
fn header_text(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] == '&' && i + 1 < c.len() {
            match c[i + 1] {
                'L' | 'C' | 'R' => {
                    out.push(' ');
                    i += 2;
                }
                '"' => {
                    i += 2;
                    while i < c.len() && c[i] != '"' {
                        i += 1;
                    }
                    i += 1;
                }
                d if d.is_ascii_digit() => {
                    i += 1;
                    while i < c.len() && c[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                _ => {
                    out.push(c[i + 1]);
                    i += 2;
                }
            }
        } else {
            out.push(c[i]);
            i += 1;
        }
    }
    out
}

fn attr<'a>(tag_text: &'a str, name: &str) -> Option<&'a str> {
    // an attribute name follows white space, so `t="` is not found inside `ht="`
    let key = format!(" {name}=\"");
    let i = tag_text.find(&key)? + key.len();
    let j = tag_text[i..].find('"')?;
    Some(&tag_text[i..i + j])
}

fn hit(found: &mut Vec<Evidence>, text: &str, place: &'static str, part: &str) {
    if let Some(marker) = find_grif(text) {
        found.push(Evidence {
            place,
            part: part.to_string(),
            marker,
        });
    }
}

/// The OOXML document properties: core, app and custom parts, all text.
fn properties<R: Read + Seek>(p: &mut Package<R>, found: &mut Vec<Evidence>) {
    for part in [
        "docProps/core.xml",
        "docProps/app.xml",
        "docProps/custom.xml",
    ] {
        if let Some(x) = p.text(part) {
            hit(found, &plain(&x), "properties", part);
        }
    }
}

const TOP_PARAGRAPHS: usize = 25;
const TOP_ROWS: usize = 5;

/// The paragraphs at the top of a docx body: the first `TOP_PARAGRAPHS` in reading order, plus every paragraph of a table that has one
/// of them. A two-column letterhead is a table whose right cell comes after all of the left one (gate review M4).
fn top_paragraphs(xml: &str) -> Vec<String> {
    let tg = tags(xml);
    let mut paras: Vec<(String, Option<usize>)> = vec![];
    let (mut depth, mut tables, mut cur) = (0usize, 0usize, None);
    for (k, (name, closing, _, end)) in tg.iter().enumerate() {
        match (*name, *closing) {
            ("w:tbl", false) => {
                if depth == 0 {
                    tables += 1;
                    cur = Some(tables);
                }
                depth += 1;
            }
            ("w:tbl", true) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    cur = None;
                }
            }
            ("w:p", false) => paras.push((String::new(), cur)),
            ("w:t", false) => {
                let next = tg.get(k + 1).map_or(xml.len(), |n| n.2);
                if let Some(p) = paras.last_mut() {
                    p.0.push_str(&decode(&xml[*end..next]));
                }
            }
            ("w:tab" | "w:br" | "w:cr", false) => {
                if let Some(p) = paras.last_mut() {
                    p.0.push(' ');
                }
            }
            _ => {}
        }
    }
    paras.retain(|p| !p.0.trim().is_empty());
    let top_tables: std::collections::HashSet<usize> = paras
        .iter()
        .take(TOP_PARAGRAPHS)
        .filter_map(|p| p.1)
        .collect();
    paras
        .into_iter()
        .enumerate()
        .filter(|(i, p)| *i < TOP_PARAGRAPHS || p.1.is_some_and(|t| top_tables.contains(&t)))
        .map(|(_, p)| p.0)
        .collect()
}

fn detect_docx(src: impl Read + Seek) -> Result<Vec<Evidence>, String> {
    let mut p = Package::new(src)?;
    let mut found = vec![];
    if let Some(doc) = p.text("word/document.xml") {
        for t in top_paragraphs(&doc) {
            hit(&mut found, &t, "first-page-top", "word/document.xml");
        }
    }
    for n in p
        .names()
        .into_iter()
        .filter(|n| n.starts_with("word/header") && n.ends_with(".xml"))
    {
        if let Some(x) = p.text(&n) {
            for t in paragraphs(&x, "w:p", "w:t", &["w:tab", "w:br", "w:cr"]) {
                hit(&mut found, &t, "header", &n);
            }
        }
    }
    properties(&mut p, &mut found);
    Ok(found)
}

fn detect_xlsx(src: impl Read + Seek) -> Result<Vec<Evidence>, String> {
    let mut p = Package::new(src)?;
    let mut found = vec![];
    // headers and footers of every sheet
    for n in p
        .names()
        .into_iter()
        .filter(|n| n.starts_with("xl/worksheets/") && n.ends_with(".xml"))
    {
        let Some(x) = p.text(&n) else { continue };
        let tg = tags(&x);
        for (k, (name, closing, _, end)) in tg.iter().enumerate() {
            if !closing
                && matches!(
                    *name,
                    "oddHeader"
                        | "oddFooter"
                        | "evenHeader"
                        | "evenFooter"
                        | "firstHeader"
                        | "firstFooter"
                )
            {
                let next = tg.get(k + 1).map_or(x.len(), |t| t.2);
                hit(
                    &mut found,
                    &header_text(&decode(&x[*end..next])),
                    "header",
                    &n,
                );
            }
        }
    }
    // the top rows of the first sheet
    // one entry per <si>, empty ones included: a cell's index counts them (gate review m13)
    let shared: Vec<String> = p
        .text("xl/sharedStrings.xml")
        .map(|x| {
            x.split("<si")
                .skip(1)
                .filter(|seg| seg.starts_with(['>', ' ', '/']))
                .map(|seg| paragraphs(seg, "\0", "t", &[]).concat())
                .collect()
        })
        .unwrap_or_default();
    let sheet = (|| {
        let wb = p.text("xl/workbook.xml")?;
        let first = tags(&wb).into_iter().find(|t| t.0 == "sheet" && !t.1)?;
        let rid = attr(&wb[first.2..first.3], "r:id")?.to_string();
        let rels = p.text("xl/_rels/workbook.xml.rels")?;
        let rel = tags(&rels)
            .into_iter()
            .filter(|t| t.0 == "Relationship" && !t.1)
            .find(|t| attr(&rels[t.2..t.3], "Id") == Some(rid.as_str()))?;
        let target = attr(&rels[rel.2..rel.3], "Target")?.to_string();
        Some(match target.strip_prefix('/') {
            Some(abs) => abs.to_string(),
            None => format!("xl/{target}"),
        })
    })();
    if let Some(name) = sheet
        && let Some(x) = p.text(&name)
    {
        let tg = tags(&x);
        let mut row = 0usize;
        for (k, (tn, closing, s, e)) in tg.iter().enumerate() {
            if *closing {
                continue;
            }
            if *tn == "row" {
                row = attr(&x[*s..*e], "r")
                    .and_then(|r| r.parse().ok())
                    .unwrap_or(row + 1);
            } else if *tn == "c" && row <= TOP_ROWS && row > 0 {
                let kind = attr(&x[*s..*e], "t").unwrap_or("");
                let end_c = tg[k + 1..]
                    .iter()
                    .find(|t| t.0 == "c" && !t.1 || t.0 == "row")
                    .map_or(x.len(), |t| t.2);
                let cell = &x[*e..end_c];
                let text = match kind {
                    "s" => plain(cell)
                        .trim()
                        .parse::<usize>()
                        .ok()
                        .and_then(|i| shared.get(i).cloned())
                        .unwrap_or_default(),
                    "inlineStr" | "str" => plain(cell),
                    _ => String::new(),
                };
                hit(&mut found, &text, "first-page-top", &name);
            }
        }
    }
    properties(&mut p, &mut found);
    Ok(found)
}

fn detect_pptx(src: impl Read + Seek) -> Result<Vec<Evidence>, String> {
    let mut p = Package::new(src)?;
    let mut found = vec![];
    for n in p.names().into_iter().filter(|n| {
        (n.starts_with("ppt/slideMasters/") || n.starts_with("ppt/slideLayouts/"))
            && n.ends_with(".xml")
    }) {
        if let Some(x) = p.text(&n) {
            for t in paragraphs(&x, "a:p", "a:t", &["a:br"]) {
                hit(&mut found, &t, "header", &n);
            }
        }
    }
    let first = (|| {
        let pres = p.text("ppt/presentation.xml")?;
        let id = tags(&pres).into_iter().find(|t| t.0 == "p:sldId" && !t.1)?;
        let rid = attr(&pres[id.2..id.3], "r:id")?.to_string();
        let height: u64 = tags(&pres)
            .into_iter()
            .find(|t| t.0 == "p:sldSz")
            .and_then(|t| attr(&pres[t.2..t.3], "cy")?.parse().ok())
            .unwrap_or(5_143_500);
        let rels = p.text("ppt/_rels/presentation.xml.rels")?;
        let rel = tags(&rels)
            .into_iter()
            .filter(|t| t.0 == "Relationship" && !t.1)
            .find(|t| attr(&rels[t.2..t.3], "Id") == Some(rid.as_str()))?;
        let target = attr(&rels[rel.2..rel.3], "Target")?.to_string();
        Some((
            target
                .strip_prefix('/')
                .map_or_else(|| format!("ppt/{target}"), String::from),
            height,
        ))
    })();
    if let Some((name, height)) = first
        && let Some(x) = p.text(&name)
    {
        // shape by shape: a shape in the top quarter, or one that takes its place from the layout, is the top of the page
        for shape in x.split("<p:sp>").skip(1).chain(x.split("<p:sp ").skip(1)) {
            let top = tags(shape)
                .into_iter()
                .find(|t| t.0 == "a:off" && !t.1)
                .and_then(|t| attr(&shape[t.2..t.3], "y")?.parse::<u64>().ok());
            if top.is_none_or(|y| y < height / 4) {
                for t in paragraphs(shape, "a:p", "a:t", &["a:br"]) {
                    hit(&mut found, &t, "first-page-top", &name);
                }
            }
        }
    }
    properties(&mut p, &mut found);
    Ok(found)
}

/// OpenDocument text and spreadsheets: the top paragraphs of `content.xml`, every header and footer (the only text in `styles.xml`), and
/// `meta.xml` (title, subject, keywords).
fn detect_odf(src: impl Read + Seek) -> Result<Vec<Evidence>, String> {
    let mut p = Package::new(src)?;
    let mut found = vec![];
    if let Some(x) = p.text("content.xml") {
        let tg = tags(&x);
        let (mut paras, mut open) = (Vec::<String>::new(), false);
        for (k, (name, closing, _, end)) in tg.iter().enumerate() {
            match (*name, *closing) {
                ("text:p" | "text:h", false) => {
                    paras.push(String::new());
                    open = true;
                }
                ("text:p" | "text:h", true) => open = false,
                ("text:s" | "text:tab" | "text:line-break", false) if open => {
                    paras.last_mut().expect("open").push(' ')
                }
                _ => {}
            }
            if open && let Some(cur) = paras.last_mut() {
                let next = tg.get(k + 1).map_or(x.len(), |n| n.2);
                cur.push_str(&decode(&x[*end..next]));
            }
        }
        for t in paras
            .iter()
            .filter(|t| !t.trim().is_empty())
            .take(TOP_PARAGRAPHS)
        {
            hit(&mut found, t, "first-page-top", "content.xml");
        }
    }
    if let Some(x) = p.text("styles.xml") {
        hit(&mut found, &plain(&x), "header", "styles.xml");
    }
    if let Some(x) = p.text("meta.xml") {
        hit(&mut found, &plain(&x), "properties", "meta.xml");
    }
    Ok(found)
}

fn lines_hit(text: &str, place: &'static str, part: &str, found: &mut Vec<Evidence>) {
    for l in text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(TOP_PARAGRAPHS)
    {
        hit(found, l, place, part);
    }
}

/// `(evidence, checkable)`: a PDF whose page 1 has no text (a scan, an image) cannot be read for a grif, which is not the same as
/// "not marked" (gate review M4); properties that name the grif still count as evidence.
fn detect_pdf(
    rel: &str,
    size_mb: f64,
    tools: &dyn ToolRunner,
) -> Result<(Vec<Evidence>, bool), Failure> {
    let mut found = vec![];
    let page1 = tools.run(
        ToolId::Pdf,
        &["text".into(), rel.into(), "--page".into(), "1".into()],
        size_mb,
    )?;
    let page1 = String::from_utf8_lossy(&page1);
    let checkable = !page1.trim().is_empty();
    lines_hit(&page1, "first-page-top", "page 1", &mut found);
    let info = tools.run(ToolId::Pdf, &["info".into(), rel.into()], size_mb)?;
    if let Ok(v) = serde_json::from_slice::<Value>(&info) {
        for k in ["title", "author", "subject", "keywords"] {
            if let Some(s) = v[k].as_str() {
                hit(&mut found, s, "properties", k);
            }
        }
    }
    Ok((found, checkable))
}

/// Kinds `detect` reads (doc, rtf, xls and xlsb through a probe conversion). A job whose input kind is not one of these is checked on
/// its output (see the module doc).
pub fn scannable(kind: Option<Kind>) -> bool {
    matches!(
        kind,
        Some(
            Kind::Docx
                | Kind::Doc
                | Kind::Rtf
                | Kind::Odt
                | Kind::Ods
                | Kind::Xls
                | Kind::Xlsb
                | Kind::Xlsx
                | Kind::Pptx
                | Kind::Pdf
                | Kind::Txt
                | Kind::Md
                | Kind::Csv
                | Kind::Tsv
                | Kind::Html
        )
    )
}

/// Output kinds `detect` reads (a png is an image: no text to look at).
pub fn output_scannable(ext: &str) -> bool {
    matches!(
        ext,
        "docx" | "xlsx" | "pptx" | "pdf" | "txt" | "md" | "csv" | "html"
    )
}

/// Look for the grif in one input file, in the places the Rules name. `rel` is the path inside the job directory (the tools'
/// working directory). A file that cannot be read at all is an error: not knowing is not the same as "not marked".
pub fn detect(
    path: &Path,
    rel: &str,
    kind: Option<Kind>,
    tools: &dyn ToolRunner,
) -> Result<Vec<Evidence>, Failure> {
    detect_checked(path, rel, kind, 0, tools).map(|(found, _)| found)
}

/// Like `detect`, with whether the file could really be read for a grif (`false`: an image-only PDF). `n` numbers the probe file.
fn detect_checked(
    path: &Path,
    rel: &str,
    kind: Option<Kind>,
    n: usize,
    tools: &dyn ToolRunner,
) -> Result<(Vec<Evidence>, bool), Failure> {
    let unreadable = |e: String| Failure {
        exit: Exit::ToolError,
        msg: format!("the XDFU check could not read the input: {e}"),
    };
    let mb = fs::metadata(path)
        .map(|m| m.len() as f64 / 1_048_576.0)
        .unwrap_or(0.0);
    // the legacy kinds: what the importer makes of them, as docx or xlsx, in `mid/` (not collected)
    let probe = |tool: ToolId, ext: &str| -> Result<String, Failure> {
        let out = format!("mid/xdfu-probe-{n}.{ext}");
        tools.run(tool, &["convert".into(), rel.into(), out.clone()], mb)?;
        Ok(out)
    };
    let checked = |r: Result<Vec<Evidence>, String>| r.map(|f| (f, true)).map_err(unreadable);
    match kind {
        Some(Kind::Docx) => checked(open_file(path).and_then(detect_docx)),
        Some(Kind::Xlsx) => checked(open_file(path).and_then(detect_xlsx)),
        Some(Kind::Pptx) => checked(open_file(path).and_then(detect_pptx)),
        Some(Kind::Odt | Kind::Ods) => checked(open_file(path).and_then(detect_odf)),
        Some(Kind::Doc | Kind::Rtf) => {
            let out = probe(ToolId::Docs, "docx")?;
            let r = checked(open_file(&tools_path(path, &out)).and_then(detect_docx));
            let _ = fs::remove_file(tools_path(path, &out));
            r
        }
        Some(Kind::Xls | Kind::Xlsb) => {
            let out = probe(ToolId::Sheets, "xlsx")?;
            let r = checked(open_file(&tools_path(path, &out)).and_then(detect_xlsx));
            let _ = fs::remove_file(tools_path(path, &out));
            r
        }
        Some(Kind::Pdf) => detect_pdf(rel, mb, tools),
        Some(Kind::Html) => {
            let mut b = Vec::new();
            // a Word-saved page carries a long style block before the body
            fs::File::open(path)
                .and_then(|f| f.take(1 << 20).read_to_end(&mut b))
                .map_err(|e| unreadable(e.to_string()))?;
            let mut found = vec![];
            for l in html_lines(&String::from_utf8_lossy(&b))
                .iter()
                .take(TOP_PARAGRAPHS)
            {
                hit(&mut found, l, "first-lines", "html");
            }
            Ok((found, true))
        }
        Some(Kind::Txt | Kind::Md | Kind::Csv | Kind::Tsv) => {
            let mut b = Vec::new();
            fs::File::open(path)
                .and_then(|f| f.take(64 * 1024).read_to_end(&mut b))
                .map_err(|e| unreadable(e.to_string()))?;
            let mut found = vec![];
            lines_hit(
                &String::from_utf8_lossy(&b),
                "first-lines",
                "text",
                &mut found,
            );
            Ok((found, true))
        }
        _ => Ok((vec![], true)),
    }
}

/// The file `rel` names inside the job directory that holds `input` (`<job>/in/x` or `<job>/out/x`).
fn tools_path(input: &Path, rel: &str) -> PathBuf {
    input
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| PathBuf::from(rel), |job| job.join(rel))
}

// ---- the watermark -------------------------------------------------------------------------------------------------------

/// Output kinds that can carry a watermark.
pub fn can_carry(ext: &str) -> bool {
    matches!(
        ext,
        "txt" | "md" | "csv" | "html" | "docx" | "xlsx" | "pptx" | "pdf" | "png"
    )
}

/// The document server's entry (WP14): the grif found in a docx, xlsx or pptx held in memory, in the places `detect` reads; `Err` when
/// the package cannot be read or the type is not one of the three (not knowing is not the same as "not marked").
pub fn grif_in_ooxml(bytes: &[u8], ext: &str) -> Result<Option<&'static str>, String> {
    let r = std::io::Cursor::new(bytes);
    let found = match ext {
        "docx" => detect_docx(r),
        "xlsx" => detect_xlsx(r),
        "pptx" => detect_pptx(r),
        _ => return Err(format!("a .{ext} document cannot be checked for the grif")),
    }?;
    Ok(found.first().map(|e| e.marker))
}

/// The document server's stamp (WP14): `line` put into a docx, xlsx or pptx held in memory, the same way the converter does (no tool).
pub fn stamp_ooxml(bytes: &[u8], ext: &str, line: &str) -> Result<Vec<u8>, String> {
    let (r, mut out) = (
        std::io::Cursor::new(bytes),
        std::io::Cursor::new(Vec::new()),
    );
    match ext {
        "docx" => stamp_docx_io(r, &mut out, line),
        "xlsx" => stamp_xlsx_io(r, &mut out, line),
        "pptx" => stamp_pptx_io(r, &mut out, line),
        _ => return Err(format!("a .{ext} document cannot carry the stamp")),
    }
    .map_err(|f| f.msg)?;
    Ok(out.into_inner())
}

fn io_err(what: &str, e: impl std::fmt::Display) -> Failure {
    Failure {
        exit: Exit::ToolError,
        msg: format!("XDFU watermark: {what}: {e}"),
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn stamp_text_file(path: &Path, ext: &str, line: &str) -> Result<(), Failure> {
    let old = fs::read(path).map_err(|e| io_err("cannot read the output", e))?;
    let new = xdfu_core::stamp_text(&old, ext, line).map_err(|e| Failure {
        exit: Exit::ToolError,
        msg: format!("XDFU watermark: {e}"),
    })?;
    fs::write(path, new).map_err(|e| io_err("cannot write the output", e))
}

/// The temporary file a tool writes the marked copy to, inside the job directory (`mid/` is not collected).
fn temp_for(ctx: &JobCtx, n: usize, ext: &str) -> (PathBuf, String) {
    let rel = format!("mid/xdfu-{n}.{ext}");
    (ctx.job_dir.join(&rel), rel)
}

fn replace_with(tmp: &Path, target: &Path) -> Result<(), Failure> {
    let r = fs::rename(tmp, target);
    if r.is_err() {
        let _ = fs::remove_file(tmp);
    }
    r.map_err(|e| io_err("cannot put the marked file in place", e))
}

fn cmd(id: &str, params: Value) -> String {
    format!("{id}={params}")
}

/// Rewrite the zip `src` into `dst`: the parts `wants` names go through `edit` (`Some(new text)` replaces the part), every other part is
/// copied as it is. Returns how many parts were replaced.
fn rewrite_zip(
    src: impl Read + Seek,
    dst: impl std::io::Write + Seek,
    wants: impl Fn(&str) -> bool,
    mut edit: impl FnMut(&str, &str) -> Result<Option<String>, Failure>,
) -> Result<usize, Failure> {
    let mut zin =
        zip::ZipArchive::new(src).map_err(|e| io_err("the output package cannot be read", e))?;
    let mut zout = zip::ZipWriter::new(dst);
    let mut changed = 0;
    for i in 0..zin.len() {
        let name = zin
            .by_index_raw(i)
            .map_err(|e| io_err("the output package cannot be read", e))?
            .name()
            .map(|n| n.into_owned())
            .map_err(|e| io_err("the output package has a bad part name", e))?;
        let new = if wants(&name) {
            let mut text = String::new();
            zin.by_index(i)
                .map_err(|e| io_err("the output package cannot be read", e))?
                .take(PART_LIMIT)
                .read_to_string(&mut text)
                .map_err(|e| io_err("a part of the output is not text", e))?;
            edit(&name, &text)?
        } else {
            None
        };
        match new {
            Some(text) => {
                zout.start_file(&name, zip::write::SimpleFileOptions::default())
                    .and_then(|_| zout.write_all(text.as_bytes()).map_err(Into::into))
                    .map_err(|e| io_err("cannot write the output", e))?;
                changed += 1;
            }
            None => zout
                .raw_copy_file(
                    zin.by_index_raw(i)
                        .map_err(|e| io_err("the output package cannot be read", e))?,
                )
                .map_err(|e| io_err("cannot write the output", e))?,
        }
    }
    zout.finish()
        .map_err(|e| io_err("cannot finish the output", e))?;
    Ok(changed)
}

/// The file-to-file form of the `stamp_*_io` functions.
fn on_files(
    src: &Path,
    dst: &Path,
    f: impl FnOnce(fs::File, fs::File) -> Result<(), Failure>,
) -> Result<(), Failure> {
    f(
        fs::File::open(src).map_err(|e| io_err("cannot read the output", e))?,
        fs::File::create(dst).map_err(|e| io_err("cannot write the output", e))?,
    )
}

fn stamp_docx_parts(src: &Path, dst: &Path, line: &str) -> Result<(), Failure> {
    on_files(src, dst, |s, d| stamp_docx_io(s, d, line))
}

fn stamp_xlsx_parts(src: &Path, dst: &Path, line: &str) -> Result<(), Failure> {
    on_files(src, dst, |s, d| stamp_xlsx_io(s, d, line))
}

fn stamp_pptx_parts(src: &Path, dst: &Path, line: &str) -> Result<(), Failure> {
    on_files(src, dst, |s, d| stamp_pptx_io(s, d, line))
}

/// `text` with `insert` put right after the end of the first tag that starts with `open` (`<w:body`, `<w:hdr`); an empty element
/// (`<w:hdr .../>`) is opened and closed around it.
fn insert_in(text: &str, open: &str, close: &str, insert: &str) -> Option<String> {
    let at = text.find(open)?;
    let gt = at + text[at..].find('>')?;
    Some(if text[..gt].ends_with('/') {
        format!("{}>{insert}{close}{}", &text[..gt - 1], &text[gt + 1..])
    } else {
        format!("{}{insert}{}", &text[..=gt], &text[gt + 1..])
    })
}

/// docx: the stamp as the first paragraph of the body and of every header and footer part. Nothing is removed.
fn stamp_docx_io(src: impl Read + Seek, dst: impl Write + Seek, line: &str) -> Result<(), Failure> {
    let para = format!(
        "<w:p><w:pPr><w:jc w:val=\"center\"/></w:pPr><w:r><w:rPr><w:b/><w:color w:val=\"CC0000\"/><w:sz w:val=\"16\"/></w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
        html_escape(line)
    );
    let is_hf = |n: &str| {
        (n.starts_with("word/header") || n.starts_with("word/footer")) && n.ends_with(".xml")
    };
    let mut body = false;
    rewrite_zip(
        src,
        dst,
        |n| n == "word/document.xml" || is_hf(n),
        |n, text| {
            let (open, close) = match (n, n.starts_with("word/header")) {
                ("word/document.xml", _) => ("<w:body", "</w:body>"),
                (_, true) => ("<w:hdr", "</w:hdr>"),
                _ => ("<w:ftr", "</w:ftr>"),
            };
            let new = insert_in(text, open, close, &para).ok_or_else(|| {
                io_err(
                    &format!("{n} has no {open}>"),
                    "refusing to leave it unmarked",
                )
            })?;
            body |= n == "word/document.xml";
            Ok(Some(new))
        },
    )?;
    if body {
        Ok(())
    } else {
        fail(
            Exit::BadOutput,
            "XDFU watermark: the docx has no word/document.xml to mark",
        )
    }
}

/// A header or footer string of a sheet (`&L..&C..&R..`) with `stamp` (already `&&`-escaped) added at the start of its centre section.
fn merge_center(existing: &str, stamp: &str) -> String {
    let c: Vec<char> = existing.chars().collect();
    let (mut i, mut centre) = (0, None::<usize>);
    while i < c.len() {
        if c[i] == '&' && i + 1 < c.len() {
            match c[i + 1] {
                'C' => {
                    centre = Some(i + 2);
                    break;
                }
                '"' => {
                    i += 2;
                    while i < c.len() && c[i] != '"' {
                        i += 1;
                    }
                    i += 1;
                }
                _ => i += 2,
            }
        } else {
            i += 1;
        }
    }
    match centre {
        Some(at) => {
            let rest: String = c[at..].iter().collect();
            // the centre section ends at the next section code
            let empty = rest.is_empty() || rest.starts_with("&R") || rest.starts_with("&L");
            let sep = if empty { "" } else { "\n" };
            format!("{}{stamp}{sep}{rest}", c[..at].iter().collect::<String>())
        }
        None => format!("{existing}&C{stamp}"),
    }
}

/// One worksheet part with the stamp merged into its header and footer elements (the odd ones always, the even and first-page ones
/// when the sheet uses them). `Err` when no header or footer can take it within Excel's 255 characters.
fn stamp_sheet(xml: &str, line: &str) -> Result<String, String> {
    const NAMES: [&str; 6] = [
        "oddHeader",
        "oddFooter",
        "evenHeader",
        "evenFooter",
        "firstHeader",
        "firstFooter",
    ];
    let stamp = line.replace('&', "&&");
    let (open_tag, inner, span) = match xml.find("<headerFooter") {
        Some(at) => {
            let gt = at + xml[at..].find('>').ok_or("a broken headerFooter element")?;
            let selfclosed = xml[..gt].ends_with('/');
            let open = xml[at..gt].trim_end_matches('/').to_string();
            if selfclosed {
                (open, String::new(), (at, gt + 1))
            } else {
                let end = at
                    + xml[at..]
                        .find("</headerFooter>")
                        .ok_or("a broken headerFooter element")?;
                (
                    open,
                    xml[gt + 1..end].to_string(),
                    (at, end + "</headerFooter>".len()),
                )
            }
        }
        None => {
            // after pageSetup and before everything that follows it in the schema
            let at = [
                "<rowBreaks",
                "<colBreaks",
                "<customProperties",
                "<cellWatches",
                "<ignoredErrors",
                "<smartTags",
                "<drawing",
                "<legacyDrawing",
                "<legacyDrawingHF",
                "<picture",
                "<oleObjects",
                "<controls",
                "<webPublishItems",
                "<tableParts",
                "<extLst",
                "</worksheet>",
            ]
            .iter()
            .filter_map(|t| xml.find(t))
            .min()
            .ok_or("no worksheet end")?;
            ("<headerFooter".to_string(), String::new(), (at, at))
        }
    };
    let (first, even) = (
        attr(&format!("{open_tag}>"), "differentFirst").is_some_and(|v| v == "1" || v == "true"),
        attr(&format!("{open_tag}>"), "differentOddEven").is_some_and(|v| v == "1" || v == "true"),
    );
    let mut out = String::new();
    let mut placed = 0;
    for name in NAMES {
        let open = format!("<{name}>");
        let old = inner.find(&open).and_then(|i| {
            let from = i + open.len();
            inner[from..]
                .find(&format!("</{name}>"))
                .map(|e| decode(&inner[from..from + e]))
        });
        let wanted = old.is_some()
            || name.starts_with("odd")
            || (name.starts_with("even") && even)
            || (name.starts_with("first") && first);
        if !wanted {
            continue;
        }
        let old = old.unwrap_or_default();
        let merged = merge_center(&old, &stamp);
        let value = if merged.chars().count() <= 255 {
            placed += 1;
            merged
        } else {
            old
        };
        if !value.is_empty() {
            out.push_str(&format!("<{name}>{}</{name}>", html_escape(&value)));
        }
    }
    if placed == 0 {
        return Err("every header and footer of a sheet is too long to take the stamp".into());
    }
    Ok(format!(
        "{}{open_tag}>{out}</headerFooter>{}",
        &xml[..span.0],
        &xml[span.1..]
    ))
}

fn stamp_xlsx_io(src: impl Read + Seek, dst: impl Write + Seek, line: &str) -> Result<(), Failure> {
    let n = rewrite_zip(
        src,
        dst,
        |n| n.starts_with("xl/worksheets/") && n.ends_with(".xml"),
        |n, text| {
            stamp_sheet(text, line)
                .map(Some)
                .map_err(|e| io_err(&format!("sheet {n}"), e))
        },
    )?;
    if n == 0 {
        return fail(
            Exit::BadOutput,
            "XDFU watermark: the workbook lists no sheets to mark",
        );
    }
    Ok(())
}

/// pptx: a text box with the stamp along the bottom edge of every slide. The slides' own footers stay.
fn stamp_pptx_io(src: impl Read + Seek, dst: impl Write + Seek, line: &str) -> Result<(), Failure> {
    let mut src = src;
    let size = {
        let mut p = Package::new(&mut src).map_err(|e| io_err("cannot read the output", e))?;
        p.text("ppt/presentation.xml")
            .and_then(|x| {
                let t = tags(&x).into_iter().find(|t| t.0 == "p:sldSz")?;
                let tag = &x[t.2..t.3];
                Some((
                    attr(tag, "cx")?.parse::<u64>().ok()?,
                    attr(tag, "cy")?.parse::<u64>().ok()?,
                ))
            })
            .unwrap_or((9_144_000, 5_143_500))
    };
    src.rewind()
        .map_err(|e| io_err("cannot read the output", e))?;
    let h = 300_000u64.min(size.1 / 4);
    let n = rewrite_zip(
        src,
        dst,
        |n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"),
        |n, text| {
            let id = text
                .split("<p:cNvPr id=\"")
                .skip(1)
                .filter_map(|r| r.split('"').next()?.parse::<u64>().ok())
                .max()
                .unwrap_or(1)
                + 1;
            let shape = format!(
                "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"XDFU stamp\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"0\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{h}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr wrap=\"square\" anchor=\"b\"/><a:lstStyle/><a:p><a:pPr algn=\"ctr\"/><a:r><a:rPr lang=\"en-US\" sz=\"800\" b=\"1\"><a:solidFill><a:srgbClr val=\"CC0000\"/></a:solidFill></a:rPr><a:t>{}</a:t></a:r></a:p></p:txBody></p:sp>",
                size.1 - h,
                size.0,
                html_escape(line)
            );
            let at = text.rfind("</p:spTree>").ok_or_else(|| {
                io_err(
                    &format!("{n} has no </p:spTree>"),
                    "refusing to leave it unmarked",
                )
            })?;
            Ok(Some(format!("{}{shape}{}", &text[..at], &text[at..])))
        },
    )?;
    if n == 0 {
        return fail(
            Exit::BadOutput,
            "XDFU watermark: the deck has no slides to mark",
        );
    }
    Ok(())
}

fn stamp_with_tools(
    ctx: &JobCtx,
    file: &Path,
    n: usize,
    ext: &str,
    s: &Stamp,
    tools: &dyn ToolRunner,
) -> Result<(), Failure> {
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let rel_in = format!("out/{name}");
    let (tmp, rel_out) = temp_for(ctx, n, ext);
    let mb = fs::metadata(file)
        .map(|m| m.len() as f64 / 1_048_576.0)
        .unwrap_or(0.0);
    let line = s.line();
    let s_ = |v: &str| v.to_string();
    let run = |tool: ToolId, argv: Vec<String>| tools.run(tool, &argv, mb).map(|_| ());
    let done = match ext {
        "docx" => run(
            ToolId::Docs,
            vec![
                s_("run"),
                s_("--file"),
                rel_in,
                s_("--cmd"),
                cmd(
                    "design.watermark",
                    json!({"text": format!("XDFU {}", s.copy), "diagonal": true, "color": "CC0000"}),
                ),
                s_("--cmd"),
                cmd(
                    "file.properties",
                    json!({"custom": {"XDFU-copy": s.copy.to_string(), "XDFU-stamp": line}}),
                ),
                s_("--save"),
                rel_out.clone(),
            ],
        )
        .and_then(|_| {
            // the tool's output is `tmp`; the stamp paragraphs go into a second copy that replaces it
            let second = ctx.job_dir.join(format!("mid/xdfu-{n}-stamped.docx"));
            stamp_docx_parts(&tmp, &second, &line)?;
            fs::rename(&second, &tmp).map_err(|e| io_err("cannot put the marked file in place", e))
        }),
        // no tool: the stamp is merged into the package, so that nothing the document has is replaced (gate review M3)
        "pptx" => stamp_pptx_parts(&ctx.job_dir.join(&rel_in), &tmp, &line),
        "xlsx" => stamp_xlsx_parts(&ctx.job_dir.join(&rel_in), &tmp, &line),
        "pdf" => {
            let text = s.ascii_line();
            let steps = json!([
                {"tool": "doc_open", "args": {"path": rel_in}},
                {"tool": "doc_header_footer", "args": {"doc": 1, "footer_center": text, "font_size": 8, "color": "#CC0000"}},
                {"tool": "doc_watermark", "args": {"doc": 1, "text": format!("XDFU {}", s.copy), "opacity": 0.2, "rotation": 45}},
                {"tool": "doc_save", "args": {"doc": 1, "path": rel_out, "full": true}},
            ]);
            let script = format!("mid/xdfu-{n}.json");
            fs::write(ctx.job_dir.join(&script), steps.to_string())
                .map_err(|e| io_err("cannot write the script", e))?;
            let r = run(
                ToolId::Pdf,
                vec![
                    s_("run"),
                    s_("--script"),
                    script.clone(),
                    s_("--root"),
                    s_("."),
                ],
            );
            let _ = fs::remove_file(ctx.job_dir.join(&script));
            r
        }
        _ => unreachable!("only tool-made kinds come here"),
    };
    if let Err(e) = done {
        let _ = fs::remove_file(&tmp);
        return Err(Failure {
            exit: e.exit,
            msg: format!("XDFU watermark on {ext}: {}", e.msg),
        });
    }
    replace_with(&tmp, file)
}

/// One strip per png width, made once per job: the line as a page rendered by `pdf-cli` at the png's dpi, its empty rows cut off.
struct Strips(HashMap<u32, (Vec<u8>, u32)>);

fn png_strip(
    ctx: &JobCtx,
    width_px: u32,
    line: &str,
    tools: &dyn ToolRunner,
    strips: &mut Strips,
) -> Result<(Vec<u8>, u32), Failure> {
    if let Some(s) = strips.0.get(&width_px) {
        return Ok(s.clone());
    }
    let dpi = ctx.opts.dpi.clamp(72, 600);
    let width_pt = (width_px as f64 * 72.0 / dpi as f64).max(120.0);
    let (pdf, png) = (
        format!("mid/xdfu-strip-{width_px}.pdf"),
        format!("mid/xdfu-strip-{width_px}.png"),
    );
    let steps = json!([
        {"tool": "doc_create", "args": {"from": "blank", "pages": 1, "width": width_pt, "height": 48}},
        {"tool": "page_add_text", "args": {"doc": 1, "page": 1, "text": line, "at": [4, 4], "width": width_pt - 8.0, "size": 9, "bold": true, "color": "#CC0000"}},
        {"tool": "doc_save", "args": {"doc": 1, "path": pdf, "full": true}},
    ]);
    let script = format!("mid/xdfu-strip-{width_px}.json");
    fs::write(ctx.job_dir.join(&script), steps.to_string())
        .map_err(|e| io_err("cannot write the script", e))?;
    let s_ = |v: &str| v.to_string();
    tools.run(
        ToolId::Pdf,
        &[s_("run"), s_("--script"), script, s_("--root"), s_(".")],
        0.0,
    )?;
    tools.run(
        ToolId::Pdf,
        &[
            s_("render"),
            pdf.clone(),
            s_("--page"),
            s_("1"),
            s_("--dpi"),
            dpi.to_string(),
            s_("--out"),
            png.clone(),
        ],
        0.0,
    )?;
    let (rgba, w, h) = read_png(&ctx.job_dir.join(&png))?;
    // keep the rows down to the last one that is not white, and a little padding
    let row_blank = |y: u32| {
        rgba[(y * w * 4) as usize..((y + 1) * w * 4) as usize]
            .chunks(4)
            .all(|p| p[0] > 245 && p[1] > 245 && p[2] > 245)
    };
    let last = (0..h)
        .rev()
        .find(|y| !row_blank(*y))
        .ok_or_else(|| io_err("the strip came out empty", "pdf-cli render"))?;
    let keep = (last + 1 + dpi / 36).min(h);
    // exactly `width_px` wide: the render may be a pixel off
    let mut out = Vec::with_capacity((width_px * keep * 4) as usize);
    for y in 0..keep {
        let row = &rgba[(y * w * 4) as usize..((y + 1) * w * 4) as usize];
        for x in 0..width_px as usize {
            out.extend_from_slice(row.get(x * 4..x * 4 + 4).unwrap_or(&[255, 255, 255, 255]));
        }
    }
    for f in [&pdf, &png] {
        let _ = fs::remove_file(ctx.job_dir.join(f));
    }
    strips.0.insert(width_px, (out.clone(), keep));
    Ok((out, keep))
}

/// A png as 8-bit RGBA, whatever it was.
fn read_png(path: &Path) -> Result<(Vec<u8>, u32, u32), Failure> {
    let f = fs::File::open(path).map_err(|e| io_err("cannot read a png", e))?;
    let mut dec = png::Decoder::new(std::io::BufReader::new(f));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut r = dec
        .read_info()
        .map_err(|e| io_err("the png cannot be read", e))?;
    let mut buf = vec![
        0u8;
        r.output_buffer_size()
            .ok_or_else(|| io_err("the png is too large", "overflow"))?
    ];
    let info = r
        .next_frame(&mut buf)
        .map_err(|e| io_err("the png cannot be decoded", e))?;
    let data = &buf[..info.buffer_size()];
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .chunks(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => data
            .chunks(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Indexed => {
            return fail(
                Exit::BadOutput,
                "XDFU watermark: an indexed png was not expanded",
            );
        }
    };
    Ok((rgba, info.width, info.height))
}

fn stamp_png(
    ctx: &JobCtx,
    file: &Path,
    n: usize,
    s: &Stamp,
    tools: &dyn ToolRunner,
    strips: &mut Strips,
) -> Result<(), Failure> {
    let (rgba, w, h) = read_png(file)?;
    let text = s.ascii_line();
    let (strip, sh) = png_strip(ctx, w, &text, tools, strips)?;
    let (tmp, _) = temp_for(ctx, n, "png");
    {
        let out = fs::File::create(&tmp).map_err(|e| io_err("cannot write the png", e))?;
        let mut enc = png::Encoder::new(std::io::BufWriter::new(out), w, h + sh);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.add_text_chunk("XDFU".into(), text)
            .map_err(|e| io_err("cannot add the text chunk", e))?;
        let mut wr = enc
            .write_header()
            .map_err(|e| io_err("cannot write the png", e))?;
        wr.write_image_data(&[rgba, strip].concat())
            .map_err(|e| io_err("cannot write the png", e))?;
        wr.finish()
            .map_err(|e| io_err("cannot finish the png", e))?;
    }
    replace_with(&tmp, file)
}

/// Mark one output file in place.
fn mark(
    ctx: &JobCtx,
    file: &Path,
    n: usize,
    s: &Stamp,
    tools: &dyn ToolRunner,
    strips: &mut Strips,
) -> Result<(), Failure> {
    let ext = file
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "txt" | "md" | "csv" | "html" => stamp_text_file(file, &ext, &s.line()),
        "docx" | "xlsx" | "pptx" | "pdf" => stamp_with_tools(ctx, file, n, &ext, s, tools),
        "png" => stamp_png(ctx, file, n, s, tools, strips),
        other => fail(
            Exit::Unsupported,
            format!("XDFU watermark: a .{other} output cannot carry a watermark"),
        ),
    }
}

// ---- the hook ------------------------------------------------------------------------------------------------------------

#[derive(Default)]
struct Note {
    /// the input kind cannot be scanned: `after` looks at the outputs
    deferred: bool,
    evidence: Vec<Evidence>,
    copy: Option<u64>,
    refused: Option<&'static str>,
}

pub struct XdfuHook {
    mode: Mode,
    audit: Arc<AuditLog>,
    /// the CLI and the API's local mode: no authenticated user, so the OS user and this computer's address stand in
    local: bool,
    machine: String,
    jobs: Mutex<HashMap<String, Note>>,
}

impl XdfuHook {
    /// `local`: the process has no authenticated users (the CLI, or the API in local mode).
    pub fn new(mode: Mode, audit: Arc<AuditLog>, local: bool) -> XdfuHook {
        XdfuHook {
            mode,
            audit,
            local,
            machine: machine_addr(),
            jobs: Mutex::new(HashMap::new()),
        }
    }

    fn who(&self, ctx: &JobCtx) -> (String, String) {
        let user = match ctx.principal.as_deref() {
            Some(p) if !(self.local && p == "local") => p.to_string(),
            _ => os_user(),
        };
        let client = match ctx.client.as_deref() {
            Some(c)
                if !(self.local
                    && c.parse::<std::net::IpAddr>()
                        .is_ok_and(|ip| ip.is_loopback())) =>
            {
                c.to_string()
            }
            _ => self.machine.clone(),
        };
        (user, client)
    }

    fn event(&self, ctx: &JobCtx, user: &str, client: &str, evidence: &[Evidence]) -> Value {
        json!({
            "user": user, "client": client, "host": machine_name(), "mode": format!("{:?}", self.mode).to_lowercase(),
            "job": ctx.reference.clone().unwrap_or_else(|| ctx.job_id.clone()), "file": ctx.label,
            "kind": ctx.kind.map(Kind::ext), "target": ctx.target,
            "places": evidence.iter().map(|e| e.place).collect::<std::collections::BTreeSet<_>>(),
        })
    }

    /// Refuse the job: an audit line, the note for the report, and the failure.
    fn veto(
        &self,
        ctx: &JobCtx,
        evidence: Vec<Evidence>,
        why: &'static str,
        f: Failure,
    ) -> Result<Vec<Warning>, Failure> {
        let (user, client) = self.who(ctx);
        let mut e = self.event(ctx, &user, &client, &evidence);
        e["reason"] = why.into();
        self.audit.append("refused", e).map_err(|m| Failure {
            exit: Exit::ToolError,
            msg: format!("XDFU audit log: {m}"),
        })?;
        // nothing was found when the job is refused only because it could not be checked: no `xdfu` in the report (the schema says
        // "found the grif"), the audit line and the message carry it
        if !evidence.is_empty() {
            self.jobs.lock().unwrap().insert(
                ctx.job_id.clone(),
                Note {
                    evidence,
                    copy: None,
                    refused: Some(why),
                    deferred: false,
                },
            );
        }
        Err(f)
    }

    fn note_with<T>(&self, ctx: &JobCtx, f: impl FnOnce(&mut Note) -> T) -> Option<T> {
        self.jobs.lock().unwrap().get_mut(&ctx.job_id).map(f)
    }

    /// Every file of `in/`: a job with several inputs (PDF combine) is XDFU when any of them is.
    fn input_files(ctx: &JobCtx) -> Vec<(PathBuf, String)> {
        let mut v: Vec<(PathBuf, String)> = fs::read_dir(ctx.job_dir.join("in"))
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| (e.path(), format!("in/{}", e.file_name().to_string_lossy())))
            .collect();
        v.sort();
        v
    }
}

impl JobHook for XdfuHook {
    fn before(&self, ctx: &JobCtx, tools: &dyn ToolRunner) -> Result<Vec<Warning>, Failure> {
        if self.mode == Mode::Off {
            return Ok(vec![]);
        }
        // every input is looked at: a combine has several, and a job with no detected kind (a PDF operation) takes each file's kind from
        // its extension, so a grif in the second file is not missed
        let files = Self::input_files(ctx);
        let kinds: Vec<Option<Kind>> = files
            .iter()
            .map(|(p, _)| {
                ctx.kind.or_else(|| {
                    p.extension()
                        .and_then(|e| e.to_str())
                        .and_then(Kind::from_ext)
                })
            })
            .collect();
        if files.is_empty() || !kinds.iter().all(|k| scannable(*k)) {
            // the input cannot be read for a grif: check what the tool makes of it, if that can be read
            if !output_scannable(&ctx.target) {
                // no detected kind (PDF operations): name the staged input's extension, never an English word
                let what = ctx.kind.map(Kind::ext).unwrap_or_else(|| {
                    ctx.input
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("?")
                });
                return self.veto(
                    ctx,
                    vec![],
                    "cannot_check",
                    Failure {
                        exit: Exit::Unsupported,
                        msg: report::text_with(
                            "xdfu_cannot_check",
                            &[("kind", what), ("target", &ctx.target)],
                        ),
                    },
                );
            }
            self.jobs.lock().unwrap().insert(
                ctx.job_id.clone(),
                Note {
                    deferred: true,
                    ..Note::default()
                },
            );
            return Ok(vec![]);
        }
        let mut evidence = vec![];
        let mut unchecked = false;
        for (n, ((path, rel), kind)) in files.into_iter().zip(kinds).enumerate() {
            let (found, readable) = detect_checked(&path, &rel, kind, n, tools)?;
            unchecked |= !readable;
            evidence.extend(found);
        }
        if evidence.is_empty() {
            if unchecked {
                // an image-only PDF: nothing to read, which is not the same as "not marked"
                return self.veto(
                    ctx,
                    vec![],
                    "no_text_layer",
                    Failure {
                        exit: Exit::Unsupported,
                        msg: report::text("xdfu_no_text_layer"),
                    },
                );
            }
            return Ok(vec![]);
        }
        if self.mode == Mode::Refuse {
            return self.veto(
                ctx,
                evidence,
                "mode",
                Failure {
                    exit: Exit::Rejected,
                    msg: report::text("xdfu_refused"),
                },
            );
        }
        if !can_carry(&ctx.target) {
            return self.veto(
                ctx,
                evidence,
                "cannot_carry",
                Failure {
                    exit: Exit::Unsupported,
                    msg: report::text_with("xdfu_no_watermark", &[("target", &ctx.target)]),
                },
            );
        }
        self.jobs.lock().unwrap().insert(
            ctx.job_id.clone(),
            Note {
                evidence,
                ..Note::default()
            },
        );
        Ok(vec![])
    }

    fn after(
        &self,
        ctx: &JobCtx,
        outputs: &[PathBuf],
        tools: &dyn ToolRunner,
    ) -> Result<Vec<Warning>, Failure> {
        if self.note_with(ctx, |n| n.deferred) == Some(true) {
            // the input could not be scanned: look for the grif in what was made of it, before it is watermarked
            let mut found = vec![];
            for f in outputs {
                let ext = f
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if let (true, Some(kind)) = (
                    output_scannable(&ext),
                    Kind::ALL.into_iter().find(|k| k.ext() == ext),
                ) {
                    let rel = format!(
                        "out/{}",
                        f.file_name().unwrap_or_default().to_string_lossy()
                    );
                    found.extend(detect(f, &rel, Some(kind), tools)?);
                }
            }
            if found.is_empty() {
                // an ordinary document: the hook forgets the job
                self.jobs.lock().unwrap().remove(&ctx.job_id);
                return Ok(vec![]);
            }
            self.note_with(ctx, |n| {
                n.deferred = false;
                n.evidence = found.clone();
            });
            if self.mode == Mode::Refuse {
                return self.veto(
                    ctx,
                    found,
                    "mode",
                    Failure {
                        exit: Exit::Rejected,
                        msg: report::text("xdfu_refused"),
                    },
                );
            }
        }
        let Some(evidence) = self.note_with(ctx, |n| n.evidence.clone()) else {
            return Ok(vec![]);
        };
        let (user, client) = self.who(ctx);
        let mut e = self.event(ctx, &user, &client, &evidence);
        e["outputs"] = outputs.len().into();
        // the line is written after the marking has worked; a failed marking leaves a `copy_failed` line and no copy (gate review m12)
        let (copy, ()) = self
            .audit
            .append_after("copy", e, |copy| {
                self.note_with(ctx, |n| n.copy = Some(copy));
                let stamp = Stamp {
                    copy,
                    client,
                    user,
                    when: audit::utc_text(audit::now_secs()),
                };
                let mut strips = Strips(HashMap::new());
                for (n, f) in outputs.iter().enumerate() {
                    mark(ctx, f, n, &stamp, tools, &mut strips)?;
                }
                Ok::<(), Failure>(())
            })
            .map_err(|e| match e {
                audit::Appended::Log(m) => Failure {
                    exit: Exit::ToolError,
                    msg: format!("XDFU audit log: {m}"),
                },
                audit::Appended::Produce(f) => f,
            })?;
        Ok(vec![Warning {
            code: "xdfu_copy".into(),
            count: 1,
            text: report::text_with("xdfu_copy", &[("copy", &copy.to_string())]),
        }])
    }

    fn flagged(&self, ctx: &JobCtx) -> bool {
        self.jobs.lock().unwrap().contains_key(&ctx.job_id)
    }

    fn finish(&self, ctx: &JobCtx) -> Option<Value> {
        let n = self.jobs.lock().unwrap().remove(&ctx.job_id)?;
        Some(json!({
            "detected": true, "mode": format!("{:?}", self.mode).to_lowercase(), "copy": n.copy, "refused": n.refused,
            "evidence": n.evidence.iter().map(Evidence::to_json).collect::<Vec<_>>(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grif_is_found_in_all_three_languages_and_spacings_and_not_in_plain_text() {
        for (text, want) in [
            ("XDFU", "XDFU"),
            ("  xdfu  ", "XDFU"),
            ("Nusxa № 3, XDFU", "XDFU"),
            ("X D F U", "XDFU"),
            ("Nusxa a x d f u", "XDFU"),
            ("ХДФУ", "ХДФУ"),
            ("х.д.ф.у.", "ХДФУ"),
            ("Xдфу", "ХДФУ"),
            ("ДСП", "ДСП"),
            ("д с п", "ДСП"),
            ("дсп.", "ДСП"),
            ("Для служебного пользования", "Для служебного пользования"),
            (
                "ДЛЯ   СЛУЖЕБНОГО\nПОЛЬЗОВАНИЯ",
                "Для служебного пользования",
            ),
            (
                "Xizmat doirasida foydalanish uchun",
                "Xizmat doirasida foydalanish uchun",
            ),
            (
                "XIZMAT DOIRASIDA FOYDALANISH UCHUN",
                "Xizmat doirasida foydalanish uchun",
            ),
            (
                "Хизмат доирасида фойдаланиш учун",
                "Хизмат доирасида фойдаланиш учун",
            ),
        ] {
            assert_eq!(find_grif(text), Some(want), "{text:?}");
        }
        for plain in [
            "Salom dunyo, bu oddiy matn.",
            "Здравствуйте, это обычное письмо о служебной поездке.",
            "Оддий хат матни",
            "ДСПЕ плита",
            "xdfux",
            "служебного пользования",
            "для служебного",
            "",
        ] {
            assert_eq!(find_grif(plain), None, "{plain:?}");
        }
    }

    #[test]
    fn xml_helpers_read_runs_split_paragraphs_and_entities() {
        let xml = r#"<w:body><w:p><w:r><w:t>X</w:t></w:r><w:r><w:t xml:space="preserve">DFU</w:t></w:r></w:p><w:p><w:r><w:t>a &amp; b</w:t><w:tab/><w:t>c</w:t></w:r></w:p></w:body>"#;
        assert_eq!(
            paragraphs(xml, "w:p", "w:t", &["w:tab"]),
            ["XDFU", "a & b c"]
        );
        assert_eq!(
            find_grif(&paragraphs(xml, "w:p", "w:t", &[])[0]),
            Some("XDFU")
        );
        assert_eq!(decode("&#1044;&#x421;&#x41F; &lt;"), "ДСП <");
        assert_eq!(
            header_text("&LX&CДСП&R&\"Arial,Bold\"&12Page &P").trim(),
            "X ДСП Page P"
        );
        assert_eq!(
            plain("<a>x</a><b>y</b>")
                .split_whitespace()
                .collect::<Vec<_>>(),
            ["x", "y"]
        );
    }

    #[test]
    fn html_text_skips_the_head_and_reads_entities() {
        let h = "<html><head><title>XDFU in the title</title><style>p {x: 1}</style></head><body><p>X&amp;Y</p><script>var a = 'XDFU';</script><p>  second </p><br><b></b></body></html>";
        assert_eq!(html_lines(h), ["X&Y", "second"]);
        assert!(
            find_grif(&html_lines("<body><p class=a>ХДФУ</p><p>text</p>").join("\n")).is_some()
        );
        assert!(html_lines("plain text, no tags").len() == 1);
    }

    #[test]
    fn what_can_be_scanned_on_the_way_in_and_on_the_way_out() {
        for k in [
            Kind::Docx,
            Kind::Xlsx,
            Kind::Pptx,
            Kind::Pdf,
            Kind::Txt,
            Kind::Md,
            Kind::Csv,
            Kind::Tsv,
            Kind::Html,
            // read natively (odt, ods) or through a probe conversion (doc, rtf, xls, xlsb): gate review M4
            Kind::Doc,
            Kind::Rtf,
            Kind::Odt,
            Kind::Ods,
            Kind::Xls,
            Kind::Xlsb,
        ] {
            assert!(scannable(Some(k)), "{}", k.ext());
        }
        assert!(!scannable(None));
        // every output kind that is watermarked can be scanned, except the png (an image)
        for ext in ["txt", "md", "csv", "html", "docx", "xlsx", "pptx", "pdf"] {
            assert!(output_scannable(ext) && can_carry(ext), "{ext}");
        }
        assert!(can_carry("png") && !output_scannable("png"));
        assert!(!output_scannable("odt") && !output_scannable("rtf"));
    }

    #[test]
    fn the_stamp_lines() {
        let s = Stamp {
            copy: 17,
            client: "10.0.0.5".into(),
            user: "Алиса".into(),
            when: "2026-10-21 10:15:03 UTC".into(),
        };
        assert_eq!(
            s.line(),
            "XDFU copy 17 | 10.0.0.5 | Алиса | 2026-10-21 10:15:03 UTC"
        );
        let a = s.ascii_line();
        assert!(
            a.starts_with("XDFU copy 17 | 10.0.0.5 | _____~")
                && a.is_ascii()
                && a.ends_with("| 2026-10-21 10:15:03 UTC"),
            "{a}"
        );
        let plain = Stamp {
            user: "alice".into(),
            ..s
        };
        assert_eq!(plain.ascii_line(), plain.line());
    }

    #[test]
    fn text_outputs_get_a_first_line() {
        let d = std::env::temp_dir().join(format!("convert-xdfu-text-{}", std::process::id()));
        fs::create_dir_all(&d).unwrap();
        let line = "XDFU copy 1 | 10.0.0.5 | a\"b | 2026-10-21 10:15:03 UTC";
        for (ext, body, want) in [
            (
                "txt",
                "hello\n",
                "XDFU copy 1 | 10.0.0.5 | a\"b | 2026-10-21 10:15:03 UTC\nhello\n",
            ),
            (
                "md",
                "# T\n",
                "XDFU copy 1 | 10.0.0.5 | a\"b | 2026-10-21 10:15:03 UTC\n\n# T\n",
            ),
            (
                "csv",
                "a,b\n1,2\n",
                "\"XDFU copy 1 | 10.0.0.5 | a\"\"b | 2026-10-21 10:15:03 UTC\"\r\na,b\n1,2\n",
            ),
            (
                "html",
                "<html><BODY class=x><p>hi</p></BODY></html>",
                "<html><BODY class=x><p style=\"color:#b00;font-weight:bold\">XDFU copy 1 | 10.0.0.5 | a&quot;b | 2026-10-21 10:15:03 UTC</p><p>hi</p></BODY></html>",
            ),
        ] {
            let p = d.join(format!("o.{ext}"));
            fs::write(&p, body).unwrap();
            stamp_text_file(&p, ext, line).unwrap();
            assert_eq!(fs::read_to_string(&p).unwrap(), want, "{ext}");
        }
        // a BOM stays first
        let p = d.join("bom.txt");
        fs::write(&p, b"\xEF\xBB\xBFhi").unwrap();
        stamp_text_file(&p, "txt", "L").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"\xEF\xBB\xBFL\nhi");
        // html with no body is refused, not left unmarked
        fs::write(d.join("nb.html"), "<p>x</p>").unwrap();
        assert!(stamp_text_file(&d.join("nb.html"), "html", "L").is_err());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn what_can_carry_a_watermark() {
        for ext in [
            "txt", "md", "csv", "html", "docx", "xlsx", "pptx", "pdf", "png",
        ] {
            assert!(can_carry(ext), "{ext}");
        }
        for ext in ["odt", "rtf", "xls", "tsv"] {
            assert!(!can_carry(ext), "{ext}");
        }
    }

    // ---- gate review M3, M4 ----

    fn zip_of(name: &str, parts: &[(&str, &str)]) -> PathBuf {
        use std::io::Write;
        let p =
            std::env::temp_dir().join(format!("convert-xdfu-unit-{}-{name}", std::process::id()));
        let mut w = zip::ZipWriter::new(fs::File::create(&p).unwrap());
        for (n, body) in parts {
            w.start_file(*n, zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(body.as_bytes()).unwrap();
        }
        w.finish().unwrap();
        p
    }

    fn part(zip: &Path, name: &str) -> String {
        Package::open(zip).unwrap().text(name).unwrap_or_default()
    }

    fn para(t: &str) -> String {
        format!("<w:p><w:r><w:t>{t}</w:t></w:r></w:p>")
    }

    #[test]
    fn a_grif_in_the_right_cell_of_a_two_column_letterhead_is_found() {
        // the left cell has 30 lines: the grif in the right cell comes after the 25th paragraph in reading order
        let left: String = (1..=30)
            .map(|i| para(&format!("Address line {i}")))
            .collect();
        let doc = format!(
            "<w:document><w:body><w:tbl><w:tr><w:tc>{left}</w:tc><w:tc>{}</w:tc></w:tr></w:tbl>{}</w:body></w:document>",
            para("Для служебного пользования"),
            para("Dear sir")
        );
        let f = zip_of("letterhead.docx", &[("word/document.xml", &doc)]);
        let found = open_file(&f).and_then(detect_docx).unwrap();
        assert!(
            found.iter().any(|e| e.place == "first-page-top"),
            "{found:?}"
        );
        // the same words in a paragraph far below the top (no table) are not at the top of the page
        let body: String = (1..=30).map(|i| para(&format!("Line {i}"))).collect();
        let doc = format!(
            "<w:document><w:body>{body}{}</w:body></w:document>",
            para("ДСП")
        );
        let f = zip_of("deep.docx", &[("word/document.xml", &doc)]);
        assert!(open_file(&f).and_then(detect_docx).unwrap().is_empty());
        // a table that starts below the window is not read whole
        let doc = format!(
            "<w:document><w:body>{body}<w:tbl><w:tr><w:tc>{}</w:tc></w:tr></w:tbl></w:body></w:document>",
            para("ДСП")
        );
        let f = zip_of("deeptable.docx", &[("word/document.xml", &doc)]);
        assert!(open_file(&f).and_then(detect_docx).unwrap().is_empty());
    }

    #[test]
    fn odt_and_ods_are_read_natively_headers_and_properties_included() {
        let content = "<office:document-content><office:body><office:text><text:p>Plain letter</text:p></office:text></office:body></office:document-content>";
        let styles = "<office:document-styles><office:master-styles><style:master-page><style:header><text:p>Для служебного пользования</text:p></style:header></style:master-page></office:master-styles></office:document-styles>";
        let f = zip_of(
            "hdr.odt",
            &[("content.xml", content), ("styles.xml", styles)],
        );
        let found = open_file(&f).and_then(detect_odf).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(
            (found[0].place, found[0].part.as_str()),
            ("header", "styles.xml")
        );
        let body = "<office:document-content><text:p>X<text:s/>D</text:p><text:h>ХДФУ<text:span>!</text:span></text:h></office:document-content>";
        let meta = "<office:document-meta><office:meta><dc:subject>XDFU</dc:subject></office:meta></office:document-meta>";
        let f = zip_of("top.ods", &[("content.xml", body), ("meta.xml", meta)]);
        let places: Vec<_> = open_file(&f)
            .and_then(detect_odf)
            .unwrap()
            .iter()
            .map(|e| e.place)
            .collect();
        assert_eq!(places, ["first-page-top", "properties"]);
        let f = zip_of(
            "plain.odt",
            &[("content.xml", content), ("styles.xml", "<a/>")],
        );
        assert!(open_file(&f).and_then(detect_odf).unwrap().is_empty());
    }

    #[test]
    fn blank_shared_strings_do_not_shift_the_cell_indexes() {
        let wb = r#"<workbook><sheets><sheet name="S" r:id="rId1"/></sheets></workbook>"#;
        let rels = r#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>"#;
        let sst = "<sst><si><t></t></si><si><t>ДСП</t></si></sst>";
        let sheet = r#"<worksheet><sheetData><row r="1"><c r="A1" t="s"><v>1</v></c></row></sheetData></worksheet>"#;
        let f = zip_of(
            "sst.xlsx",
            &[
                ("xl/workbook.xml", wb),
                ("xl/_rels/workbook.xml.rels", rels),
                ("xl/sharedStrings.xml", sst),
                ("xl/worksheets/sheet1.xml", sheet),
            ],
        );
        assert_eq!(open_file(&f).and_then(detect_xlsx).unwrap().len(), 1);
    }

    #[test]
    fn the_docx_stamp_is_added_and_the_existing_header_and_footer_survive() {
        let doc = "<?xml version=\"1.0\"?><w:document xmlns:w=\"x\"><w:body><w:p><w:r><w:t>Body text</w:t></w:r></w:p></w:body></w:document>";
        let hdr = "<w:hdr xmlns:w=\"x\"><w:p><w:r><w:t>LETTERHEAD Acme &amp; Co</w:t></w:r></w:p></w:hdr>";
        let ftr = "<w:ftr xmlns:w=\"x\"/>";
        let src = zip_of(
            "in.docx",
            &[
                ("word/document.xml", doc),
                ("word/header1.xml", hdr),
                ("word/footer1.xml", ftr),
                ("word/media/a.png", "binary"),
            ],
        );
        let dst = src.with_extension("out.docx");
        stamp_docx_parts(&src, &dst, "XDFU copy 3 | 10.0.0.5 | a&b | now").unwrap();
        for n in ["word/document.xml", "word/header1.xml", "word/footer1.xml"] {
            assert!(
                part(&dst, n).contains("XDFU copy 3 | 10.0.0.5 | a&amp;b"),
                "{n}"
            );
        }
        assert!(part(&dst, "word/document.xml").contains("Body text"));
        let h = part(&dst, "word/header1.xml");
        assert!(
            h.contains("LETTERHEAD Acme &amp; Co")
                && h.find("XDFU copy").unwrap() < h.find("LETTERHEAD").unwrap(),
            "{h}"
        );
        assert!(part(&dst, "word/footer1.xml").ends_with("</w:ftr>"));
        assert_eq!(part(&dst, "word/media/a.png"), "binary");
        // a package with no body is refused, not left unmarked
        let bad = zip_of("bad.docx", &[("word/header1.xml", hdr)]);
        assert!(stamp_docx_parts(&bad, &bad.with_extension("o.docx"), "L").is_err());
    }

    #[test]
    fn the_sheet_stamp_merges_into_existing_headers_and_adds_missing_ones() {
        assert_eq!(merge_center("", "S"), "&CS");
        assert_eq!(merge_center("&LLeft&RRight", "S"), "&LLeft&RRight&CS");
        assert_eq!(
            merge_center("&LLeft&CMid&RRight", "S"),
            "&LLeft&CS\nMid&RRight"
        );
        assert_eq!(merge_center("&C&RRight", "S"), "&CS&RRight");
        assert_eq!(
            merge_center("&\"Arial,Bold\"&12Title", "S"),
            "&\"Arial,Bold\"&12Title&CS"
        );
        // none: a header and a footer are added
        let x =
            "<worksheet><sheetData/><pageMargins left=\"1\"/><drawing r:id=\"r1\"/></worksheet>";
        let out = stamp_sheet(x, "A & B").unwrap();
        assert!(out.contains("<headerFooter><oddHeader>&amp;CA &amp;&amp; B</oddHeader><oddFooter>&amp;CA &amp;&amp; B</oddFooter></headerFooter><drawing"), "{out}");
        // existing text stays; first-page and even-page headers get the stamp when the sheet uses them
        let x = "<worksheet><sheetData/><headerFooter differentFirst=\"1\"><oddHeader>&amp;LLetterhead&amp;RДСП</oddHeader><oddFooter>Page &amp;P</oddFooter></headerFooter><tableParts/></worksheet>";
        let out = stamp_sheet(x, "S").unwrap();
        assert!(
            out.contains("<oddHeader>&amp;LLetterhead&amp;RДСП&amp;CS</oddHeader>"),
            "{out}"
        );
        assert!(
            out.contains("<oddFooter>Page &amp;P&amp;CS</oddFooter>"),
            "{out}"
        );
        assert!(
            out.contains("<firstHeader>&amp;CS</firstHeader><firstFooter>&amp;CS</firstFooter>"),
            "{out}"
        );
        assert!(
            !out.contains("evenHeader") && out.ends_with("<tableParts/></worksheet>"),
            "{out}"
        );
        // an empty element
        assert!(
            stamp_sheet("<worksheet><sheetData/><headerFooter/></worksheet>", "S")
                .unwrap()
                .contains("<oddHeader>&amp;CS</oddHeader>")
        );
        // too long for Excel's 255 characters everywhere: an error, not a lost header
        let long = "x".repeat(250);
        let x = format!(
            "<worksheet><headerFooter><oddHeader>&amp;L{long}</oddHeader><oddFooter>&amp;L{long}</oddFooter></headerFooter></worksheet>"
        );
        assert!(stamp_sheet(&x, "XDFU copy 3 | 10.0.0.5 | alice | now").is_err());
    }

    #[test]
    fn the_pptx_stamp_is_a_new_shape_and_the_slides_own_footer_stays() {
        let pres = r#"<p:presentation><p:sldSz cx="9144000" cy="6858000"/></p:presentation>"#;
        let slide = r#"<p:sld><p:cSld><p:spTree><p:sp><p:nvSpPr><p:cNvPr id="7" name="Footer"/></p:nvSpPr><p:txBody><a:p><a:r><a:t>Our footer</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#;
        let src = zip_of(
            "in.pptx",
            &[
                ("ppt/presentation.xml", pres),
                ("ppt/slides/slide1.xml", slide),
            ],
        );
        let dst = src.with_extension("out.pptx");
        stamp_pptx_parts(&src, &dst, "XDFU copy 3").unwrap();
        let out = part(&dst, "ppt/slides/slide1.xml");
        assert!(
            out.contains("Our footer")
                && out.contains("<a:t>XDFU copy 3</a:t>")
                && out.contains(r#"<p:cNvPr id="8""#),
            "{out}"
        );
        assert!(out.contains(r#"y="6558000""#), "{out}");
        assert!(out.ends_with("</p:spTree></p:cSld></p:sld>"));
    }
}
