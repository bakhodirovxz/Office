//! WP13 N1 for Slides (PLAN section 6.3): the built-in and the uz-translit plugin (its wasm module in the native host) on the same
//! generated deck, alternating in this process, release profile. Needs `UZ_TRANSLIT_PLUGIN_WASM`; run with `--ignored --nocapture`.
//! Pass: plugin command <= built-in + 50 ms. Slides lays out on the next frame from the changed shapes; the command is the cost measured.
use std::sync::Arc;
use std::time::Instant;

use deckcraft_engine::model::text::{Run, RunKind};
use deckcraft_engine::model::{Paragraph, ShapeId};
use deckcraft_engine::Session;
use plugin_adapter::plugin_api::manifest;
use plugin_adapter::Plugin;
use serde_json::json;

const MANIFEST: &str = include_str!("../../../../../plugins/uz-translit/plugin.toml");
const WORDS: [&str; 5] = ["Toshkent shahri", "O'zbekiston Respublikasi", "yangi ish o'rni", "Xodimlar bo'limi", "mehnat shartnomasi"];

fn deck(slides: usize) -> Session {
    let mut s = Session::with_new();
    for i in 0..slides {
        if i > 0 {
            s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
        }
        let r = s.execute("shape.insert", &json!({"preset": "rect", "rect": [40.0, 40.0, 400.0, 120.0], "text": "x"})).unwrap();
        let id = ShapeId(r["id"].as_u64().unwrap() as _);
        let run = |t: String| Run { text: t, props: Default::default(), kind: RunKind::Text };
        s.edit(|doc, _| {
            let sh = deckcraft_engine::model::find_shape_mut(&mut Arc::make_mut(&mut doc.slides[i]).shapes, id).unwrap();
            sh.text.as_mut().unwrap().paragraphs = (0..8).map(|k| Paragraph { runs: vec![run(format!("{} {} ", WORDS[(i + k) % WORDS.len()], k))], ..Default::default() }).collect();
            Ok(())
        })
        .unwrap();
    }
    s.execute("slide.go", &json!({"index": 0})).unwrap();
    s
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

#[test]
#[ignore]
fn perf_plugin() {
    let Some(path) = std::env::var_os("UZ_TRANSLIT_PLUGIN_WASM") else { return };
    let wasm = std::fs::read(path).unwrap();
    plugin_adapter::set_app("slides");
    plugin_adapter::add(Plugin::new(manifest::parse(MANIFEST).unwrap(), plugin_adapter::native::wasm_guest(&wasm, 32).unwrap())).unwrap();
    slides_plugins::register();
    for n in [40usize, 400] {
        let base = deck(n);
        for (builtin, plugin, label) in [("tools.translitCyrl", "plugin.uz-translit.toCyrillic", "to Cyrillic"), ("tools.translitLatn", "plugin.uz-translit.toLatin", "to Latin")] {
            let mut start = Session::new();
            start.add_document(base.doc().unwrap().clone());
            if label == "to Latin" {
                start.execute("tools.translitCyrl", &json!({"scope": "deck"})).unwrap();
            }
            let mut t = [vec![], vec![]];
            for round in 0..6 {
                for k in 0..2 {
                    let which = (k + round) % 2;
                    let mut s = Session::new();
                    s.add_document(start.doc().unwrap().clone());
                    let (id, scope) = [(builtin, "deck"), (plugin, "document")][which];
                    let t0 = Instant::now();
                    s.execute(id, &json!({"scope": scope})).unwrap();
                    if round > 0 {
                        t[which].push(t0.elapsed().as_secs_f64() * 1e3);
                    }
                }
            }
            let [b, p] = [median(&mut t[0]), median(&mut t[1])];
            println!("N1 slides {n} slides {label}: command built-in {b:.0} ms / plugin {p:.0} ms (+{:.0}, {:+.1} %) | {} (median of 5, alternating)", p - b, (p / b - 1.0) * 100.0, if p <= b + 50.0 { "PASS" } else { "FAIL" });
        }
    }
    plugin_adapter::clear();
}
