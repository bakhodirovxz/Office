//! The planner: a pure, table-driven function `(kind, target, options) -> Vec<Step>`. Nothing runs here (PLAN 2.3).
//!
//! Paths in a plan are relative to the job directory and fixed (`in/input.<ext>`, `mid/..`, `out/..`), so no user string
//! reaches an argument or the JSON of a `--cmd`. The executor maps the real file names to these and back.
use crate::kind::{Family, Kind};
use serde_json::json;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ToolId {
    Docs,
    Sheets,
    Slides,
    Pdf,
}

impl ToolId {
    pub const ALL: [ToolId; 4] = [ToolId::Docs, ToolId::Sheets, ToolId::Slides, ToolId::Pdf];
    pub fn name(self) -> &'static str {
        match self {
            ToolId::Docs => "docs-cli",
            ToolId::Sheets => "sheets-cli",
            ToolId::Slides => "slides-cli",
            ToolId::Pdf => "pdf-cli",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Target {
    Docx,
    Odt,
    Rtf,
    Html,
    Md,
    Txt,
    Pdf,
    Png,
    Xlsx,
    Csv,
    Pptx,
}

impl Target {
    pub const ALL: [Target; 11] = [
        Target::Docx,
        Target::Odt,
        Target::Rtf,
        Target::Html,
        Target::Md,
        Target::Txt,
        Target::Pdf,
        Target::Png,
        Target::Xlsx,
        Target::Csv,
        Target::Pptx,
    ];
    pub fn ext(self) -> &'static str {
        match self {
            Target::Docx => "docx",
            Target::Odt => "odt",
            Target::Rtf => "rtf",
            Target::Html => "html",
            Target::Md => "md",
            Target::Txt => "txt",
            Target::Pdf => "pdf",
            Target::Png => "png",
            Target::Xlsx => "xlsx",
            Target::Csv => "csv",
            Target::Pptx => "pptx",
        }
    }
    pub fn parse(s: &str) -> Option<Target> {
        Target::ALL
            .into_iter()
            .find(|t| t.ext() == s.to_ascii_lowercase())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranslitDir {
    Cyrl,
    Latn,
    Auto,
}

/// The Latin alphabet when a job does not name one: follows `uz_translit::DEFAULT_LATIN_ALPHABET` (owner decision O13).
pub const DEFAULT_ALPHABET: &str = uz_translit::DEFAULT_LATIN_ALPHABET.name();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Translit {
    pub dir: TranslitDir,
    /// `"1995" | "2026" | "auto"`
    pub alphabet: &'static str,
    /// `"official" | "ascii"`
    pub apostrophe: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum SheetSel {
    #[default]
    Active,
    All,
    Name(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PdfEngine {
    Sidecar,
    #[default]
    Native,
}

/// The language numbers and dates are read and written in (gridcraft `NumLocale`). Sent to the tools as `GRIDCRAFT_NUM_LOCALE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum NumLocale {
    #[default]
    EnUs,
    Ru,
    UzLatn,
    UzCyrl,
}

impl NumLocale {
    /// A UI language or an OS locale name: `en-US`, `ru`, `ru-RU`, `uz-Latn`, `uz-Cyrl-UZ`, `uz_UZ.UTF-8`, `en_GB`. `None` = not one of ours.
    pub fn parse(s: &str) -> Option<NumLocale> {
        let t = s.trim().to_ascii_lowercase().replace('_', "-");
        let t = t.split(['.', '@']).next().unwrap_or("");
        let mut p = t.split('-');
        match (p.next()?, p.next()) {
            ("en", _) => Some(NumLocale::EnUs),
            ("ru", _) => Some(NumLocale::Ru),
            ("uz", Some("cyrl")) => Some(NumLocale::UzCyrl),
            ("uz", _) => Some(NumLocale::UzLatn),
            _ => None,
        }
    }
    /// The value of `GRIDCRAFT_NUM_LOCALE`.
    pub fn tag(self) -> &'static str {
        match self {
            NumLocale::EnUs => "en",
            NumLocale::Ru => "ru",
            NumLocale::UzLatn => "uz-latn",
            NumLocale::UzCyrl => "uz-cyrl",
        }
    }
}

/// `1,3-5`: ranges of 1-based pages, in the order given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageSpec(pub Vec<(u32, u32)>);

impl PageSpec {
    pub fn parse(s: &str) -> Result<PageSpec, String> {
        let mut v = Vec::new();
        for part in s.split(',') {
            let (a, b) = match part.split_once('-') {
                Some((a, b)) => (a, b),
                None => (part, part),
            };
            let n = |x: &str| x.parse::<u32>().ok().filter(|n| *n >= 1);
            match (n(a), n(b)) {
                (Some(a), Some(b)) if a <= b => v.push((a, b)),
                _ => {
                    return Err(format!(
                        "bad page list '{s}': use numbers and ranges like 1,3-5 (pages start at 1)"
                    ));
                }
            }
        }
        Ok(PageSpec(v))
    }
    pub fn canonical(&self) -> String {
        self.0
            .iter()
            .map(|&(a, b)| {
                if a == b {
                    a.to_string()
                } else {
                    format!("{a}-{b}")
                }
            })
            .collect::<Vec<_>>()
            .join(",")
    }
    /// The pages in order; `Err` when one lies beyond `total`.
    pub fn pages(&self, total: u32) -> Result<Vec<u32>, String> {
        let mut out = Vec::new();
        for &(a, b) in &self.0 {
            if b > total {
                return Err(format!("page {b} is beyond the last page ({total})"));
            }
            out.extend(a..=b);
        }
        Ok(out)
    }
}

#[derive(Clone, Debug)]
pub struct Options {
    pub translit: Option<Translit>,
    pub sheet: SheetSel,
    pub pages: Option<PageSpec>,
    pub dpi: u32,
    pub pdf_engine: PdfEngine,
    pub locale: NumLocale,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            translit: None,
            sheet: SheetSel::Active,
            pages: None,
            dpi: 150,
            pdf_engine: PdfEngine::Native,
            locale: NumLocale::EnUs,
        }
    }
}

/// What a step must have written. Checked by the executor before the next step starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outputs {
    Files(Vec<String>),
    /// at least one file with this extension in the directory; exactly `expect` of them when the plan knows the count
    Dir {
        dir: String,
        ext: String,
        expect: Option<usize>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolStep {
    pub tool: ToolId,
    pub argv: Vec<String>,
    pub outputs: Outputs,
    /// the tool's stdout is the output (pdf-cli text)
    pub stdout_to: Option<String>,
    /// an empty output is not a failure but a warning (a scanned PDF has no text)
    pub allow_empty: bool,
    /// files the planner generated and the executor writes before the step (a `pdf-cli run --script`)
    pub write_files: Vec<(String, String)>,
}

/// A step whose shape depends on what an earlier step produced. The executor runs `info`, then calls `expand_*`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expand {
    /// `sheets-cli info <input> --json`, then one `convert` per sheet, or only the sheet `only` (checked against the names)
    CsvSheets { input: String, only: Option<String> },
    /// `pdf-cli info <pdf>`, then one `pdf-cli run --script` that renders the pages
    PdfPages {
        pdf: String,
        spec: Option<PageSpec>,
        dpi: u32,
    },
    /// `slides-cli info <deck>`, then `render --all`, or one `render --slide N` per chosen slide
    SlidePages {
        deck: String,
        spec: Option<PageSpec>,
        dpi: u32,
    },
    /// `pdf-cli info <pdf>`, then `split` with the exact number of parts known
    PdfSplit { pdf: String, by: SplitBy },
    /// `pdf-cli info <pdf>` and nothing else: what the tool could not read (a cut-off file, a damaged cross-reference table) becomes a
    /// `pdf_damaged` warning before the text or the docx is made from the file
    PdfCheck { pdf: String },
}

/// What an expansion produced: the steps, and warnings the planner knows about before anything runs (hidden sheets).
#[derive(Debug, PartialEq, Eq)]
pub struct Expanded {
    pub steps: Vec<Step>,
    pub warnings: Vec<(&'static str, u32)>,
}

/// Limits that keep one document from fanning out into thousands of processes or files (review M8). Over a limit: exit 9.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caps {
    /// pages of a PDF, slides of a deck
    pub max_pages_in: u32,
    /// PNG files (or split parts) one job may write
    pub max_png_out: u32,
    pub max_sheets: u32,
}

impl Default for Caps {
    fn default() -> Self {
        Caps {
            max_pages_in: 500,
            max_png_out: 200,
            max_sheets: 50,
        }
    }
}

/// One POST to a sidecar (PLAN 2.3): the file `input` in the job directory is the body, the answer is written to `output`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpStep {
    pub sidecar: Sidecar,
    pub route: String,
    pub query: Vec<(String, String)>,
    pub input: String,
    pub output: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sidecar {
    Pdf2Docx,
}

/// After a native pdf to docx (D5): the letters of the PDF text layer against the letters of `docx`. A shortfall over the threshold
/// is redone by `fallback` (the pdf2docx sidecar) when it is configured and answers, and warned about otherwise.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PdfTextGuard {
    pub pdf: String,
    pub docx: String,
    pub fallback: HttpStep,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Tool(ToolStep),
    Http(HttpStep),
    Expand(Expand),
    PdfTextGuard(PdfTextGuard),
}

#[derive(Debug, PartialEq, Eq)]
pub enum PlanError {
    /// exit 3
    Unsupported(String),
    /// exit 7: the direction exists but a sidecar is needed and not wired in yet
    NeedsSidecar(String),
    /// exit 2
    Usage(String),
    /// exit 9: over a limit
    Rejected(String),
}

pub fn input_path(kind: Kind) -> String {
    format!("in/input.{}", kind.ext())
}

fn tool(tool: ToolId, argv: &[&str], out: &str) -> ToolStep {
    ToolStep {
        tool,
        argv: argv.iter().map(|s| s.to_string()).collect(),
        outputs: Outputs::Files(vec![out.to_string()]),
        stdout_to: None,
        allow_empty: false,
        write_files: vec![],
    }
}

fn cmd_arg(id: &str, params: serde_json::Value) -> String {
    format!("{id}={params}")
}

fn translit_cmd(t: &Translit, scope: &str) -> String {
    let id = match t.dir {
        TranslitDir::Cyrl => "tools.translitCyrl",
        TranslitDir::Latn => "tools.translitLatn",
        TranslitDir::Auto => "tools.translitAuto",
    };
    cmd_arg(
        id,
        json!({"scope": scope, "latinAlphabet": t.alphabet, "apostrophe": t.apostrophe}),
    )
}

/// Every (kind, target) pair the planner knows, with its first step's tool. This is the table `formats` prints.
pub fn rows() -> Vec<(Kind, Target)> {
    let mut v = Vec::new();
    for k in Kind::ALL {
        for t in Target::ALL {
            if route(k, t).is_some() {
                v.push((k, t));
            }
        }
    }
    v
}

/// The table: does a route exist without a sidecar? (`.ppt`, `.odp` and the other direct gaps have none.)
fn route(k: Kind, t: Target) -> Option<()> {
    use Target::*;
    let ok = match k.family() {
        Family::Docs => matches!(t, Docx | Odt | Rtf | Html | Md | Txt | Pdf | Png),
        Family::Sheets => matches!(t, Xlsx | Csv | Pdf),
        Family::Slides => k == Kind::Pptx && matches!(t, Pptx | Pdf | Png),
        Family::Pdf => matches!(t, Png | Txt | Docx),
    };
    ok.then_some(())
}

/// Options that make sense for a row (for `formats --json`; the API and the UI read it).
pub fn row_options(k: Kind, t: Target) -> Vec<&'static str> {
    let mut o = vec![];
    let translit = match k.family() {
        Family::Docs => matches!(
            t,
            Target::Docx
                | Target::Odt
                | Target::Rtf
                | Target::Html
                | Target::Md
                | Target::Txt
                | Target::Pdf
                | Target::Png
        ),
        Family::Sheets => matches!(t, Target::Xlsx | Target::Pdf),
        Family::Slides => true,
        Family::Pdf => t == Target::Docx,
    };
    if translit {
        o.extend(["translit", "alphabet", "apostrophe"]);
    }
    if t == Target::Csv {
        o.push("sheet");
    }
    if t == Target::Png {
        o.extend(["pages", "dpi"]);
    }
    if k == Kind::Pdf && t == Target::Txt {
        o.push("pages");
    }
    if k == Kind::Pdf && t == Target::Docx {
        o.push("pdf-engine");
    }
    o
}

