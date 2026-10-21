//! WP3: one fixture per warning code, "a plain letter gives no warning", --verify, zip-bomb rejection, the report schema.
//! Hermetic: the domain CLIs are replaced by `convert-fake-tool`, which writes a valid stub output and can print what the
//! real CLIs print (stderr `Warning:` lines, the translit result JSON).
use convert::exec::{Config, Exit};
use convert::job::{self, Dest, FileReport, JobSpec};
use convert::plan::{Options, Outputs, Step, Target, ToolId, ToolStep};
use convert::report;
use std::io::Write;
use std::path::{Path, PathBuf};

struct Sandbox(PathBuf);
impl Sandbox {
    fn new(name: &str) -> Sandbox {
        let p = std::env::temp_dir().join(format!("convert-warn-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Sandbox(p)
    }
    fn cfg(&self) -> Config {
        let mut c = Config::from_env();
        c.work_dir = self.0.join("work");
        for t in ToolId::ALL {
            c.tools.insert(t, env!("CARGO_BIN_EXE_convert-fake-tool").into());
        }
        c.sample_ms = 50;
        c
    }
    fn dest(&self) -> Dest {
        Dest { out_dir: self.0.join("dest"), out_file: None }
    }
    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }
    fn zip(&self, name: &str, parts: &[&str]) -> PathBuf {
        let p = self.0.join(name);
        let mut w = zip::ZipWriter::new(std::fs::File::create(&p).unwrap());
        for part in parts {
            w.start_file(*part, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(b"<x/>").unwrap();
        }
        w.finish().unwrap();
        p
    }
    /// A zip whose parts have the given bodies (the scanner looks inside document.xml, content.xml and the header parts).
    fn zip_bodies(&self, name: &str, parts: &[(&str, &str)]) -> PathBuf {
        let p = self.0.join(name);
        let mut w = zip::ZipWriter::new(std::fs::File::create(&p).unwrap());
        // stored, so that an ODF package's `mimetype` is readable in the first bytes, as kind detection expects
        let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (part, body) in parts {
            w.start_file(*part, stored).unwrap();
            w.write_all(body.as_bytes()).unwrap();
        }
        w.finish().unwrap();
        p
    }
    fn docx(&self, name: &str, extra: &[&str]) -> PathBuf {
        let mut parts = vec!["[Content_Types].xml", "word/document.xml"];
        parts.extend_from_slice(extra);
        self.zip(name, &parts)
    }
    fn convert(&self, input: &Path, to: Target) -> FileReport {
        self.convert_with(&self.cfg(), input, to)
    }
    fn convert_with(&self, cfg: &Config, input: &Path, to: Target) -> FileReport {
        job::convert_file(cfg, input, to, &Options::default(), &self.dest())
    }
    /// Run explicit fake-tool steps (the planner is not involved).
    fn steps(&self, cfg: &Config, steps: Vec<(&[&str], &str, Option<&str>)>) -> FileReport {
        let input = self.file("sample.txt", b"hello");
        let steps = steps
            .into_iter()
            .map(|(argv, out, stdout_to)| {
                Step::Tool(ToolStep {
                    tool: ToolId::Docs,
                    argv: argv.iter().map(|s| s.to_string()).collect(),
                    outputs: Outputs::Files(vec![out.into()]),
                    stdout_to: stdout_to.map(String::from),
                    allow_empty: stdout_to.is_some(),
                    write_files: vec![],
                })
            })
            .collect();
        let dest = self.dest();
        job::run_job(
            cfg,
            JobSpec {
                label: "sample.txt".into(),
                kind: Some("txt".into()),
                target: "x".into(),
                inputs: vec![(input.clone(), "in/input.txt".into())],
                origins: vec![input],
                steps: Ok(steps),
                stem: "sample".into(),
                ..JobSpec::new(&dest)
            },
        )
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn codes(r: &FileReport) -> Vec<&str> {
    r.warnings.iter().map(|w| w.code.as_str()).collect()
}

const OLE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
fn ole(stream: &str) -> Vec<u8> {
    [&OLE[..], &stream.encode_utf16().flat_map(|c| c.to_le_bytes()).collect::<Vec<u8>>()].concat()
}

#[test]
fn a_plain_letter_to_pdf_gives_no_warning() {
    let s = Sandbox::new("plain");
    let r = s.convert(&s.docx("letter.docx", &[]), Target::Pdf);
    assert_eq!((r.code, r.status), (0, "ok"), "{}", r.message);
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    // images and headers matter to md/txt only, not to pdf
    let r = s.convert(&s.docx("rich.docx", &["word/media/image1.png", "word/header1.xml"]), Target::Pdf);
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
}

#[test]
fn doc_import_loss() {
    let s = Sandbox::new("doc");
    let r = s.convert(&s.file("order.doc", &ole("WordDocument")), Target::Pdf);
    assert_eq!(codes(&r), ["doc_import_loss"], "{}", r.message);
    assert!(r.warnings[0].text.contains("comments"));
}

#[test]
fn xls_pdf_chart_page_and_en_us() {
    let s = Sandbox::new("xls");
    let r = s.convert(&s.file("staff.xls", &ole("Workbook")), Target::Pdf);
    assert_eq!(codes(&r), ["xls_pdf_chart_page", "pdf_en_us"], "{}", r.message);
}

#[test]
fn csv_is_en_us_one_sheet() {
    let s = Sandbox::new("csv");
    let r = s.convert(&s.zip("staff.xlsx", &["[Content_Types].xml", "xl/workbook.xml"]), Target::Csv);
    assert_eq!(codes(&r), ["csv_one_sheet_en_us"], "{}", r.message);
}

const FOOTER: &str = "<w:ftr><w:p><w:r><w:t>Page 1</w:t></w:r></w:p></w:ftr>";
const EMPTY_HEADER: &str = "<w:hdr><w:p/></w:hdr>";

#[test]
fn plain_text_layout_needs_a_header_or_an_image() {
    let s = Sandbox::new("md");
    assert!(s.convert(&s.docx("plain.docx", &[]), Target::Md).warnings.is_empty());
    // Word's empty first-page header and footer parts are not a header: a plain letter stays quiet
    let letter = s.zip_bodies("letter.docx", &[("word/document.xml", "<w:document><w:p/></w:document>"), ("word/header1.xml", EMPTY_HEADER), ("word/footer1.xml", EMPTY_HEADER)]);
    for to in [Target::Md, Target::Txt, Target::Odt, Target::Rtf, Target::Html] {
        assert!(s.convert(&letter, to).warnings.is_empty(), "a plain letter to {} warned", to.ext());
    }
    let rich = s.zip_bodies("rich.docx", &[("word/document.xml", "<w:document/>"), ("word/media/image1.png", "x"), ("word/footer1.xml", FOOTER)]);
    let r = s.convert(&rich, Target::Md);
    assert_eq!(codes(&r), ["plain_text_layout"], "{}", r.message);
    assert_eq!(r.warnings[0].count, 2);
}

/// WP9 rows end to end: the planner is real, the tool is the fake one, the scanner reads a package built here.
#[test]
fn the_wp9_loss_rows_warn_through_convert_only_when_the_feature_is_there() {
    let s = Sandbox::new("wp9");
    let doc = |body: &str| format!("<w:document>{body}</w:document>");
    let rich_body = doc("<w:tbl><w:tr><w:tc><w:gridSpan w:val=\"2\"/></w:tc></w:tr></w:tbl><w:ins w:id=\"1\"/><w:footnoteReference w:id=\"2\"/>");
    let rich = s.zip_bodies(
        "rich.docx",
        &[
            ("word/document.xml", &rich_body),
            ("word/comments.xml", "<w:comments/>"),
            ("word/header2.xml", "<w:hdr><w:p><w:r><w:t>HRM</w:t></w:r></w:p></w:hdr>"),
        ],
    );
    for (to, want) in [
        (Target::Odt, vec!["comments_dropped", "footnotes_dropped", "headers_footers_dropped"]),
        (Target::Rtf, vec!["comments_dropped", "footnotes_dropped", "headers_footers_dropped"]),
        (Target::Html, vec!["comments_dropped", "footnotes_dropped", "headers_footers_dropped", "revisions_dropped"]),
        (Target::Md, vec!["plain_text_layout", "comments_dropped", "footnotes_dropped", "revisions_dropped", "merged_cells_flattened"]),
        (Target::Txt, vec!["plain_text_layout", "comments_dropped", "footnotes_dropped", "revisions_dropped", "merged_cells_flattened", "tables_to_text"]),
        (Target::Pdf, vec![]),
        (Target::Docx, vec![]),
    ] {
        let r = s.convert(&rich, to);
        let mut got = codes(&r);
        let mut want: Vec<String> = want.into_iter().map(String::from).collect();
        got.sort();
        want.sort();
        assert_eq!(got, want, "docx to {}: {}", to.ext(), r.message);
    }
    // the same package without the features: nothing
    let bare_body = doc("<w:p/>");
    let bare = s.zip_bodies("bare.docx", &[("word/document.xml", &bare_body)]);
    for to in [Target::Odt, Target::Rtf, Target::Html, Target::Md, Target::Txt] {
        assert!(s.convert(&bare, to).warnings.is_empty(), "a bare docx to {} warned", to.ext());
    }
    // odt: comments and notes are lost going anywhere else, headers going to docx and pdf (the reader drops them)
    let odt = s.zip_bodies(
        "a.odt",
        &[
            ("mimetype", "application/vnd.oasis.opendocument.text"),
            ("content.xml", "<office:text><office:annotation office:name=\"c\"/><text:note text:id=\"n\"/></office:text>"),
            ("styles.xml", "<office:styles><style:header><text:p/></style:header></office:styles>"),
        ],
    );
    let r = s.convert(&odt, Target::Docx);
    let mut got = codes(&r);
    got.sort();
    assert_eq!(got, ["comments_dropped", "footnotes_dropped", "headers_footers_dropped"]);
    // rtf: a scanned file only gets the rows it has features for; and a real one with a comment says so
    let rtf = s.file("a.rtf", b"{\\rtf1\\ansi {\\*\\annotation c} text\\par}");
    assert_eq!(codes(&s.convert(&rtf, Target::Docx)), ["comments_dropped"]);
    let plain_rtf = s.file("b.rtf", b"{\\rtf1\\ansi text\\par}");
    assert!(s.convert(&plain_rtf, Target::Docx).warnings.is_empty());
    // html with a linked picture (and one inline) to docx; without a linked one nothing
    let html = s.file("a.html", b"<!DOCTYPE html><p><img src=\"order_files/a.png\"><img src=\"data:image/png;base64,AA\">");
    assert_eq!(codes(&s.convert(&html, Target::Docx)), ["html_images_not_embedded"]);
    let inline = s.file("b.html", b"<!DOCTYPE html><p><img src=\"data:image/png;base64,AA\">");
    assert!(s.convert(&inline, Target::Docx).warnings.is_empty());
}

#[test]
fn macros_dropped() {
    let s = Sandbox::new("vba");
    let r = s.convert(&s.docx("macro.docx", &["word/vbaProject.bin"]), Target::Pdf);
    assert_eq!(codes(&r), ["macros_dropped"], "{}", r.message);
}

#[test]
fn kind_mismatch() {
    let s = Sandbox::new("mismatch");
    let d = s.docx("really-a-docx.docx", &[]);
    let wrong = s.0.join("wrong.xlsx");
    std::fs::copy(&d, &wrong).unwrap();
    let r = s.convert(&wrong, Target::Pdf);
    assert_eq!(codes(&r), ["kind_mismatch"], "{}", r.message);
    assert!(r.warnings[0].text.contains(".docx"), "{}", r.warnings[0].text);
}

#[test]
fn upstream_lines_are_kept_and_repeats_are_counted() {
    let s = Sandbox::new("up");
    let r = s.steps(&s.cfg(), vec![(&["warn", "out/mid.pdf"], "out/mid.pdf", None), (&["warn", "out/output.pdf"], "out/output.pdf", None)]);
    let up: Vec<_> = r.warnings.iter().filter(|w| w.code == "upstream").collect();
    assert_eq!(up.len(), 1, "{:?}", r.warnings);
    assert_eq!(up[0].count, 2);
    assert!(up[0].text.starts_with("Warning:"));
}

#[test]
fn empty_text_from_a_scanned_pdf() {
    let s = Sandbox::new("empty");
    let r = s.steps(&s.cfg(), vec![(&["silent"], "out/output.txt", Some("out/output.txt"))]);
    assert_eq!(r.code, 0, "{}", r.message);
    assert_eq!(codes(&r), ["empty_text"]);
}

#[test]
fn translit_result_is_captured_and_nothing_to_convert_warns() {
    let s = Sandbox::new("tr");
    let ok = r#"{"direction":"cyrl","paragraphs":3,"runs":5,"skipped":{"lang":0},"rulesVersion":"uz-translit-rules/3"}"#;
    let r = s.steps(&s.cfg(), vec![(&["translit", "out/output.pdf", ok], "out/output.pdf", None)]);
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    assert_eq!(r.translit.as_ref().unwrap()["runs"], 5);
    // the sheets shape: {"command":..,"result":{..}} with formulas holding text
    let sheets = r#"{"command":"tools.translitCyrl","result":{"cells":18,"runs":18,"formulasWithText":3,"rulesVersion":"uz-translit-rules/3"}}"#;
    let r = s.steps(&s.cfg(), vec![(&["translit", "out/output.pdf", sheets], "out/output.pdf", None)]);
    assert_eq!(codes(&r), ["translit_formula_text"]);
    assert_eq!(r.warnings[0].count, 3);
    let none = r#"{"direction":"cyrl","paragraphs":4,"runs":0,"rulesVersion":"uz-translit-rules/3"}"#;
    let r = s.steps(&s.cfg(), vec![(&["translit", "out/output.pdf", none], "out/output.pdf", None)]);
    assert_eq!(codes(&r), ["translit_nothing"]);
    assert_eq!(r.translit.as_ref().unwrap()["paragraphs"], 4);
}

#[test]
fn every_code_has_catalog_text() {
    let mut emitted: Vec<String> = convert::losses::rows().iter().map(|r| r.code.clone()).collect();
    emitted.extend(
        ["upstream", "kind_mismatch", "empty_text", "translit_nothing", "translit_formula_text", "sidecar", "hidden_sheet_exported", "very_hidden_sheet_skipped", "xdfu_copy", "xdfu_refused", "xdfu_no_watermark", "text_transcoded"]
            .map(String::from),
    );
    emitted.sort();
    emitted.dedup();
    assert_eq!(emitted, report::codes(), "i18n/converter.tsv and the codes the converter emits must be the same set");
}

#[test]
fn verify_reopens_outputs() {
    let s = Sandbox::new("verify");
    let mut c = s.cfg();
    c.verify = true;
    let r = s.convert_with(&c, &s.docx("letter.docx", &[]), Target::Pdf);
    assert_eq!((r.code, r.verified), (0, Some(true)), "{}", r.message);
    assert!(r.steps.iter().any(|st| st.tool == "pdf-cli"), "the pdf was re-opened with pdf-cli");
    let r = s.steps(&c, vec![(&["ok", "out/output.pdf", "BADINFO"], "out/output.pdf", None)]);
    assert_eq!(r.code, Exit::BadOutput.code(), "{}", r.message);
    assert!(r.message.contains("--verify") && r.outputs.is_empty(), "{}", r.message);
    // without --verify the same file passes
    assert_eq!(s.steps(&s.cfg(), vec![(&["ok", "out/output.pdf", "BADINFO"], "out/output.pdf", None)]).code, 0);
}

#[test]
fn input_limits_are_exit_9_or_3() {
    let s = Sandbox::new("limits");
    // an honest bomb: 11 MB of zeros in a few KB
    let p = s.0.join("bomb.docx");
    let mut w = zip::ZipWriter::new(std::fs::File::create(&p).unwrap());
    w.start_file("word/document.xml", zip::write::SimpleFileOptions::default()).unwrap();
    w.write_all(&vec![0u8; 11 << 20]).unwrap();
    w.start_file("[Content_Types].xml", zip::write::SimpleFileOptions::default()).unwrap();
    w.finish().unwrap();
    let r = s.convert(&p, Target::Pdf);
    assert_eq!((r.code, r.status), (9, "failed"), "{}", r.message);
    assert!(r.message.contains("zip bomb"));
    // legacy-encoded text is transcoded under a text extension (tests/legacy_text.rs) and refused under any other, never converted as
    // garbage; so are unlisted text types
    let r = s.convert(&s.file("cp1251.docx",b"\xcf\xf0\xe8\xe2\xe5\xf2 \xec\xe8\xf0"), Target::Docx);
    assert_eq!(r.code, 3, "{}", r.message);
    assert!(r.message.contains("UTF-8"), "{}", r.message);
    let r = s.convert(&s.file("order.xml", b"<?xml version=\"1.0\"?><w:wordDocument/>"), Target::Docx);
    assert_eq!(r.code, 3, "{}", r.message);
    let r = s.convert(&s.file("data.json", b"{\"a\":1}"), Target::Docx);
    assert_eq!(r.code, 3, "{}", r.message);
}

#[test]
fn the_report_matches_its_schema() {
    let schema: serde_json::Value = serde_json::from_str(include_str!("../data/report.schema.json")).unwrap();
    let props: Vec<&String> = schema["properties"].as_object().unwrap().keys().collect();
    let s = Sandbox::new("schema");
    let mut c = s.cfg();
    c.verify = true;
    let ok = r#"{"runs":1,"rulesVersion":"x"}"#;
    let r = s.steps(&c, vec![(&["translit", "out/output.pdf", ok], "out/output.pdf", None)]);
    let v = serde_json::to_value(&r).unwrap();
    for k in v.as_object().unwrap().keys() {
        assert!(props.contains(&k), "report field {k} is not in the schema");
    }
    for k in schema["required"].as_array().unwrap() {
        assert!(v.get(k.as_str().unwrap()).is_some(), "required field {k} missing");
    }
    assert_eq!(v["schema"], schema["properties"]["schema"]["const"]);
    assert!(v.get("translit").is_some() && v.get("verified").is_some(), "optional fields appear when set");
}
