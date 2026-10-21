//! XDFU ("Xizmat doirasida foydalanish uchun") documents (PLAN 10, WP15). Opt-in and OFF by default: with `Mode::Off` no hook is
//! installed, so nothing is detected, nothing is logged and the output is byte for byte what it was before.
//!
//! With `Watermark` or `Refuse` the hook sees every job twice (`hook.rs`):
//!
//! 0. A kind that cannot be scanned (`doc`, `xls`, `xlsb`, `odt`, `ods`, `rtf`; `scannable`) is checked on the tool's OUTPUT instead: `before`
//!    only notes that the check is deferred, and `after` looks for the grif in the outputs (every output kind we watermark except `png` is
//!    scannable) before they are watermarked. If neither the input nor the output can be scanned, the job is refused (exit 3,
//!    `xdfu_cannot_check`): not knowing is not the same as "not marked".
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
//! - `docx`: a header, a footer, a diagonal watermark and a custom property `XDFU-copy`, with `docs-cli`;
//! - `xlsx`: a header and a footer on every sheet (visible in the page layout and on paper), with `sheets-cli`;
//! - `pptx`: a footer on every slide, with `slides-cli`;
//! - `txt`, `md`, `csv`, `html`: a first line (a quoted one-field row in csv; a paragraph after `<body>` in html);
//! - `odt` and `rtf` cannot carry one, so the job is refused before any tool runs.
//!
//! `shortcut: for an unscanned input kind the converted output is scanned, not the input, so a grif that the conversion itself drops or
//! changes (a header the writer does not keep) is missed; the first 8 paragraphs, rows or lines of the output are what is looked at.`
//! `shortcut: the grif is looked for by text in the top part of the first page, not by layout: the first 8 paragraphs of a docx,
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
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Off,
    Watermark,
    Refuse,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s {
            "off" => Ok(Mode::Off),
            "watermark" => Ok(Mode::Watermark),
            "refuse" => Ok(Mode::Refuse),
            other => Err(format!(
                "xdfu = \"{other}\": expected \"off\", \"watermark\" or \"refuse\""
            )),
        }
    }
}

// ---- finding the grif ----------------------------------------------------------------------------------------------------

/// Lowercase alphanumeric words of `text`: everything else separates them.
fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(String::from)
        .collect()
}

/// Latin letters that look like Cyrillic ones, folded to the Cyrillic letter (a grif typed with a Latin "x" or "c" is still a grif).
fn fold_cyrillic(w: &str) -> String {
    w.chars()
        .map(|c| match c {
            'a' => 'а',
            'c' => 'с',
            'e' => 'е',
            'k' => 'к',
            'm' => 'м',
            'o' => 'о',
            'p' => 'р',
            't' => 'т',
            'x' => 'х',
            'y' => 'у',
            c => c,
        })
        .collect()
}

const PHRASES: [(&str, &str); 3] = [
    ("для служебного пользования", "Для служебного пользования"),
    (
        "xizmat doirasida foydalanish uchun",
        "Xizmat doirasida foydalanish uchun",
    ),
    (
        "хизмат доирасида фойдаланиш учун",
        "Хизмат доирасида фойдаланиш учун",
    ),
];

/// The grif marker found in `text`, if any. Case and spacing do not matter: "xdfu", "X D F U", "Х.Д.Ф.У." and "ДСП" all match, as
/// do the phrase "для служебного пользования" and its Uzbek forms. A code must stand alone as a word (or as spaced single letters),
/// so "ДСПЕ" or "xdfux" do not match.
pub fn find_grif(text: &str) -> Option<&'static str> {
    let w = words(text);
    let codes = |s: &str| -> Option<&'static str> {
        match fold_cyrillic(s).as_str() {
            "хдфу" => Some("ХДФУ"),
            "дсп" => Some("ДСП"),
            _ if s == "xdfu" => Some("XDFU"),
            _ => None,
        }
    };
    // whole words
    if let Some(m) = w.iter().find_map(|x| codes(x)) {
        return Some(m);
    }
    // single letters in a row ("Х.Д.Ф.У.", "D S P"), also after another one-letter word ("a x d f u")
    let mut run: Vec<char> = vec![];
    for x in w.iter().map(Some).chain([None]) {
        match x {
            Some(x) if x.chars().count() == 1 => run.extend(x.chars()),
            _ => {
                for len in [3, 4] {
                    if let Some(m) = run
                        .windows(len)
                        .find_map(|win| codes(&win.iter().collect::<String>()))
                    {
                        return Some(m);
                    }
                }
                run.clear();
            }
        }
    }
    let joined = format!(" {} ", w.join(" "));
    PHRASES
        .iter()
        .find(|(p, _)| joined.contains(&format!(" {p} ")))
        .map(|(_, shown)| *shown)
}

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

struct Package(zip::ZipArchive<fs::File>);