pub fn plan(kind: Kind, target: Target, o: &Options) -> Result<Vec<Step>, PlanError> {
    // owner decision O2: legacy .ppt is out of scope (no sidecar); .odp has no reader either
    if matches!(kind, Kind::Ppt | Kind::Odp) {
        return Err(PlanError::Unsupported(format!(
            ".{} files are not supported: re-save the file as .pptx in PowerPoint (or Impress) and convert that",
            kind.ext()
        )));
    }
    if route(kind, target).is_none() {
        return Err(PlanError::Unsupported(format!(
            "no converter writes .{} from .{} files",
            target.ext(),
            kind.ext()
        )));
    }
    let inp = input_path(kind);
    let want_translit = o.translit.is_some();
    if want_translit && !row_options(kind, target).contains(&"translit") {
        return Err(PlanError::Unsupported(format!(
            "--translit is not available for .{} to .{}",
            kind.ext(),
            target.ext()
        )));
    }
    if o.sheet != SheetSel::Active && target != Target::Csv {
        return Err(PlanError::Usage("--sheet only applies to --to csv".into()));
    }
    if o.pages.is_some() && !(target == Target::Png || (kind == Kind::Pdf && target == Target::Txt))
    {
        return Err(PlanError::Usage(
            "--pages only applies to png output and pdf to txt".into(),
        ));
    }
    let tr = o.translit.as_ref();
    Ok(match kind.family() {
        Family::Docs => docs(&inp, target, o, tr),
        Family::Sheets => sheets(&inp, target, o, tr)?,
        Family::Slides => slides(&inp, target, o, tr),
        Family::Pdf => pdf(&inp, target, o, tr)?,
    })
}

