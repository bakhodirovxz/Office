//! WP15 done-checks (PLAN section 8), against the real domain CLIs; skipped loudly when they are not built (they are looked up under
//! `$OFFICE_TARGET`, default `E:/office-target/p1`, as `<domain>/release/<tool>-cli(.exe)`):
//!
//! - `xdfu` off (the default) changes nothing: the outputs are byte for byte what a converter without the hook writes, there is no
//!   audit file and no `xdfu` in the report;
//! - a grif fixture per language (uz-Latn, uz-Cyrl, ru) is detected in a docx, xlsx, pptx and pdf, and a plain document is not;
//! - every output direction of an XDFU input either carries the watermark (text extracted from the output) or is refused;
//! - the audit chain verifies, tampering with one line breaks it, and the CLI's `xdfu-verify` says so.
use convert::audit::{self, AuditLog};
use convert::exec::{Config, Exit};
use convert::job::{self, Dest, FileReport};
use convert::plan::{self, Options, Target, ToolId};
use convert::xdfu::{Mode, XdfuHook};
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

struct Env {
    dir: PathBuf,
    tools: Vec<(ToolId, PathBuf)>,
}

impl Env {
    fn new(name: &str) -> Option<Env> {
        let target =
            std::env::var("OFFICE_TARGET").unwrap_or_else(|_| "E:/office-target/p1".into());
        let mut tools = vec![];
        for (id, dom) in [
            (ToolId::Docs, "docs"),
            (ToolId::Sheets, "sheets"),
            (ToolId::Slides, "slides"),
            (ToolId::Pdf, "pdf"),
        ] {
            let p = PathBuf::from(&target)
                .join(dom)
                .join("release")
                .join(format!("{}{}", id.name(), std::env::consts::EXE_SUFFIX));
            if !p.is_file() {
                eprintln!("SKIPPED xdfu e2e: {} not built", p.display());
                return None;
            }
            tools.push((id, p));
        }
        let dir = std::env::temp_dir().join(format!("convert-xdfu-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Some(Env { dir, tools })
    }

    fn tool(&self, id: ToolId) -> &Path {
        &self.tools.iter().find(|t| t.0 == id).unwrap().1
    }

    /// A converter config with the tools, and (unless `mode` is None) the XDFU hook as the CLI installs it.
    fn cfg(&self, mode: Option<Mode>, audit: &Path, redact: bool) -> Config {
        let mut c = Config::from_env();
        c.timeout_s = Some(180);
        c.work_dir = self.dir.join("work");
        for (id, p) in &self.tools {
            c.tools.insert(*id, p.clone());
        }
        if let Some(m) = mode {
            c.hooks.push(Arc::new(XdfuHook::new(
                m,
                Arc::new(AuditLog::open(audit, redact).unwrap()),
                true,
            )));
        }
        c
    }

    fn run_tool(&self, id: ToolId, args: &[&str]) {
        let out = Command::new(self.tool(id))
            .args(args)
            .current_dir(&self.dir)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{} {args:?}: {}",
            id.name(),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// A document of `kind` whose first line (or first cell, or first slide's title) is `grif`, or a plain one.
    fn fixture(&self, kind: &str, grif: Option<&str>, tag: &str) -> PathBuf {
        let head = grif.unwrap_or("Oddiy hujjat");
        let path = self.dir.join(format!("{tag}.{kind}"));
        let rel = |ext: &str| format!("{tag}.{ext}");
        match kind {
            "txt" | "md" => std::fs::write(
                &path,
                format!("{head}\n\nSalom dunyo, bu sinov matni.\nIkkinchi qator.\n"),
            )
            .unwrap(),
            "csv" => {
                std::fs::write(&path, format!("\"{head}\",x\nnom,qiymat\nA,1\nB,2\n")).unwrap()
            }
            "docx" => {
                std::fs::write(
                    self.dir.join(rel("src.txt")),
                    format!("{head}\n\nSalom dunyo, bu sinov matni.\nIkkinchi qator.\n"),
                )
                .unwrap();
                self.run_tool(ToolId::Docs, &["convert", &rel("src.txt"), &rel("docx")]);
            }
            "xlsx" => {
                std::fs::write(
                    self.dir.join(rel("src.csv")),
                    format!("\"{head}\",x\nnom,qiymat\nA,1\nB,2\n"),
                )
                .unwrap();
                self.run_tool(ToolId::Sheets, &["convert", &rel("src.csv"), &rel("xlsx")]);
            }
            "pptx" => {
                let title = format!(
                    "slide.new={{\"title\":{}}}",
                    serde_json::to_string(head).unwrap()
                );
                self.run_tool(
                    ToolId::Slides,
                    &[
                        "run",
                        "--cmd",
                        &title,
                        "--cmd",
                        "slide.delete={\"index\":0}",
                        "--save",
                        &rel("pptx"),
                    ],
                );
            }
            "pdf" => {
                self.fixture("docx", grif, &format!("{tag}-pdfsrc"));
                self.run_tool(
                    ToolId::Docs,
                    &["convert", &format!("{tag}-pdfsrc.docx"), &rel("pdf")],
                );
            }
            other => panic!("no fixture for {other}"),
        }
        path
    }

    fn convert(&self, cfg: &Config, input: &Path, to: &str, out: &str) -> FileReport {
        let dest = Dest {
            out_dir: self.dir.join(out),
            out_file: None,
        };
        job::convert_file(
            cfg,
            input,
            Target::parse(to).unwrap(),
            &Options::default(),
            &dest,
        )
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn xdfu_of(r: &FileReport) -> Option<serde_json::Value> {
    serde_json::to_value(r).unwrap().get("xdfu").cloned()
}

// ---- detection ---------------------------------------------------------------------------------------------------------------

/// A target the planner has for the kind, so that a plain document really converts.
fn target_of(kind: &str) -> &'static str {
    match kind {
        "xlsx" => "csv",
        "pptx" => "pdf",
        _ => "txt",
    }
}

/// One marker per language, in its short form and as the phrase.
const MARKERS: [(&str, &str); 6] = [
    ("uz-Latn code", "XDFU"),
    ("uz-Latn phrase", "Xizmat doirasida foydalanish uchun"),
    ("uz-Cyrl code", "ХДФУ"),
    ("uz-Cyrl phrase", "Хизмат доирасида фойдаланиш учун"),
    ("ru code", "ДСП"),
    ("ru phrase", "Для служебного пользования"),
];

#[test]
fn a_grif_fixture_per_language_is_detected_in_docx_xlsx_pptx_and_pdf_and_a_plain_document_is_not() {
    let Some(e) = Env::new("detect") else { return };
    let audit = e.dir.join("audit.jsonl");
    // refuse mode: detection and a veto, no conversion, so this is quick
    let cfg = e.cfg(Some(Mode::Refuse), &audit, false);
    let mut refusals = 0;
    for kind in ["docx", "xlsx", "pptx", "pdf"] {
        for (i, (what, marker)) in MARKERS.iter().enumerate() {
            let f = e.fixture(kind, Some(marker), &format!("g{i}-{kind}"));
            let r = e.convert(&cfg, &f, target_of(kind), "o");
            assert_eq!(r.code, 9, "{kind} {what}: {} {:?}", r.message, r.warnings);
            let x = xdfu_of(&r).unwrap_or_else(|| panic!("{kind} {what}: no xdfu in the report"));
            assert_eq!(x["detected"], true, "{kind} {what}");
            assert!(
                !x["evidence"].as_array().unwrap().is_empty(),
                "{kind} {what}: {x}"
            );
            assert_eq!(
                x["evidence"][0]["place"], "first-page-top",
                "{kind} {what}: {x}"
            );
            refusals += 1;
        }
        let plain = e.fixture(kind, None, &format!("plain-{kind}"));
        let r = e.convert(&cfg, &plain, target_of(kind), "o");
        assert_eq!(r.code, 0, "{kind} plain: {}", r.message);
        assert!(xdfu_of(&r).is_none(), "{kind} plain: marked as XDFU");
    }
    // text kinds, the first lines
    for kind in ["txt", "md", "csv"] {
        let to = if kind == "csv" { "csv" } else { "docx" };
        let f = e.fixture(
            kind,
            Some("Для служебного пользования"),
            &format!("t-{kind}"),
        );
        let r = e.convert(&cfg, &f, to, "o");
        assert_eq!(r.code, 9, "{kind}: {}", r.message);
        refusals += 1;
        let f = e.fixture(kind, None, &format!("tp-{kind}"));
        assert_eq!(e.convert(&cfg, &f, to, "o").code, 0, "{kind} plain");
    }
    // the other places: a header, the document properties (docx), a sheet header (xlsx), the pdf title
    e.fixture("docx", None, "hdr-src");
    e.run_tool(
        ToolId::Docs,
        &[
            "run",
            "--file",
            "hdr-src.docx",
            "--cmd",
            "insert.header={\"text\":\"Для служебного пользования\"}",
            "--save",
            "hdr.docx",
        ],
    );
    e.run_tool(
        ToolId::Docs,
        &[
            "run",
            "--file",
            "hdr-src.docx",
            "--cmd",
            "file.properties={\"subject\":\"ХДФУ\"}",
            "--save",
            "prop.docx",
        ],
    );
    e.fixture("xlsx", None, "xh-src");
    e.run_tool(
        ToolId::Sheets,
        &[
            "run",
            "--in",
            "xh-src.xlsx",
            "--cmd",
            "pageLayout.headerFooter={\"header\":\"&RДСП\"}",
            "--out",
            "xh.xlsx",
        ],
    );
    for (f, place) in [
        ("hdr.docx", "header"),
        ("prop.docx", "properties"),
        ("xh.xlsx", "header"),
    ] {
        let r = e.convert(
            &cfg,
            &e.dir.join(f),
            target_of(f.rsplit('.').next().unwrap()),
            "o",
        );
        assert_eq!(r.code, 9, "{f}: {}", r.message);
        let x = xdfu_of(&r).unwrap();
        assert!(
            x["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["place"] == place),
            "{f}: {x}"
        );
        refusals += 1;
    }
    std::fs::write(
        e.dir.join("t.json"),
        serde_json::json!([{"tool": "doc_open", "args": {"path": "plain-pdf.pdf"}}, {"tool": "doc_set_info", "args": {"doc": 1, "key": "Title", "value": "XDFU nusxa"}}, {"tool": "doc_save", "args": {"doc": 1, "path": "title.pdf", "full": true}}]).to_string(),
    )
    .unwrap();
    e.run_tool(ToolId::Pdf, &["run", "--script", "t.json", "--root", "."]);
    let r = e.convert(&cfg, &e.dir.join("title.pdf"), "txt", "o");
    assert_eq!(r.code, 9, "{}", r.message);
    assert!(
        xdfu_of(&r).unwrap()["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["place"] == "properties")
    );
    refusals += 1;
    // every refusal was logged; the plain documents logged nothing
    assert_eq!(audit::verify(&audit).unwrap().lines, refusals);
}

// ---- off changes nothing ---------------------------------------------------------------------------------------------------

#[test]
fn off_is_byte_identical_and_leaves_no_audit_and_no_report_field() {
    let Some(e) = Env::new("off") else { return };
    let audit = e.dir.join("never.jsonl");
    let bare = e.cfg(None, &audit, false);
    // the hook exists but is off: it must be a no-op as well
    let mut off = e.cfg(None, &audit, false);
    off.hooks.push(Arc::new(XdfuHook::new(
        Mode::Off,
        Arc::new(AuditLog::open(&e.dir.join("off.jsonl"), false).unwrap()),
        true,
    )));
    for (kind, to) in [
        ("docx", "txt"),
        ("docx", "md"),
        ("xlsx", "csv"),
        ("pdf", "txt"),
        ("pptx", "pdf"),
        ("txt", "md"),
        ("csv", "csv"),
        ("docx", "pdf"),
    ] {
        let f = e.fixture(kind, Some("ДСП"), &format!("off-{kind}"));
        let (a, b) = (
            e.convert(&bare, &f, to, &format!("a-{kind}-{to}")),
            e.convert(&off, &f, to, &format!("b-{kind}-{to}")),
        );
        assert_eq!(
            (a.code, b.code),
            (0, 0),
            "{kind}->{to}: {} / {}",
            a.message,
            b.message
        );
        assert_eq!(a.warnings.len(), b.warnings.len(), "{kind}->{to}");
        assert!(
            xdfu_of(&a).is_none() && xdfu_of(&b).is_none(),
            "{kind}->{to}"
        );
        assert!(
            !a.outputs.is_empty() && a.outputs.len() == b.outputs.len(),
            "{kind}->{to}"
        );
        for (x, y) in a.outputs.iter().zip(&b.outputs) {
            let (bx, by) = (std::fs::read(x).unwrap(), std::fs::read(y).unwrap());
            if matches!(to, "txt" | "md" | "csv") {
                assert_eq!(bx, by, "{kind}->{to}: the outputs differ");
            } else {
                // a pdf carries its creation time, so two runs of the plain converter differ in a few bytes too: same size, no mark
                assert_eq!(
                    bx.len(),
                    by.len(),
                    "{kind}->{to}: the outputs differ in size"
                );
                assert!(
                    !String::from_utf8_lossy(&by).contains("XDFU copy"),
                    "{kind}->{to}: marked"
                );
            }
        }
    }
    assert!(!audit.exists(), "no audit file with xdfu off");
    assert_eq!(
        audit::verify(&e.dir.join("off.jsonl"))
            .map(|v| v.lines)
            .unwrap_or(0),
        0,
        "the off hook wrote nothing"
    );
}

// ---- every direction carries the watermark or is refused ------------------------------------------------------------------

/// The text a watermark can be read from, per output kind.
fn extracted(e: &Env, path: &Path) -> String {
    let ext = path.extension().unwrap().to_string_lossy().to_lowercase();
    let part = |zip: &Path, pred: &dyn Fn(&str) -> bool| -> String {
        let mut z = zip::ZipArchive::new(std::fs::File::open(zip).unwrap()).unwrap();
        let names: Vec<String> = (0..z.len())
            .filter_map(|i| {
                z.by_index_raw(i)
                    .ok()
                    .and_then(|f| f.name().ok().map(|n| n.into_owned()))
            })
            .collect();
        let mut all = String::new();
        for n in names.iter().filter(|n| pred(n)) {
            let mut s = Vec::new();
            z.by_name(n).unwrap().read_to_end(&mut s).unwrap();
            all.push_str(&String::from_utf8_lossy(&s));
        }
        all
    };
    match ext.as_str() {
        "txt" | "md" | "csv" | "html" => std::fs::read_to_string(path).unwrap(),
        "docx" => part(path, &|n| {
            n.starts_with("word/header") || n.starts_with("word/footer")
        }),
        "xlsx" => part(path, &|n| n.starts_with("xl/worksheets/")),
        "pptx" => part(path, &|n| n.starts_with("ppt/slides/slide")),
        "pdf" => {
            let o = Command::new(e.tool(ToolId::Pdf))
                .arg("text")
                .arg(path)
                .output()
                .unwrap();
            String::from_utf8_lossy(&o.stdout).into_owned()
        }
        "png" => {
            let dec =
                png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
            let r = dec.read_info().unwrap();
            r.info()
                .uncompressed_latin1_text
                .iter()
                .map(|t| t.text.clone())
                .collect::<Vec<_>>()
                .join("\n")
        }
        other => panic!("no text extraction for .{other}"),
    }
}

#[test]
fn every_direction_of_an_xdfu_input_carries_the_watermark_or_is_refused() {
    let Some(e) = Env::new("matrix") else { return };
    let audit = e.dir.join("matrix.jsonl");
    let cfg = e.cfg(Some(Mode::Watermark), &audit, false);
    let (mut carried, mut refused, mut other) = (BTreeSet::new(), BTreeSet::new(), vec![]);
    let kinds = ["docx", "xlsx", "pptx", "pdf", "txt", "md", "csv"];
    let sources: Vec<(&str, PathBuf)> = kinds
        .iter()
        .map(|k| (*k, e.fixture(k, Some("ДСП"), &format!("m-{k}"))))
        .collect();
    for (from, to) in plan::rows() {
        let Some((_, f)) = sources.iter().find(|s| s.0 == from.ext()) else {
            continue;
        };
        let out = format!("o-{}-{}", from.ext(), to.ext());
        let r = e.convert(&cfg, f, to.ext(), &out);
        let label = format!("{}->{}", from.ext(), to.ext());
        let x = xdfu_of(&r);
        if matches!(to.ext(), "odt" | "rtf") {
            assert_eq!(r.code, 3, "{label}: {} (must be refused)", r.message);
            assert!(
                r.outputs.is_empty() && r.message.contains("watermark"),
                "{label}: {}",
                r.message
            );
            refused.insert(label);
            continue;
        }
        if r.code != 0 && r.code != 10 {
            other.push(format!("{label}: exit {} {}", r.code, r.message));
            continue;
        }
        let x = x.unwrap_or_else(|| panic!("{label}: no xdfu in the report"));
        let copy = x["copy"].as_u64().unwrap_or_else(|| panic!("{label}: {x}"));
        assert!(r.warnings.iter().any(|w| w.code == "xdfu_copy"), "{label}");
        assert!(!r.outputs.is_empty(), "{label}");
        for o in &r.outputs {
            let text = extracted(&e, Path::new(o));
            assert!(
                text.contains(&format!("XDFU copy {copy} |")),
                "{label}: {o} carries no watermark: {:.300}",
                text
            );
            assert!(
                text.contains("| 20") && text.contains(" UTC"),
                "{label}: no date and time in {o}"
            );
        }
        carried.insert(label);
    }
    println!(
        "carried {} directions: {carried:?}\nrefused {}: {refused:?}",
        carried.len(),
        refused.len()
    );
    assert!(
        other.is_empty(),
        "directions that failed for another reason:\n{}",
        other.join("\n")
    );
    assert!(
        carried.len() >= 25 && !refused.is_empty(),
        "{carried:?} {refused:?}"
    );
    // png: the strip is under the page (taller than the plain render) and the text chunk is there
    let plain = e.convert(
        &e.cfg(None, &audit, false),
        &sources[3].1,
        "png",
        "png-plain",
    );
    assert_eq!(plain.code, 0, "{}", plain.message);
    let height = |p: &str| {
        png::Decoder::new(std::io::BufReader::new(std::fs::File::open(p).unwrap()))
            .read_info()
            .unwrap()
            .info()
            .height
    };
    let marked = e.convert(&cfg, &sources[3].1, "png", "png-marked");
    assert_eq!(marked.code, 0, "{}", marked.message);
    assert!(
        height(&marked.outputs[0]) > height(&plain.outputs[0]),
        "the strip is appended under the page"
    );
    assert_eq!(
        audit::verify(&audit).unwrap().lines as usize,
        carried.len() + refused.len() + 1,
        "one audit line per XDFU job"
    );
}

// ---- refuse, redact, the audit chain, the CLI ------------------------------------------------------------------------------

#[test]
fn refuse_vetoes_before_any_conversion_and_redact_keeps_file_names_out_of_the_log() {
    let Some(e) = Env::new("refuse") else { return };
    let audit = e.dir.join("refuse.jsonl");
    let cfg = e.cfg(Some(Mode::Refuse), &audit, true);
    let f = e.fixture("docx", Some("XDFU"), "secret-name");
    let r = e.convert(&cfg, &f, "pdf", "o");
    assert_eq!(r.code, 9, "{}", r.message);
    assert!(
        r.message.contains("refuse") && r.steps.is_empty() && r.outputs.is_empty(),
        "{} {:?}",
        r.message,
        r.steps.len()
    );
    assert!(!e.dir.join("o").exists() || std::fs::read_dir(e.dir.join("o")).unwrap().count() == 0);
    let text = std::fs::read_to_string(&audit).unwrap();
    assert!(
        text.contains("\"event\":\"refused\"") && text.contains("\"reason\":\"mode\""),
        "{text}"
    );
    assert!(
        !text.contains("secret-name") && !text.contains("\"file\""),
        "a file name in the redacted log: {text}"
    );
    // without redact the name is there
    let audit2 = e.dir.join("named.jsonl");
    let r = e.convert(&e.cfg(Some(Mode::Refuse), &audit2, false), &f, "pdf", "o");
    assert_eq!(r.code, 9);
    assert!(
        std::fs::read_to_string(&audit2)
            .unwrap()
            .contains("secret-name")
    );
    // a plain document under refuse is just converted, and logs nothing
    let p = e.fixture("docx", None, "ordinary");
    assert_eq!(
        e.convert(&e.cfg(Some(Mode::Refuse), &audit, true), &p, "pdf", "o")
            .code,
        0
    );
    assert_eq!(audit::verify(&audit).unwrap().lines, 1);
}

#[test]
fn the_audit_chain_verifies_tampering_breaks_it_and_the_cli_says_so() {
    let Some(e) = Env::new("chain") else { return };
    let audit = e.dir.join("chain.jsonl");
    let cfg = e.cfg(Some(Mode::Watermark), &audit, false);
    for i in 0..3 {
        let f = e.fixture("txt", Some("ДСП"), &format!("c{i}"));
        let r = e.convert(&cfg, &f, "md", &format!("o{i}"));
        assert_eq!(r.code, 0, "{}", r.message);
        assert_eq!(
            xdfu_of(&r).unwrap()["copy"],
            i + 1,
            "copy numbers count up in one chain"
        );
    }
    let bin = env!("CARGO_BIN_EXE_convert");
    let verify = |p: &Path| {
        Command::new(bin)
            .arg("xdfu-verify")
            .arg(p)
            .output()
            .unwrap()
    };
    let out = verify(&audit);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("3 lines"),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    // who, when, from where, which job and file; the copy number is the sequence number; no content
    let text = std::fs::read_to_string(&audit).unwrap();
    let first: serde_json::Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    for k in ["user", "client", "ts", "job", "file", "seq", "prev", "hash"] {
        assert!(first.get(k).is_some(), "{k} missing in {first}");
    }
    assert!(!text.contains("Salom dunyo"), "document content in the log");
    // change one line
    let lines: Vec<&str> = text.lines().collect();
    let bad = [
        lines[0].to_string(),
        lines[1].replacen("\"md\"", "\"pdf\"", 1),
        lines[2].to_string(),
    ]
    .join("\n")
        + "\n";
    assert_ne!(bad, text);
    std::fs::write(&audit, &bad).unwrap();
    let out = verify(&audit);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("line 2"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // and the converter refuses to extend a damaged log
    assert!(AuditLog::open(&audit, false).is_err());
}

// ---- input kinds the converter cannot scan are checked on their output ------------------------------------------------------

/// `tests/golden/xdfu/{grif,plain}.{doc,xls}`: made with Word and Excel from synthetic text (a first paragraph or cell "ХДФУ" for
/// the grif, "Oddiy hujjat" for the plain one), small enough to commit.
fn golden(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/xdfu")
        .join(name)
}

fn audit_lines(p: &Path) -> usize {
    std::fs::read_to_string(p)
        .map(|t| t.lines().count())
        .unwrap_or(0)
}

#[test]
fn a_doc_and_an_xls_with_a_grif_are_found_on_the_output_and_watermarked() {
    let Some(e) = Env::new("deferred") else {
        return;
    };
    let audit = e.dir.join("audit.jsonl");
    let cfg = e.cfg(Some(Mode::Watermark), &audit, false);
    for (src, to) in [
        ("grif.doc", "docx"),
        ("grif.doc", "txt"),
        ("grif.doc", "pdf"),
        ("grif.doc", "html"),
        ("grif.xls", "xlsx"),
        ("grif.xls", "csv"),
        ("grif.xls", "pdf"),
    ] {
        let r = e.convert(&cfg, &golden(src), to, &format!("g-{src}-{to}"));
        assert_eq!(r.code, 0, "{src} to {to}: {}", r.message);
        let x = xdfu_of(&r)
            .unwrap_or_else(|| panic!("{src} to {to}: the grif was not found on the output"));
        assert_eq!(
            (x["detected"].clone(), x["refused"].clone()),
            (serde_json::json!(true), serde_json::Value::Null),
            "{src} to {to}"
        );
        assert!(
            x["copy"].as_u64().is_some() && !x["evidence"].as_array().unwrap().is_empty(),
            "{src} to {to}: {x}"
        );
        assert!(
            r.warnings.iter().any(|w| w.code == "xdfu_copy"),
            "{src} to {to}"
        );
        let out = PathBuf::from(&r.outputs[0]);
        assert!(
            extracted(&e, &out).contains("XDFU copy"),
            "{src} to {to}: the output carries no watermark"
        );
    }
    // a plain document of the same kinds: converted as it always was, no xdfu in the report, nothing in the audit log
    let before = audit_lines(&audit);
    for (src, to) in [
        ("plain.doc", "docx"),
        ("plain.doc", "txt"),
        ("plain.xls", "xlsx"),
        ("plain.xls", "csv"),
    ] {
        let r = e.convert(&cfg, &golden(src), to, &format!("p-{src}-{to}"));
        assert_eq!(r.code, 0, "{src} to {to}: {}", r.message);
        assert!(xdfu_of(&r).is_none(), "{src} to {to}: {:?}", xdfu_of(&r));
        assert!(
            !extracted(&e, &PathBuf::from(&r.outputs[0])).contains("XDFU copy"),
            "{src} to {to}: marked"
        );
    }
    assert_eq!(
        audit_lines(&audit),
        before,
        "a plain document left a line in the audit log"
    );
    assert!(audit::verify(&audit).is_ok());
}

#[test]
fn refuse_mode_vetoes_a_doc_with_a_grif_after_the_conversion_and_lets_a_plain_one_through() {
    let Some(e) = Env::new("deferred-refuse") else {
        return;
    };
    let audit = e.dir.join("audit.jsonl");
    let cfg = e.cfg(Some(Mode::Refuse), &audit, false);
    for (src, to) in [("grif.doc", "docx"), ("grif.xls", "xlsx")] {
        let r = e.convert(&cfg, &golden(src), to, &format!("r-{src}"));
        assert_eq!(r.code, Exit::Rejected.code(), "{src}: {}", r.message);
        assert!(r.outputs.is_empty(), "{src}: a refused job returned a file");
        assert_eq!(xdfu_of(&r).unwrap()["refused"], "mode", "{src}");
    }
    assert!(
        std::fs::read_to_string(&audit)
            .unwrap()
            .lines()
            .filter(|l| l.contains("\"refused\""))
            .count()
            == 2
    );
    for (src, to) in [("plain.doc", "docx"), ("plain.xls", "xlsx")] {
        let r = e.convert(&cfg, &golden(src), to, &format!("rp-{src}"));
        assert_eq!(r.code, 0, "{src}: {}", r.message);
    }
}

/// A legacy input is read on the input now (a probe conversion), so a grif.doc is found whatever the target is; what is refused is a
/// target that cannot carry the watermark. A plain document of the same kind converts to any target.
#[test]
fn a_legacy_input_is_checked_whatever_the_target_and_a_target_that_cannot_carry_the_mark_is_refused()
 {
    let Some(e) = Env::new("cannot-carry") else {
        return;
    };
    let audit = e.dir.join("audit.jsonl");
    let cfg = e.cfg(Some(Mode::Watermark), &audit, false);
    let r = e.convert(&cfg, &golden("grif.doc"), "odt", "c-odt");
    assert_eq!(r.code, Exit::Unsupported.code(), "{}", r.message);
    assert!(r.message.contains("cannot carry"), "{}", r.message);
    assert!(r.outputs.is_empty() && r.steps.is_empty(), "a tool ran");
    let cfg_r = e.cfg(Some(Mode::Refuse), &audit, false);
    let r = e.convert(&cfg_r, &golden("grif.doc"), "odt", "c-odt-r");
    assert_eq!(r.code, Exit::Rejected.code(), "{}", r.message);
    for to in ["rtf", "png"] {
        let r = e.convert(&cfg, &golden("plain.doc"), to, &format!("c-plain-{to}"));
        assert_eq!(r.code, 0, "plain.doc to {to}: {}", r.message);
        assert!(xdfu_of(&r).is_none());
    }
    assert!(audit::verify(&audit).is_ok());
}

// ---- gate review M3: the mark is added, nothing the document has is replaced ----------------------------------------------------

fn read_part(zip: &Path, pred: &dyn Fn(&str) -> bool) -> String {
    let mut z = zip::ZipArchive::new(std::fs::File::open(zip).unwrap()).unwrap();
    let names: Vec<String> = (0..z.len())
        .filter_map(|i| {
            z.by_index_raw(i)
                .ok()
                .and_then(|f| f.name().ok().map(|n| n.into_owned()))
        })
        .collect();
    let mut all = String::new();
    for n in names.iter().filter(|n| pred(n)) {
        let mut b = Vec::new();
        z.by_name(n).unwrap().read_to_end(&mut b).unwrap();
        all.push_str(&String::from_utf8_lossy(&b));
    }
    all
}

#[test]
fn the_watermark_keeps_the_documents_own_header_footer_and_footer_text() {
    let Some(e) = Env::new("keep") else { return };
    let audit = e.dir.join("audit.jsonl");
    let cfg = e.cfg(Some(Mode::Watermark), &audit, false);
    // docx: a letterhead in the header and a footer, the grif in the body
    e.fixture("docx", Some("ХДФУ"), "kd-src");
    e.run_tool(
        ToolId::Docs,
        &[
            "run",
            "--file",
            "kd-src.docx",
            "--cmd",
            "insert.header={\"text\":\"LETTERHEAD Acme Ltd\"}",
            "--cmd",
            "insert.footer={\"text\":\"FOOTER page text\"}",
            "--save",
            "kd.docx",
        ],
    );
    let r = e.convert(&cfg, &e.dir.join("kd.docx"), "docx", "kd");
    assert_eq!(r.code, 0, "{}", r.message);
    let out = PathBuf::from(&r.outputs[0]);
    let hf = read_part(&out, &|n| {
        n.starts_with("word/header") || n.starts_with("word/footer")
    });
    assert!(
        hf.contains("LETTERHEAD Acme Ltd") && hf.contains("FOOTER page text"),
        "the header or footer was replaced: {hf}"
    );
    assert!(hf.contains("XDFU copy"), "{hf}");
    let body = read_part(&out, &|n| n == "word/document.xml");
    assert!(
        body.contains("XDFU copy") && body.contains("ХДФУ"),
        "{body}"
    );
    // a header holding the grif (and a different first page) still carries the grif and the stamp afterwards
    e.run_tool(
        ToolId::Docs,
        &[
            "run",
            "--file",
            "kd-src.docx",
            "--cmd",
            "insert.header={\"text\":\"Для служебного пользования\"}",
            "--save",
            "kg.docx",
        ],
    );
    let r = e.convert(&cfg, &e.dir.join("kg.docx"), "docx", "kg");
    assert_eq!(r.code, 0, "{}", r.message);
    let hf = read_part(&PathBuf::from(&r.outputs[0]), &|n| {
        n.starts_with("word/header")
    });
    assert!(
        hf.contains("Для служебного пользования") && hf.contains("XDFU copy"),
        "{hf}"
    );

    // xlsx: a letterhead on the left of the header and a page number in the footer
    e.fixture("xlsx", Some("ДСП"), "kx-src");
    e.run_tool(
        ToolId::Sheets,
        &[
            "run",
            "--in",
            "kx-src.xlsx",
            "--cmd",
            "pageLayout.headerFooter={\"header\":\"&LLETTERHEAD Acme\",\"footer\":\"Page &P\"}",
            "--out",
            "kx.xlsx",
        ],
    );
    let r = e.convert(&cfg, &e.dir.join("kx.xlsx"), "xlsx", "kx");
    assert_eq!(r.code, 0, "{}", r.message);
    let sheet = read_part(&PathBuf::from(&r.outputs[0]), &|n| {
        n.starts_with("xl/worksheets/")
    });
    assert!(
        sheet.contains("LETTERHEAD Acme")
            && sheet.contains("Page &amp;P")
            && sheet.contains("XDFU copy"),
        "{sheet}"
    );

    // pptx: the slide's own footer
    e.fixture("pptx", Some("XDFU"), "kp-src");
    e.run_tool(
        ToolId::Slides,
        &[
            "run",
            "--in",
            "kp-src.pptx",
            "--cmd",
            "design.headerFooter={\"footer\":true,\"footerText\":\"OUR FOOTER\",\"all\":true}",
            "--save",
            "kp.pptx",
        ],
    );
    let r = e.convert(&cfg, &e.dir.join("kp.pptx"), "pptx", "kp");
    assert_eq!(r.code, 0, "{}", r.message);
    let slides = read_part(&PathBuf::from(&r.outputs[0]), &|n| {
        n.starts_with("ppt/slides/slide")
    });
    assert!(
        slides.contains("OUR FOOTER") && slides.contains("XDFU copy"),
        "{slides}"
    );
    // and every mark is audited
    assert_eq!(audit::verify(&audit).unwrap().lines, 4);
}

// ---- gate review M4 ---------------------------------------------------------------------------------------------------------------

#[test]
fn an_image_only_pdf_cannot_be_checked_and_is_refused() {
    let Some(e) = Env::new("scan") else { return };
    let audit = e.dir.join("audit.jsonl");
    // a page with no text at all, as a scan has none
    std::fs::write(
        e.dir.join("blank.json"),
        serde_json::json!([
            {"tool": "doc_create", "args": {"from": "blank", "pages": 1, "width": 300, "height": 200}},
            {"tool": "doc_save", "args": {"doc": 1, "path": "blank.pdf", "full": true}}
        ])
        .to_string(),
    )
    .unwrap();
    e.run_tool(
        ToolId::Pdf,
        &["run", "--script", "blank.json", "--root", "."],
    );
    for mode in [Mode::Watermark, Mode::Refuse] {
        let cfg = e.cfg(Some(mode), &audit, false);
        let r = e.convert(&cfg, &e.dir.join("blank.pdf"), "txt", "o");
        assert_eq!(r.code, Exit::Unsupported.code(), "{mode:?}: {}", r.message);
        assert!(r.message.contains("no text"), "{}", r.message);
        assert!(r.outputs.is_empty() && r.steps.is_empty(), "a tool ran");
    }
    let log = std::fs::read_to_string(&audit).unwrap();
    assert!(log.lines().any(|l| l.contains("no_text_layer")), "{log}");
    // with xdfu off the same file converts as before
    let off = e.cfg(None, &audit, false);
    assert_ne!(
        e.convert(&off, &e.dir.join("blank.pdf"), "txt", "off").code,
        Exit::Unsupported.code()
    );
}

#[test]
fn a_grif_in_the_header_of_an_odt_and_of_an_rtf_is_found_on_the_input() {
    let Some(e) = Env::new("legacy-hdr") else {
        return;
    };
    let audit = e.dir.join("audit.jsonl");
    let cfg = e.cfg(Some(Mode::Refuse), &audit, false);
    // our writers drop headers for odt and rtf, so both are made by hand
    std::fs::write(
        e.dir.join("oh.rtf"),
        r"{\rtf1\ansi\deff0{\fonttbl{\f0 Arial;}}{\header\pard\plain XDFU\par}\pard\plain Body text\par}",
    )
    .unwrap();
    {
        use std::io::Write;
        let mut w = zip::ZipWriter::new(std::fs::File::create(e.dir.join("oh.odt")).unwrap());
        let stored = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (n, body) in [
            ("mimetype", "application/vnd.oasis.opendocument.text"),
            (
                "content.xml",
                "<office:document-content><office:body><office:text><text:p>Body text</text:p></office:text></office:body></office:document-content>",
            ),
            (
                "styles.xml",
                "<office:document-styles><office:master-styles><style:master-page><style:header><text:p>Для служебного пользования</text:p></style:header></style:master-page></office:master-styles></office:document-styles>",
            ),
        ] {
            w.start_file(n, stored).unwrap();
            w.write_all(body.as_bytes()).unwrap();
        }
        w.finish().unwrap();
    }
    for f in ["oh.odt", "oh.rtf"] {
        let r = e.convert(&cfg, &e.dir.join(f), "txt", &format!("o-{f}"));
        assert_eq!(r.code, Exit::Rejected.code(), "{f}: {}", r.message);
        let x = xdfu_of(&r).unwrap_or_else(|| panic!("{f}: no xdfu in the report"));
        assert!(
            x["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["place"] == "header"),
            "{f}: {x}"
        );
    }
}

// ---- gate review M5: the keyed audit chain ---------------------------------------------------------------------------------------

#[test]
fn a_keyed_audit_log_verifies_with_its_key_and_a_consistent_rewrite_without_it_does_not() {
    let Some(e) = Env::new("keyed") else { return };
    let (audit, key) = (e.dir.join("keyed.jsonl"), e.dir.join("audit.key"));
    std::fs::write(&key, "0123456789abcdef0123456789abcdef").unwrap();
    let mut cfg = e.cfg(None, &audit, false);
    cfg.hooks.push(Arc::new(XdfuHook::new(
        Mode::Watermark,
        Arc::new(AuditLog::open_keyed(&audit, false, Some(&key)).unwrap()),
        true,
    )));
    for i in 0..3 {
        let f = e.fixture("txt", Some("ДСП"), &format!("k{i}"));
        assert_eq!(e.convert(&cfg, &f, "md", &format!("o{i}")).code, 0);
    }
    let bin = env!("CARGO_BIN_EXE_convert");
    let verify = |p: &Path, key: Option<&Path>| {
        let mut c = Command::new(bin);
        c.arg("xdfu-verify").arg(p);
        if let Some(k) = key {
            c.arg("--key").arg(k);
        }
        c.output().unwrap()
    };
    assert!(verify(&audit, Some(&key)).status.success());
    assert!(
        !verify(&audit, None).status.success(),
        "a keyed log must not verify without its key"
    );
    // someone with write access changes line 2 and recomputes the plain SHA-256 chain from there: no key, so the HMAC cannot follow
    let text = std::fs::read_to_string(&audit).unwrap();
    let mut prev = String::new();
    let mut out = vec![];
    for (i, l) in text.lines().enumerate() {
        let mut o: serde_json::Map<String, serde_json::Value> = serde_json::from_str(l).unwrap();
        o.remove("hash");
        if i == 1 {
            o.insert("user".into(), "mallory".into());
        }
        if i >= 1 {
            o.insert("prev".into(), prev.clone().into());
        }
        let body = serde_json::to_string(&o).unwrap();
        use sha2::{Digest, Sha256};
        let h: String = Sha256::digest(body.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        o.insert("hash".into(), h.clone().into());
        prev = h;
        out.push(serde_json::to_string(&o).unwrap());
    }
    std::fs::write(&audit, out.join("\n") + "\n").unwrap();
    let v = verify(&audit, Some(&key));
    assert!(!v.status.success());
    assert!(
        String::from_utf8_lossy(&v.stderr).contains("line"),
        "{}",
        String::from_utf8_lossy(&v.stderr)
    );
}

#[test]
fn an_html_input_is_scanned_and_a_grif_in_it_is_found() {
    let Some(e) = Env::new("html") else { return };
    let audit = e.dir.join("audit.jsonl");
    let cfg = e.cfg(Some(Mode::Watermark), &audit, false);
    let page = |head: &str| {
        format!(
            "<!DOCTYPE html><html><head><title>t</title><style>p {{ color: red }}</style></head><body><p>{head}</p><p>Salom dunyo, sinov matni.</p></body></html>"
        )
    };
    let grif = e.dir.join("grif.html");
    std::fs::write(&grif, page("ХДФУ")).unwrap();
    let r = e.convert(&cfg, &grif, "txt", "html-grif");
    assert_eq!(r.code, 0, "{}", r.message);
    assert_eq!(xdfu_of(&r).unwrap()["detected"], true);
    assert!(extracted(&e, &PathBuf::from(&r.outputs[0])).contains("XDFU copy"));
    let plain = e.dir.join("plain.html");
    std::fs::write(&plain, page("Oddiy hujjat")).unwrap();
    let r = e.convert(&cfg, &plain, "txt", "html-plain");
    assert!(r.code == 0 && xdfu_of(&r).is_none(), "{}", r.message);
    // the grif in a <style> or the title is not text on the page
    let hidden = e.dir.join("hidden.html");
    std::fs::write(
        &hidden,
        "<html><head><title>ХДФУ</title></head><body><p>Oddiy hujjat</p></body></html>",
    )
    .unwrap();
    assert!(xdfu_of(&e.convert(&cfg, &hidden, "txt", "html-hidden")).is_none());
}

/// A combine looks at every input: a grif in the second file is found, and the job is refused (or marked) as a whole.
#[test]
fn a_grif_in_the_second_file_of_a_combine_is_found() {
    let Some(e) = Env::new("combine") else { return };
    let audit = e.dir.join("audit.jsonl");
    let plain = e.fixture("pdf", None, "cb-plain");
    let grif = e.fixture("pdf", Some("ДСП"), "cb-grif");
    let dest = Dest {
        out_dir: e.dir.join("o"),
        out_file: Some(e.dir.join("o/combined.pdf")),
    };
    let inputs = vec![plain, grif];
    let r = job::pdf_op(
        &e.cfg(Some(Mode::Refuse), &audit, false),
        "combine",
        &inputs,
        plan::pdf_combine(2),
        &dest,
    );
    assert_eq!(r.code, Exit::Rejected.code(), "{}", r.message);
    assert!(r.outputs.is_empty());
    let r = job::pdf_op(
        &e.cfg(Some(Mode::Watermark), &audit, false),
        "combine",
        &inputs,
        plan::pdf_combine(2),
        &dest,
    );
    assert_eq!(r.code, 0, "{}", r.message);
    assert!(extracted(&e, &PathBuf::from(&r.outputs[0])).contains("XDFU copy"));
    // two plain files: an ordinary combine
    let p2 = e.fixture("pdf", None, "cb-plain2");
    let r = job::pdf_op(
        &e.cfg(Some(Mode::Refuse), &audit, false),
        "combine",
        &[inputs[0].clone(), p2],
        plan::pdf_combine(2),
        &dest,
    );
    assert_eq!(r.code, 0, "{}", r.message);
}
