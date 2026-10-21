//! WP4 done-check, end to end through `convert` with the real domain CLIs and the Phase 2 fixtures and comparers.
//! Skipped (loudly) when the release CLIs are not built: they are looked up under `$OFFICE_TARGET` (default
//! `E:/office-target/p1`) as `<domain>/release/<tool>-cli(.exe)`, or through `CONVERT_*_CLI` / `CONVERT_TOOLS_DIR`.
//!
//! - docx: for each of the 9 Phase 2 Word-written fixtures, `convert --translit cyrl` (R) against `convert` alone (B): every
//!   part except the text parts is byte-identical (the rule of `docs-translit/tests/preserve.rs`), and R equals what the Phase 2
//!   route (`docs-cli run --cmd tools.translitCyrl={}`) writes, part for part.
//! - xlsx and pptx: the Phase 2 comparer `scripts/translit/preserve.py` on B and R from `convert`, both rounds (to Cyrillic,
//!   then back), as `scripts/translit/preserve-check.sh` does.
//! - 2026: the approved both-direction rows of `crates/uz-translit/data/golden.tsv` through `--alphabet 2026`, both directions.
use convert::exec::Config;
use convert::job::{self, Dest};
use convert::plan::{Options, Target, ToolId, Translit, TranslitDir};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn config(work: &Path) -> Option<Config> {
    let target = std::env::var("OFFICE_TARGET").unwrap_or_else(|_| "E:/office-target/p1".into());
    let mut c = Config::from_env();
    c.work_dir = work.join("work");
    for (id, dir) in [(ToolId::Docs, "docs"), (ToolId::Sheets, "sheets"), (ToolId::Slides, "slides"), (ToolId::Pdf, "pdf")] {
        let p = PathBuf::from(&target).join(dir).join("release").join(format!("{}{}", id.name(), std::env::consts::EXE_SUFFIX));
        if c.resolve(id).is_err() {
            if !p.is_file() {
                eprintln!("SKIPPED translit e2e: {} not built", p.display());
                return None;
            }
            c.tools.insert(id, p);
        }
    }
    Some(c)
}