fn pages_png(pdf: &str, o: &Options) -> Step {
    Step::Expand(Expand::PdfPages {
        pdf: pdf.into(),
        spec: o.pages.clone(),
        dpi: o.dpi,
    })
}

fn docs(inp: &str, t: Target, o: &Options, tr: Option<&Translit>) -> Vec<Step> {
    let dest = if t == Target::Png {
        "mid/doc.pdf".to_string()
    } else {
        format!("out/output.{}", t.ext())
    };
    let first = match tr {
        None => tool(ToolId::Docs, &["convert", inp, &dest], &dest),
        Some(tr) => tool(
            ToolId::Docs,
            &[
                "run",
                "--file",
                inp,
                "--cmd",
                &translit_cmd(tr, "document"),
                "--print",
                "--save",
                &dest,
            ],
            &dest,
        ),
    };
    let mut v = vec![Step::Tool(first)];
    if t == Target::Png {
        v.push(pages_png(&dest, o));
    }
    v
}

fn sheets(
    inp: &str,
    t: Target,
    o: &Options,
    tr: Option<&Translit>,
) -> Result<Vec<Step>, PlanError> {
    Ok(match t {
        Target::Xlsx => match tr {
            None => vec![Step::Tool(tool(
                ToolId::Sheets,
                &["convert", inp, "out/output.xlsx"],
                "out/output.xlsx",
            ))],
            Some(tr) => vec![Step::Tool(tool(
                ToolId::Sheets,
                &[
                    "run",
                    "--in",
                    inp,
                    "--cmd",
                    &translit_cmd(tr, "workbook"),
                    "--out",
                    "out/output.xlsx",
                ],
                "out/output.xlsx",
            ))],
        },
        Target::Pdf => {
            let export = cmd_arg(
                "file.exportPdf",
                json!({"path": "out/output.pdf", "sheets": "all"}),
            );
            let mut argv = vec!["run", "--in", inp];
            let tc;
            if let Some(tr) = tr {
                tc = translit_cmd(tr, "workbook");
                argv.extend(["--cmd", &tc]);
            }
            argv.extend(["--cmd", &export]);
            vec![Step::Tool(tool(ToolId::Sheets, &argv, "out/output.pdf"))]
        }
        Target::Csv => match &o.sheet {
            SheetSel::Active => vec![Step::Tool(tool(
                ToolId::Sheets,
                &["convert", inp, "out/output.csv"],
                "out/output.csv",
            ))],
            SheetSel::Name(n) => vec![Step::Expand(Expand::CsvSheets {
                input: inp.into(),
                only: Some(n.clone()),
            })],
            SheetSel::All => vec![Step::Expand(Expand::CsvSheets {
                input: inp.into(),
                only: None,
            })],
        },
        _ => unreachable!("route() admitted only xlsx, csv, pdf"),
    })
}