impl Package {
    fn open(path: &Path) -> Result<Package, String> {
        let f = fs::File::open(path).map_err(|e| e.to_string())?;
        zip::ZipArchive::new(f)
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
fn properties(p: &mut Package, found: &mut Vec<Evidence>) {
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

const TOP_PARAGRAPHS: usize = 8;
const TOP_ROWS: usize = 5;

fn detect_docx(path: &Path) -> Result<Vec<Evidence>, String> {
    let mut p = Package::open(path)?;
    let mut found = vec![];
    if let Some(doc) = p.text("word/document.xml") {
        for t in paragraphs(&doc, "w:p", "w:t", &["w:tab", "w:br", "w:cr"])
            .iter()
            .take(TOP_PARAGRAPHS)
        {
            hit(&mut found, t, "first-page-top", "word/document.xml");
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

fn detect_xlsx(path: &Path) -> Result<Vec<Evidence>, String> {
    let mut p = Package::open(path)?;
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
    let shared: Vec<String> = p
        .text("xl/sharedStrings.xml")
        .map(|x| paragraphs(&x, "si", "t", &[]))
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

fn detect_pptx(path: &Path) -> Result<Vec<Evidence>, String> {
    let mut p = Package::open(path)?;
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

fn lines_hit(text: &str, place: &'static str, part: &str, found: &mut Vec<Evidence>) {
    for l in text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(TOP_PARAGRAPHS)
    {
        hit(found, l, place, part);
    }
}

fn detect_pdf(rel: &str, size_mb: f64, tools: &dyn ToolRunner) -> Result<Vec<Evidence>, Failure> {
    let mut found = vec![];
    let page1 = tools.run(
        ToolId::Pdf,
        &["text".into(), rel.into(), "--page".into(), "1".into()],
        size_mb,
    )?;
    lines_hit(
        &String::from_utf8_lossy(&page1),
        "first-page-top",
        "page 1",
        &mut found,
    );
    let info = tools.run(ToolId::Pdf, &["info".into(), rel.into()], size_mb)?;
    if let Ok(v) = serde_json::from_slice::<Value>(&info) {
        for k in ["title", "author", "subject", "keywords"] {
            if let Some(s) = v[k].as_str() {
                hit(&mut found, s, "properties", k);
            }
        }
    }
    Ok(found)
}

/// Kinds `detect` reads. A job whose input kind is not one of these is checked on its output (see the module doc).
pub fn scannable(kind: Option<Kind>) -> bool {
    matches!(
        kind,
        Some(
            Kind::Docx
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
    let unreadable = |e: String| Failure {
        exit: Exit::ToolError,
        msg: format!("the XDFU check could not read the input: {e}"),
    };
    let mb = fs::metadata(path)
        .map(|m| m.len() as f64 / 1_048_576.0)
        .unwrap_or(0.0);
    match kind {
        Some(Kind::Docx) => detect_docx(path).map_err(unreadable),
        Some(Kind::Xlsx) => detect_xlsx(path).map_err(unreadable),
        Some(Kind::Pptx) => detect_pptx(path).map_err(unreadable),
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
            Ok(found)
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
            Ok(found)
        }
        _ => Ok(vec![]),
    }
}

// ---- the watermark -------------------------------------------------------------------------------------------------------

/// Output kinds that can carry a watermark.
pub fn can_carry(ext: &str) -> bool {
    matches!(
        ext,
        "txt" | "md" | "csv" | "html" | "docx" | "xlsx" | "pptx" | "pdf" | "png"
    )
}

pub struct Stamp {
    pub copy: u64,
    pub client: String,
    pub user: String,
    pub when: String,
}

fn one_line(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { '_' } else { c })
        .take(64)
        .collect()
}

impl Stamp {
    /// `XDFU copy 17 | 10.0.0.5 | alice | 2026-10-21 10:15:03 UTC`
    pub fn line(&self) -> String {
        format!(
            "XDFU copy {} | {} | {} | {}",
            self.copy,
            one_line(&self.client),
            one_line(&self.user),
            self.when
        )
    }
    /// The same for outputs whose fonts cover Western European text only (pdf, png): anything else becomes `_`, and a user name
    /// that had to change gets a short digest so that two such names stay apart.
    pub fn ascii_line(&self) -> String {
        let a = |s: &str| -> String {
            one_line(s)
                .chars()
                .map(|c| {
                    if c.is_ascii_graphic() && !matches!(c, '<' | '>') || c == ' ' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect()
        };
        let user = a(&self.user);
        let user = if user == one_line(&self.user) {
            user
        } else {
            format!("{user}~{}", &digest(&self.user)[..6])
        };
        format!(
            "XDFU copy {} | {} | {} | {}",
            self.copy,
            a(&self.client),
            user,
            self.when
        )
    }
}

fn digest(s: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
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
    let bom: &[u8] = if old.starts_with(b"\xEF\xBB\xBF") {
        b"\xEF\xBB\xBF"
    } else {
        b""
    };
    let body = &old[bom.len()..];
    let mut new = bom.to_vec();
    match ext {
        "csv" => new.extend_from_slice(format!("\"{}\"\r\n", line.replace('"', "\"\"")).as_bytes()),
        "md" => new.extend_from_slice(format!("{line}\n\n").as_bytes()),
        "html" => {
            let text = String::from_utf8_lossy(body).into_owned();
            let lower = text.to_ascii_lowercase();
            let at = lower
                .find("<body")
                .and_then(|i| lower[i..].find('>').map(|j| i + j + 1))
                .ok_or_else(|| {
                    io_err(
                        "the html output has no <body>",
                        "refusing to leave it unmarked",
                    )
                })?;
            let mark = format!(
                "<p style=\"color:#b00;font-weight:bold\">{}</p>",
                html_escape(line)
            );
            new.extend_from_slice(format!("{}{}{}", &text[..at], mark, &text[at..]).as_bytes());
            return fs::write(path, new).map_err(|e| io_err("cannot write the output", e));
        }
        _ => new.extend_from_slice(format!("{line}\n").as_bytes()),
    }
    new.extend_from_slice(body);
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
                cmd("insert.header", json!({"text": line})),
                s_("--cmd"),
                cmd("insert.footer", json!({"text": line})),
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
        ),
        "pptx" => run(
            ToolId::Slides,
            vec![
                s_("run"),
                s_("--in"),
                rel_in,
                s_("--cmd"),
                cmd(
                    "design.headerFooter",
                    json!({"footer": true, "footerText": line, "all": true}),
                ),
                s_("--save"),
                rel_out.clone(),
            ],
        ),
        "xlsx" => {
            let info = tools.run(
                ToolId::Sheets,
                &[s_("info"), rel_in.clone(), s_("--json")],
                mb,
            )?;
            let v: Value = serde_json::from_slice(&info)
                .map_err(|e| io_err("sheets-cli info did not print JSON", e))?;
            let names: Vec<String> = v["sheets"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|x| x["name"].as_str().map(String::from))
                .collect();
            if names.is_empty() {
                return fail(
                    Exit::BadOutput,
                    "XDFU watermark: the workbook lists no sheets to mark",
                );
            }
            // `&` starts a code in a header, so a literal one is doubled
            let code = format!("&C{}", line.replace('&', "&&"));
            let mut argv = vec![s_("run"), s_("--in"), rel_in];
            for sheet in &names {
                argv.extend([
                    s_("--cmd"),
                    cmd(
                        "pageLayout.headerFooter",
                        json!({"sheet": sheet, "header": code, "footer": code}),
                    ),
                ]);
            }
            argv.extend([s_("--out"), rel_out.clone()]);
            run(ToolId::Sheets, argv)
        }
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

/// Where the CLI keeps the audit log when `--xdfu-audit` is not given: the user's data directory.
pub fn default_audit_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_DATA_HOME").map(PathBuf::from))
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
        });
    base.map_or_else(
        || PathBuf::from("xdfu-audit.jsonl"),
        |b| b.join("convert").join("xdfu-audit.jsonl"),
    )
}

pub fn os_user() -> String {
    ["USERNAME", "USER", "LOGNAME"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .unwrap_or_else(|| "unknown".into())
}

pub fn machine_name() -> String {
    ["COMPUTERNAME", "HOSTNAME"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .unwrap_or_else(|| "unknown".into())
}

/// The address this computer reaches the network with: the source address a UDP socket would use towards a public address (nothing
/// is sent). Falls back to the loopback address on a computer with no route.
pub fn machine_addr() -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| s.connect("192.0.2.1:9").and_then(|_| s.local_addr()))
        .map_or_else(|_| "127.0.0.1".into(), |a| a.ip().to_string())
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
        if !scannable(ctx.kind) {
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
        for (path, rel) in Self::input_files(ctx) {
            evidence.extend(detect(&path, &rel, ctx.kind, tools)?);
        }
        if evidence.is_empty() {
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
        let copy = self.audit.append("copy", e).map_err(|m| Failure {
            exit: Exit::ToolError,
            msg: format!("XDFU audit log: {m}"),
        })?;
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
        Ok(vec![Warning {
            code: "xdfu_copy".into(),
            count: 1,
            text: report::text_with("xdfu_copy", &[("copy", &copy.to_string())]),
        }])
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
        ] {
            assert!(scannable(Some(k)), "{}", k.ext());
        }
        for k in [
            Kind::Doc,
            Kind::Xls,
            Kind::Xlsb,
            Kind::Odt,
            Kind::Ods,
            Kind::Rtf,
        ] {
            assert!(!scannable(Some(k)), "{}", k.ext());
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
}