struct Work(PathBuf);
impl Work {
    fn new(name: &str) -> Work {
        let p = std::env::temp_dir().join(format!("convert-e2e-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Work(p)
    }
}
impl Drop for Work {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tr(dir: TranslitDir, alphabet: &'static str) -> Options {
    Options { translit: Some(Translit { dir, alphabet, apostrophe: "official" }), ..Options::default() }
}

/// Convert `input` to `to` with `opts` into `out_dir`; the single output path.
fn run(cfg: &Config, input: &Path, to: Target, opts: &Options, out_dir: &Path) -> PathBuf {
    let r = job::convert_file(cfg, input, to, opts, &Dest { out_dir: out_dir.into(), out_file: None });
    assert_eq!(r.code, 0, "{}: {}", input.display(), r.message);
    assert!(r.outputs.len() == 1, "{:?}", r.outputs);
    r.outputs[0].clone().into()
}

/// `dcterms:modified` is the save time, so two saves a second apart differ in nothing else.
fn without_modified(v: &[u8]) -> Vec<u8> {
    let t = String::from_utf8_lossy(v);
    match (t.find("<dcterms:modified"), t.find("</dcterms:modified>")) {
        (Some(a), Some(b)) => format!("{}{}", &t[..a], &t[b + "</dcterms:modified>".len()..]).into_bytes(),
        _ => v.to_vec(),
    }
}

fn parts(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut m = BTreeMap::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        if f.is_dir() {
            continue;
        }
        let mut v = vec![];
        f.read_to_end(&mut v).unwrap();
        let name = f.name().unwrap().into_owned();
        let v = if name == "docProps/core.xml" { without_modified(&v) } else { v };
        m.insert(name, v);
    }
    m
}

/// Text parts of the Phase 2 rule, plus `docProps/app.xml`, whose word statistics follow the text (Cyrillic words count
/// differently from digraph-laden Latin ones).
fn is_word_text_part(name: &str) -> bool {
    if name == "docProps/app.xml" {
        return true;
    }
    let n = name.strip_prefix("word/").unwrap_or("");
    n == "document.xml" || n == "footnotes.xml" || n == "endnotes.xml" || ((n.starts_with("header") || n.starts_with("footer") || n.starts_with("comments")) && n.ends_with(".xml"))
}

#[test]
fn docx_fixtures_match_the_phase_2_route() {
    let w = Work::new("docx");
    let Some(cfg) = config(&w.0) else { return };
    let dir = root().join("domains/docs/crates/docs-translit/tests/fixtures/word");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).collect();
    files.sort();
    assert_eq!(files.len(), 9, "the 9 Phase 2 docx fixtures");
    let docs_cli = cfg.resolve(ToolId::Docs).unwrap();
    let mut changed_text = 0;
    for f in &files {
        let name = f.file_stem().unwrap().to_string_lossy().into_owned();
        let b = run(&cfg, f, Target::Docx, &Options::default(), &w.0.join(format!("b-{name}")));
        let r = run(&cfg, f, Target::Docx, &tr(TranslitDir::Cyrl, "1995"), &w.0.join(format!("r-{name}")));
        let (pb, pr) = (parts(&b), parts(&r));
        assert_eq!(pb.keys().collect::<Vec<_>>(), pr.keys().collect::<Vec<_>>(), "{name}: the part lists differ");
        for (k, v) in &pb {
            assert!(is_word_text_part(k) || v == &pr[k], "{name}: non-text part {k} differs between B and R");
        }
        if pb.iter().any(|(k, v)| is_word_text_part(k) && v != &pr[k]) {
            changed_text += 1;
        }
        // the Phase 2 route: docs-cli run --cmd tools.translitCyrl={} --save
        let p2 = w.0.join(format!("p2-{name}.docx"));
        let out = Command::new(&docs_cli).args(["run", "--file"]).arg(f).args(["--cmd", "tools.translitCyrl={}", "--save"]).arg(&p2).output().unwrap();
        assert!(out.status.success(), "{name}: {}", String::from_utf8_lossy(&out.stderr));
        let p2parts = parts(&p2);
        for (k, v) in &pr {
            if v != &p2parts[k] {
                panic!("{name}: part {k} differs between convert --translit cyrl and the Phase 2 route
  C: {}
  P: {}", String::from_utf8_lossy(v), String::from_utf8_lossy(&p2parts[k]));
            }
        }
    }
    assert!(changed_text >= 7, "the conversion must really change text in most fixtures (changed in {changed_text} of 9)");
    eprintln!("docx: 9 fixtures: non-text parts byte-identical (B vs R), R equal to the Phase 2 route; text changed in {changed_text}");
}

fn preserve(kind: &str, b: &Path, r: &Path) -> String {
    let out = Command::new("python").args(["-I"]).arg(root().join("scripts/translit/preserve.py")).arg(kind).arg(b).arg(r).output().expect("python is needed for preserve.py");
    assert!(out.status.success(), "preserve.py {kind}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn office_fixtures(kind: &str, dir: &str, to: Target) {
    let w = Work::new(kind);
    let Some(cfg) = config(&w.0) else { return };
    let mut files: Vec<PathBuf> = std::fs::read_dir(root().join(dir)).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == kind)).collect();
    files.sort();
    assert_eq!(files.len(), 2, "the 2 Phase 2 {kind} fixtures");
    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy().into_owned();
        // round 1 to Cyrillic, round 2 takes that file and goes back to Latin (the order of preserve-check.sh)
        let b1 = run(&cfg, f, to, &Options::default(), &w.0.join("b1"));
        let r1 = run(&cfg, f, to, &tr(TranslitDir::Cyrl, "1995"), &w.0.join("r1"));
        assert_eq!(preserve(kind, &b1, &r1), "IDENTICAL", "{name} to Cyrillic");
        let b2 = run(&cfg, &r1, to, &Options::default(), &w.0.join("b2"));
        let r2 = run(&cfg, &r1, to, &tr(TranslitDir::Latn, "1995"), &w.0.join("r2"));
        assert_eq!(preserve(kind, &b2, &r2), "IDENTICAL", "{name} to Latin");
        for d in ["b1", "r1", "b2", "r2"] {
            let _ = std::fs::remove_dir_all(w.0.join(d));
        }
        eprintln!("{kind} {name}: IDENTICAL both rounds");
    }
}

#[test]
fn xlsx_fixtures_pass_preserve_py() {
    office_fixtures("xlsx", "domains/sheets/tests/translit-fixtures/xlsx", Target::Xlsx);
}

#[test]
fn pptx_fixtures_pass_preserve_py() {
    office_fixtures("pptx", "domains/slides/tests/translit-fixtures/pptx", Target::Pptx);
}

fn is_apos(c: char) -> bool {
    matches!(c, '\u{2BB}' | '\u{2BC}' | '\u{2018}' | '\u{2019}' | '\'' | '`' | '\u{2BF}' | '\u{B4}')
}