fn slides(inp: &str, t: Target, o: &Options, tr: Option<&Translit>) -> Vec<Step> {
    // with --translit the first step is a `run` that saves the converted deck, then the plain route continues from it
    let (src, mut v) = match tr {
        None => (inp.to_string(), vec![]),
        Some(tr) if t == Target::Pptx || t == Target::Pdf => {
            let dest = format!("out/output.{}", t.ext());
            return vec![Step::Tool(tool(
                ToolId::Slides,
                &[
                    "run",
                    "--in",
                    inp,
                    "--cmd",
                    &translit_cmd(tr, "deck"),
                    "--print",
                    "--save",
                    &dest,
                ],
                &dest,
            ))];
        }
        Some(tr) => (
            "mid/deck.pptx".to_string(),
            vec![Step::Tool(tool(
                ToolId::Slides,
                &[
                    "run",
                    "--in",
                    inp,
                    "--cmd",
                    &translit_cmd(tr, "deck"),
                    "--print",
                    "--save",
                    "mid/deck.pptx",
                ],
                "mid/deck.pptx",
            ))],
        ),
    };
    match t {
        Target::Png => v.push(Step::Expand(Expand::SlidePages {
            deck: src,
            spec: o.pages.clone(),
            dpi: o.dpi,
        })),
        _ => {
            let dest = format!("out/output.{}", t.ext());
            v.push(Step::Tool(tool(
                ToolId::Slides,
                &["convert", &src, &dest],
                &dest,
            )));
        }
    }
    v
}

