// WP2 step 1 native split (Phase 6 plan section 7). Built as an example of docs-bind (dev-dependencies wordcraft-layout/-fonts):
//   cargo run --release -p docs-bind --example p6_m3_layout -- <docx> [reps]
// Prints, per view, layout ms with a cold LayoutCache (shaping + pagination + repeated passes) and with a warm one (cache hits:
// pagination + repeated passes only), and the number of pages. System fonts are off (the web build has bundled fonts only).
use std::time::Instant;
use wordcraft_layout::{LayoutCache, LayoutOptions, ViewMode};

fn med(v: &mut [f64]) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[v.len() / 2], v[0], v[v.len() - 1])
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&a[1]).unwrap();
    let reps: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);
    wordcraft_fonts::FontDb::global().set_system_fallback(false);
    let mut parse = vec![];
    let mut doc = None;
    for _ in 0..reps {
        let t = Instant::now();
        doc = Some(wordcraft_engine::io::open_bytes("x.docx", &bytes).unwrap());
        parse.push(t.elapsed().as_secs_f64() * 1e3);
    }
    let doc = doc.unwrap();
    let (m, lo, hi) = med(&mut parse);
    println!("parse            median {m:8.1} ms  (min {lo:.1}, max {hi:.1})");
    for (name, view) in [("Print", ViewMode::Print), ("Web", ViewMode::Web), ("Read", ViewMode::Read)] {
        let opts = LayoutOptions { view, web_width: 360.0, read_height: 700.0, ..Default::default() };
        let (mut cold, mut warm, mut pages, mut hits0, mut hits1) = (vec![], vec![], 0, 0, 0);
        for _ in 0..reps {
            let mut cache = LayoutCache::new();
            let t = Instant::now();
            let l = wordcraft_layout::layout(&doc, &mut cache, &opts);
            cold.push(t.elapsed().as_secs_f64() * 1e3);
            pages = l.pages.len();
            hits0 = cache.hits;
            let t = Instant::now();
            let _ = wordcraft_layout::layout(&doc, &mut cache, &opts);
            warm.push(t.elapsed().as_secs_f64() * 1e3);
            hits1 = cache.hits;
        }
        let (c, clo, chi) = med(&mut cold);
        let (w, wlo, whi) = med(&mut warm);
        println!("{name:5} pages {pages:4}  cold {c:8.1} ms ({clo:.1}-{chi:.1})  warm {w:8.1} ms ({wlo:.1}-{whi:.1})  shaping+paragraph = {:5.1}% of cold  cache hits cold-run {hits0}, warm-run +{}", 100.0 * (c - w) / c, hits1 - hits0);
    }
}
