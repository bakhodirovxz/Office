//! WP10a/WP10 N1: the 108-page document, engine-level timings (run with `--release -- --ignored --nocapture`).
use std::time::Instant;

use wordcraft_doc::StoryRef;
use wordcraft_engine::{Session, io};

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

#[test]
#[ignore]
fn engine_baseline_108() {
    for name in ["uz108-latn.docx", "uz108-cyrl.docx"] {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        let bytes = std::fs::read(&path).unwrap();
        let (mut open, mut lay, mut iter, mut conv, mut save, mut relay) =
            (vec![], vec![], vec![], vec![], vec![], vec![]);
        let mut pages = 0;
        let (mut chars, mut runs, mut paras) = (0, 0, 0);
        for round in 0..6 {
            let t = Instant::now();
            let (doc, _) = io::open_bytes_with(name, &bytes, None).unwrap();
            let o = t.elapsed().as_secs_f64() * 1e3;
            let mut s = Session::new(doc);
            let t = Instant::now();
            let l = s.layout();
            let ll = t.elapsed().as_secs_f64() * 1e3;
            pages = l.pages.len();
            // iterate every run of every story
            let t = Instant::now();
            let (mut c, mut r, mut p) = (0usize, 0usize, 0usize);
            let mut stories = vec![StoryRef::Body];
            stories.extend(s.doc.parts.keys().map(|k| StoryRef::Part(*k)));
            let mut text = String::new();
            for st in stories {
                for path in s.doc.para_paths(st) {
                    let para = s.doc.para(st, &path).unwrap();
                    p += 1;
                    for (range, _props) in para.run_ranges() {
                        r += 1;
                        c += range.len();
                        text.push_str(&para.text[range]);
                    }
                }
            }
            let it = t.elapsed().as_secs_f64() * 1e3;
            let t = Instant::now();
            let dir = if name.contains("latn") {
                uz_translit::Direction::ToCyrillic
            } else {
                uz_translit::Direction::ToLatin
            };
            let out = uz_translit::convert(&text, dir, &uz_translit::Options::default());
            let cv = t.elapsed().as_secs_f64() * 1e3;
            std::hint::black_box(&out);
            let t = Instant::now();
            let saved = io::save_bytes("out.docx", &s.doc).unwrap();
            let sv = t.elapsed().as_secs_f64() * 1e3;
            std::hint::black_box(&saved);
            // a full-document edit: touch every paragraph, then lay out again
            let t = Instant::now();
            for st in std::iter::once(StoryRef::Body) {
                for path in s.doc.para_paths(st) {
                    s.doc.para_mut(st, &path).unwrap().touch();
                }
            }
            s.touch();
            let l2 = s.layout();
            let rl = t.elapsed().as_secs_f64() * 1e3;
            std::hint::black_box(&l2);
            (chars, runs, paras) = (c, r, p);
            if round > 0 {
                open.push(o);
                lay.push(ll);
                iter.push(it);
                conv.push(cv);
                save.push(sv);
                relay.push(rl);
            }
        }
        println!(
            "{name}: pages {pages}, paragraphs {paras}, runs {runs}, text {chars} B | open {:.0} ms, layout {:.0}, iterate runs {:.2}, convert whole text {:.1}, save {:.0}, touch-all+relayout {:.0} (median of 5, 1 warm-up)",
            median(&mut open),
            median(&mut lay),
            median(&mut iter),
            median(&mut conv),
            median(&mut save),
            median(&mut relay)
        );
    }
}

/// N1: the command on the 108-page document, headless: wall time from `Session::run` to a finished full relayout.
#[test]
#[ignore]
fn perf_108() {
    for (name, id) in [
        ("uz108-latn.docx", "tools.translitCyrl"),
        ("uz108-cyrl.docx", "tools.translitLatn"),
    ] {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        let bytes = std::fs::read(&path).unwrap();
        let (mut cmd, mut lay, mut tot, mut undo, mut redo, mut ours) =
            (vec![], vec![], vec![], vec![], vec![], vec![]);
        let mut pages = 0;
        let mut res = serde_json::Value::Null;
        for round in 0..6 {
            let (doc, _) = io::open_bytes_with(name, &bytes, None).unwrap();
            let mut s = Session::new(doc);
            docs_translit::install(&mut s);
            s.layout(); // the document is already shown
            let t = Instant::now();
            res = s.run(id, &serde_json::json!({})).unwrap();
            let c = t.elapsed().as_secs_f64() * 1e3;
            let l = s.layout();
            let all = t.elapsed().as_secs_f64() * 1e3;
            pages = l.pages.len();
            let t = Instant::now();
            s.run("edit.undo", &serde_json::json!({})).unwrap();
            s.layout();
            let u = t.elapsed().as_secs_f64() * 1e3;
            let t = Instant::now();
            s.run("edit.redo", &serde_json::json!({})).unwrap();
            s.layout();
            let r = t.elapsed().as_secs_f64() * 1e3;
            if round > 0 {
                cmd.push(c);
                lay.push(all - c);
                tot.push(all);
                undo.push(u);
                redo.push(r);
                ours.push(
                    res["ms"]["convert"].as_f64().unwrap() + res["ms"]["apply"].as_f64().unwrap(),
                );
            }
        }
        println!(
            "{name} {id}: pages after {pages}, paragraphs {}, runs {} | run() {:.0} ms (ours: convert+apply {:.0}, of which result.ms {}), relayout {:.0}, command->layout {:.0}, undo->layout {:.0}, redo->layout {:.0} (median of 5)",
            res["paragraphs"],
            res["runs"],
            median(&mut cmd),
            median(&mut ours),
            res["ms"],
            median(&mut lay),
            median(&mut tot),
            median(&mut undo),
            median(&mut redo)
        );
    }
}