fn pdf(inp: &str, t: Target, o: &Options, tr: Option<&Translit>) -> Result<Vec<Step>, PlanError> {
    Ok(match t {
        Target::Png => vec![pages_png(inp, o)],
        Target::Txt => {
            let mut argv = vec!["text", inp];
            let page;
            if let Some(p) = &o.pages {
                if p.0.len() != 1 || p.0[0].0 != p.0[0].1 {
                    return Err(PlanError::Usage(
                        "pdf to txt takes one page (--pages N), or none for all".into(),
                    ));
                }
                page = p.0[0].0.to_string();
                argv.extend(["--page", &page]);
            }
            let mut st = tool(ToolId::Pdf, &argv, "out/output.txt");
            st.stdout_to = Some("out/output.txt".into());
            st.allow_empty = true;
            vec![
                Step::Expand(Expand::PdfCheck { pdf: inp.into() }),
                Step::Tool(st),
            ]
        }
        Target::Docx => {
            // with --translit the pdf -> docx step writes a middle file and docs-cli converts it (PLAN 4)
            let first = if tr.is_some() {
                "mid/pdf.docx"
            } else {
                "out/output.docx"
            };
            let side = HttpStep {
                sidecar: Sidecar::Pdf2Docx,
                route: "/v1/pdf2docx".into(),
                query: vec![("merge".into(), "1".into())],
                input: inp.into(),
                output: first.into(),
            };
            let mut v = match o.pdf_engine {
                PdfEngine::Sidecar => vec![Step::Http(side.clone())],
                PdfEngine::Native => {
                    // doc id 1: a fresh `run` process opens exactly one document
                    let script = json!([
                        {"tool": "doc_open", "args": {"path": inp}},
                        {"tool": "doc_export_office", "args": {"doc": 1, "path": first}}
                    ]);
                    let mut st = tool(
                        ToolId::Pdf,
                        &["run", "--script", "mid/native.json", "--root", "."],
                        first,
                    );
                    st.write_files = vec![("mid/native.json".into(), script.to_string())];
                    vec![
                        Step::Expand(Expand::PdfCheck { pdf: inp.into() }),
                        Step::Tool(st),
                        Step::PdfTextGuard(PdfTextGuard {
                            pdf: inp.into(),
                            docx: first.into(),
                            fallback: side,
                        }),
                    ]
                }
            };
            if let Some(tr) = tr {
                v.push(Step::Tool(tool(
                    ToolId::Docs,
                    &[
                        "run",
                        "--file",
                        first,
                        "--cmd",
                        &translit_cmd(tr, "document"),
                        "--print",
                        "--save",
                        "out/output.docx",
                    ],
                    "out/output.docx",
                )));
            }
            v
        }
        _ => unreachable!("route() admitted only png, txt, docx"),
    })
}

