//! Planner table test (PLAN WP2 step 3): every (kind, target, options) maps to a fixed argv. The golden file is
//! `tests/golden/plan.txt`; regenerate with `UPDATE_GOLDEN=1 cargo test --test plan_golden` and read the diff.
use convert::kind::Kind;
use convert::plan::*;
use serde_json::json;

fn show(steps: &[Step]) -> String {
    steps
        .iter()
        .map(|s| match s {
            Step::Tool(t) => {
                let mut x = format!("{} {}", t.tool.name(), t.argv.join(" "));
                if let Some(o) = &t.stdout_to {
                    x += &format!(" > {o}");
                }
                for (f, b) in &t.write_files {
                    x += &format!(" [writes {f}: {b}]");
                }
                x
            }
            Step::Expand(e) => format!("EXPAND {e:?}"),
        })
        .collect::<Vec<_>>()
        .join("\n    ")
}

fn line(label: &str, k: Kind, t: Target, o: &Options) -> String {
    match plan(k, t, o) {
        Ok(s) => format!("{label} {}->{}:\n    {}\n", k.ext(), t.ext(), show(&s)),
        Err(e) => format!("{label} {}->{}: {e:?}\n", k.ext(), t.ext()),
    }
}

fn tr(dir: TranslitDir, alphabet: &'static str) -> Option<Translit> {
    Some(Translit { dir, alphabet, apostrophe: "official" })
}

fn table() -> String {
    let mut s = String::new();
    let base = Options::default();
    // 1. every row, default options
    for (k, t) in rows() {
        s += &line("row", k, t, &base);
    }
    // 2. every (kind, target) NOT in the table: the refusal
    for k in Kind::ALL {
        for t in Target::ALL {
            if !rows().contains(&(k, t)) {
                s += &line("none", k, t, &base);
            }
        }
    }
    // 3. option variants
    let cyrl = Options { translit: tr(TranslitDir::Cyrl, "1995"), ..Options::default() };
    let latn26 = Options { translit: tr(TranslitDir::Latn, "2026"), ..Options::default() };
    let auto = Options { translit: tr(TranslitDir::Auto, "auto"), ..Options::default() };
    for (k, t) in [
        (Kind::Docx, Target::Docx), (Kind::Doc, Target::Pdf), (Kind::Md, Target::Png), (Kind::Xlsx, Target::Xlsx),
        (Kind::Xls, Target::Pdf), (Kind::Pptx, Target::Pptx), (Kind::Pptx, Target::Pdf), (Kind::Pptx, Target::Png),
        (Kind::Csv, Target::Csv), (Kind::Pdf, Target::Docx),
    ] {
        s += &line("translit-cyrl", k, t, &cyrl);
    }
    s += &line("translit-latn-2026", Kind::Docx, Target::Docx, &latn26);
    s += &line("translit-auto", Kind::Xlsx, Target::Pdf, &auto);
    for sheet in [SheetSel::Active, SheetSel::All, SheetSel::Name("Q1 2026".into()), SheetSel::Name("-x".into())] {
        s += &line(&format!("sheet={sheet:?}"), Kind::Xlsx, Target::Csv, &Options { sheet, ..Options::default() });
    }
    s += &line("sheet-on-pdf", Kind::Xlsx, Target::Pdf, &Options { sheet: SheetSel::All, ..Options::default() });
    let pages = Options { pages: Some(PageSpec::parse("1,3-5").unwrap()), dpi: 300, ..Options::default() };
    s += &line("pages+dpi", Kind::Docx, Target::Png, &pages);
    s += &line("pages+dpi", Kind::Pptx, Target::Png, &pages);
    s += &line("pages+dpi", Kind::Pdf, Target::Png, &pages);
    s += &line("pages-on-pdf-out", Kind::Docx, Target::Pdf, &pages);
    s += &line("pdf-text-page", Kind::Pdf, Target::Txt, &Options { pages: Some(PageSpec::parse("2").unwrap()), ..Options::default() });
    s += &line("pdf-text-pages", Kind::Pdf, Target::Txt, &pages);
    s += &line("pdf-sidecar", Kind::Pdf, Target::Docx, &Options { pdf_engine: PdfEngine::Sidecar, ..Options::default() });
    // 4. dynamic expansions, with a fixture `info`
    let info = json!({"pages": 5});
    s += &format!("expand pages all: {}\n", show(&expand_pages("mid/doc.pdf", &None, 150, &info).unwrap()));
    s += &format!("expand pages 1,3-4 @300: {}\n", show(&expand_pages("in/input.pdf", &Some(PageSpec::parse("1,3-4").unwrap()), 300, &info).unwrap()));
    s += &format!("expand pages out of range: {:?}\n", expand_pages("in/input.pdf", &Some(PageSpec::parse("6").unwrap()), 150, &info).unwrap_err());
    let sheets = json!({"sheets": [{"name": "Sheet 1"}, {"name": "sheet_1"}, {"name": "Ёлка"}, {"name": "A/B"}, {"name": ""}]});
    s += &format!("expand csv: {}\n", show(&expand_csv("in/input.xlsx", None, &sheets).unwrap()));
    s += &format!("expand csv only Sheet 1: {}\n", show(&expand_csv("in/input.xlsx", Some("Sheet 1"), &sheets).unwrap()));
    s += &format!("expand csv only wrong: {:?}\n", expand_csv("in/input.xlsx", Some("Nope"), &sheets).unwrap_err());
    // 5. the PDF subcommands
    s += &format!("pdf combine 3: {}\n", show(&pdf_combine(3)));
    s += &format!("pdf extract: {}\n", show(&pdf_extract(&PageSpec::parse("5-8,1").unwrap())));
    s += &format!("pdf split every: {}\n", show(&pdf_split(&SplitBy::Every(2))));
    s += &format!("pdf split before: {}\n", show(&pdf_split(&SplitBy::Before(vec![3, 7]))));
    s
}

#[test]
fn planner_table_matches_golden() {
    let got = table();
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/plan.txt");
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(path, &got).unwrap();
    }
    let want = std::fs::read_to_string(path).expect("golden file missing; run with UPDATE_GOLDEN=1");
    assert_eq!(got.replace("\r\n", "\n"), want.replace("\r\n", "\n"), "planner output changed; review and regenerate");
}

#[test]
fn rows_have_no_duplicates_and_formats_json_agrees() {
    let r = rows();
    let mut sorted = r.clone();
    sorted.dedup();
    assert_eq!(r.len(), sorted.len());
    let j = convert::formats::formats_json();
    assert_eq!(j["rows"].as_array().unwrap().len(), r.len());
    assert_eq!(j["unsupported_inputs"], json!(["ppt", "odp"]));
}

#[test]
fn page_spec_parsing() {
    assert_eq!(PageSpec::parse("1,3-5").unwrap().canonical(), "1,3-5");
    assert_eq!(PageSpec::parse("2-2").unwrap().canonical(), "2");
    for bad in ["", "0", "a", "3-1", "1,,2", "1-", "-2"] {
        assert!(PageSpec::parse(bad).is_err(), "{bad}");
    }
}