/// oʻ gʻ sh ch -> ö ğ ş ç (case of the first letter); s + apostrophe + h -> sh. A copy of `crates/uz-translit/tests/common`
/// (rules.md section 15.2): the 2026 Latin is the golden Latin passed through this substitution.
fn to_2026(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut o = String::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        let lc = c.to_ascii_lowercase();
        let n1 = cs.get(i + 1).copied();
        let l1 = n1.map(|x| x.to_ascii_lowercase());
        let l2 = cs.get(i + 2).map(|x| x.to_ascii_lowercase());
        let letter = |lo: char| if c.is_ascii_uppercase() { lo.to_uppercase().next().unwrap() } else { lo };
        match (lc, n1) {
            ('o', Some(a)) if is_apos(a) => {
                o.push(letter('ö'));
                i += 2;
            }
            ('g', Some(a)) if is_apos(a) => {
                o.push(letter('ğ'));
                i += 2;
            }
            ('s', Some(a)) if is_apos(a) && l2 == Some('h') => {
                o.push(c);
                o.push(cs[i + 2]);
                i += 3;
            }
            ('s', _) if l1 == Some('h') => {
                o.push(letter('ş'));
                i += 2;
            }
            ('c', _) if l1 == Some('h') => {
                o.push(letter('ç'));
                i += 2;
            }
            _ => {
                o.push(c);
                i += 1;
            }
        }
    }
    o
}

#[test]
fn alphabet_2026_round_trips_the_phase_2_rows() {
    let w = Work::new("a2026");
    let Some(cfg) = config(&w.0) else { return };
    let tsv = std::fs::read_to_string(root().join("crates/uz-translit/data/golden.tsv")).unwrap();
    let mut rows: Vec<(String, String, String)> = vec![]; // id, latin 1995, cyrillic
    for l in tsv.lines().skip(1) {
        let c: Vec<&str> = l.split('\t').collect();
        let (id, lat, cyr, dir, rule, status) = (c[0], c[1], c[2], c[3], c[4], c[5]);
        // the same selection as crates/uz-translit/tests/golden.rs golden_2026, plus plain one-line text
        if !status.starts_with("reviewed:") || dir != "both" || rule.contains("R15") || cyr.chars().any(|ch| ch.is_ascii_alphabetic()) {
            continue;
        }
        if lat.is_empty() || lat != lat.trim() || cyr != cyr.trim() || lat.contains(['\\', '\n', '\t']) {
            continue;
        }
        rows.push((id.into(), lat.into(), cyr.into()));
    }
    assert!(rows.len() >= 500, "{} rows", rows.len());
    let write = |name: &str, lines: Vec<&str>| -> PathBuf {
        let p = w.0.join(name);
        std::fs::write(&p, lines.join("\n") + "\n").unwrap();
        p
    };
    let l26: Vec<String> = rows.iter().map(|r| to_2026(&r.1)).collect();
    let cyr_in = write("cyr.txt", rows.iter().map(|r| r.2.as_str()).collect());
    let lat_in = write("lat26.txt", l26.iter().map(String::as_str).collect());
    let read = |p: PathBuf| -> Vec<String> { std::fs::read_to_string(p).unwrap().lines().map(String::from).collect() };
    // Cyrillic -> 2026 Latin, and 2026 Latin -> Cyrillic, one paragraph per row, through the plain-text route of docs-cli
    let to_latin = read(run(&cfg, &cyr_in, Target::Txt, &tr(TranslitDir::Latn, "2026"), &w.0.join("o1")));
    let to_cyr = read(run(&cfg, &lat_in, Target::Txt, &tr(TranslitDir::Cyrl, "2026"), &w.0.join("o2")));
    let mut bad = vec![];
    if to_latin.len() != rows.len() || to_cyr.len() != rows.len() {
        panic!("line counts changed: {} rows, {} and {} lines back", rows.len(), to_latin.len(), to_cyr.len());
    }
    for (i, (id, _, cyr)) in rows.iter().enumerate() {
        if to_latin[i] != l26[i] {
            bad.push(format!("{id} c2l {cyr:?} -> {:?}, expected {:?}", to_latin[i], l26[i]));
        }
        if to_cyr[i] != *cyr {
            bad.push(format!("{id} l2c {:?} -> {:?}, expected {cyr:?}", l26[i], to_cyr[i]));
        }
    }
    assert!(bad.is_empty(), "{} mismatches of {} rows\n{}", bad.len(), rows.len(), bad.iter().take(25).cloned().collect::<Vec<_>>().join("\n"));
    eprintln!("2026: {} rows round-trip in both directions through convert", rows.len());
}