fn count(info: &serde_json::Value, key: &str, what: &str) -> Result<u32, PlanError> {
    let n = match info.get(key) {
        Some(v) if v.is_array() => v.as_array().map(|a| a.len() as u64),
        Some(v) => v.as_u64(),
        None => None,
    };
    n.filter(|&n| n >= 1)
        .map(|n| n as u32)
        .ok_or_else(|| PlanError::Unsupported(format!("the tool's info reported no {what}")))
}

fn chosen(
    spec: &Option<PageSpec>,
    total: u32,
    caps: &Caps,
    what: &str,
) -> Result<Vec<u32>, PlanError> {
    if total > caps.max_pages_in {
        return Err(PlanError::Rejected(format!(
            "the file has {total} {what}, over the limit of {}",
            caps.max_pages_in
        )));
    }
    let pages = match spec {
        Some(s) => s.pages(total).map_err(PlanError::Usage)?,
        None => (1..=total).collect(),
    };
    if pages.len() as u32 > caps.max_png_out {
        return Err(PlanError::Rejected(format!(
            "{} images were asked for, over the limit of {}",
            pages.len(),
            caps.max_png_out
        )));
    }
    Ok(pages)
}

/// The pages of a PDF as PNGs, in ONE `pdf-cli run --script` (census F4: 108 pages in 12-16 s in one process, against a
/// reopen of the whole PDF per page).
pub fn expand_pages(
    pdf: &str,
    spec: &Option<PageSpec>,
    dpi: u32,
    info: &serde_json::Value,
    caps: &Caps,
) -> Result<Expanded, PlanError> {
    let total = count(info, "pages", "pages")?;
    let pages = chosen(spec, total, caps, "pages")?;
    let mut script = vec![json!({"tool": "doc_open", "args": {"path": pdf}})];
    for p in &pages {
        script.push(json!({"tool": "page_render", "args": {"doc": 1, "page": p, "dpi": dpi}, "out": format!("out/p{p:02}.png")}));
    }
    let mut st = tool(
        ToolId::Pdf,
        &["run", "--script", "mid/pages.json", "--root", "."],
        "out",
    );
    st.outputs = Outputs::Dir {
        dir: "out".into(),
        ext: "png".into(),
        expect: Some(pages.len()),
    };
    st.write_files = vec![(
        "mid/pages.json".into(),
        serde_json::Value::Array(script).to_string(),
    )];
    Ok(Expanded {
        steps: vec![Step::Tool(st)],
        warnings: vec![],
    })
}

/// Slides as PNGs: all of them in one `render --all`, or the chosen ones one `render --slide N` each. The count is known from
/// `info`, so a render that stops early is exit 8, and `--pages` is honoured.
pub fn expand_slides(
    deck: &str,
    spec: &Option<PageSpec>,
    dpi: u32,
    info: &serde_json::Value,
    caps: &Caps,
) -> Result<Expanded, PlanError> {
    let total = count(info, "slides", "slides")?;
    let pages = chosen(spec, total, caps, "slides")?;
    // deckcraft takes a scale factor (2 = 1920 px wide); 150 dpi maps to 2
    let scale = format!("{:.2}", dpi as f32 / 75.0);
    let steps = if spec.is_none() {
        let mut st = tool(
            ToolId::Slides,
            &["render", deck, "--all", "out", "--scale", &scale],
            "out",
        );
        st.outputs = Outputs::Dir {
            dir: "out".into(),
            ext: "png".into(),
            expect: Some(pages.len()),
        };
        vec![Step::Tool(st)]
    } else {
        pages
            .iter()
            .map(|p| {
                let out = format!("out/slide-{p:02}.png");
                Step::Tool(tool(
                    ToolId::Slides,
                    &[
                        "render",
                        deck,
                        "--slide",
                        &(p - 1).to_string(),
                        "--scale",
                        &scale,
                        &out,
                    ],
                    &out,
                ))
            })
            .collect()
    };
    Ok(Expanded {
        steps,
        warnings: vec![],
    })
}

/// `pdf split` with the number of parts known from the page count.
pub fn expand_split(
    input: &str,
    by: &SplitBy,
    info: &serde_json::Value,
    caps: &Caps,
) -> Result<Expanded, PlanError> {
    let total = count(info, "pages", "pages")?;
    if total > caps.max_pages_in {
        return Err(PlanError::Rejected(format!(
            "the PDF has {total} pages, over the limit of {}",
            caps.max_pages_in
        )));
    }
    let mut argv = vec!["split".to_string(), input.into()];
    let parts = match by {
        SplitBy::Every(n) => {
            argv.extend(["--every".into(), n.to_string()]);
            total.div_ceil(*n) as usize
        }
        SplitBy::Before(v) => {
            if let Some(bad) = v.iter().find(|&&b| b < 2 || b > total) {
                return Err(PlanError::Usage(format!(
                    "cannot split before page {bad}: the PDF has {total} pages (split points are 2 to {total})"
                )));
            }
            argv.extend([
                "--before".into(),
                v.iter().map(u32::to_string).collect::<Vec<_>>().join(","),
            ]);
            let mut u = v.clone();
            u.sort_unstable();
            u.dedup();
            u.len() + 1
        }
    };
    argv.extend(["--out-dir".into(), "out".into()]);
    if parts as u32 > caps.max_png_out {
        return Err(PlanError::Rejected(format!(
            "the split would write {parts} files, over the limit of {}",
            caps.max_png_out
        )));
    }
    Ok(Expanded {
        steps: vec![Step::Tool(ToolStep {
            tool: ToolId::Pdf,
            argv,
            outputs: Outputs::Dir {
                dir: "out".into(),
                ext: "pdf".into(),
                expect: Some(parts),
            },
            stdout_to: None,
            allow_empty: false,
            write_files: vec![],
        })],
        warnings: vec![],
    })
}

/// One `convert ... .csv --sheet NAME` per sheet of `sheets-cli info --json` (`only`: just that sheet, written as
/// `out/output.csv`, and an unknown name is a usage error that lists the real ones). Internal file names are `sNN-<safe name>`:
/// the index keeps them unique, and `safe_name` keeps document content from choosing a device name (review m7). Hidden sheets
/// are exported with a warning; very hidden ones are skipped with a warning.
pub fn expand_csv(
    input: &str,
    only: Option<&str>,
    info: &serde_json::Value,
    caps: &Caps,
) -> Result<Expanded, PlanError> {
    let sheets: Vec<(&str, &str)> = info
        .get("sheets")
        .and_then(|s| s.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|s| {
                    Some((
                        s.get("name")?.as_str()?,
                        s.get("visibility")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Visible"),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    if sheets.is_empty() {
        return Err(PlanError::Unsupported(
            "sheets-cli info listed no sheets".into(),
        ));
    }
    if sheets.len() as u32 > caps.max_sheets {
        return Err(PlanError::Rejected(format!(
            "the workbook has {} sheets, over the limit of {}",
            sheets.len(),
            caps.max_sheets
        )));
    }
    let names: Vec<&str> = sheets.iter().map(|s| s.0).collect();
    if let Some(n) = only {
        if !names.contains(&n) {
            return Err(PlanError::Usage(format!(
                "no sheet named '{n}'; the workbook has: {}",
                names.join(", ")
            )));
        }
        if n.starts_with('-') {
            return Err(PlanError::Unsupported(format!(
                "sheet name '{n}' starts with '-' and cannot be passed to the tool"
            )));
        }
        return Ok(Expanded {
            steps: vec![Step::Tool(tool(
                ToolId::Sheets,
                &["convert", input, "out/output.csv", "--sheet", n],
                "out/output.csv",
            ))],
            warnings: vec![],
        });
    }
    let (mut hidden, mut skipped) = (0u32, 0u32);
    let mut v = vec![];
    for (i, (n, vis)) in sheets.iter().enumerate() {
        match *vis {
            "VeryHidden" => {
                skipped += 1;
                continue;
            }
            "Hidden" => hidden += 1,
            _ => {}
        }
        if n.starts_with('-') {
            return Err(PlanError::Unsupported(format!(
                "sheet name '{n}' starts with '-' and cannot be passed to the tool"
            )));
        }
        let out = format!(
            "out/s{:02}-{}.csv",
            i + 1,
            crate::names::safe_name(n, "sheet")
        );
        v.push(Step::Tool(tool(
            ToolId::Sheets,
            &["convert", input, &out, "--sheet", n],
            &out,
        )));
    }
    if v.is_empty() {
        return Err(PlanError::Unsupported(
            "every sheet of the workbook is hidden".into(),
        ));
    }
    let mut warnings = vec![];
    if hidden > 0 {
        warnings.push(("hidden_sheet_exported", hidden));
    }
    if skipped > 0 {
        warnings.push(("very_hidden_sheet_skipped", skipped));
    }
    Ok(Expanded { steps: v, warnings })
}

/// The PDF subcommands (no kind detection, the inputs are PDFs by contract).
pub fn pdf_combine(n: usize) -> Vec<Step> {
    let ins: Vec<String> = (1..=n).map(|i| format!("in/input{i}.pdf")).collect();
    let mut argv: Vec<&str> = vec!["combine"];
    argv.extend(ins.iter().map(String::as_str));
    argv.extend(["--out", "out/output.pdf"]);
    vec![Step::Tool(tool(ToolId::Pdf, &argv, "out/output.pdf"))]
}

pub fn pdf_extract(pages: &PageSpec) -> Vec<Step> {
    vec![Step::Tool(tool(
        ToolId::Pdf,
        &[
            "extract",
            "in/input.pdf",
            "--pages",
            &pages.canonical(),
            "--out",
            "out/output.pdf",
        ],
        "out/output.pdf",
    ))]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SplitBy {
    Every(u32),
    Before(Vec<u32>),
}

pub fn pdf_split(by: &SplitBy) -> Vec<Step> {
    vec![Step::Expand(Expand::PdfSplit {
        pdf: "in/input.pdf".into(),
        by: by.clone(),
    })]
}
