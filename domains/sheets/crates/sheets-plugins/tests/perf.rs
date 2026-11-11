//! WP13 N1 for Sheets (PLAN section 6.3): the built-in and the uz-translit plugin (its wasm module in the native host) on the same
//! workbook, alternating in this process, release profile. Needs `UZ_TRANSLIT_PLUGIN_WASM`; run with `--ignored --nocapture`.
//! Pass: plugin command <= built-in + 50 ms. Sheets has no relayout step, so the command is the whole cost (the grid is drawn on the
//! next frame from the changed cells).
use std::time::Instant;

use gridcraft_engine::Session;
use plugin_adapter::plugin_api::manifest;
use plugin_adapter::Plugin;
use serde_json::{json, Value};

const MANIFEST: &str = include_str!("../../../../../plugins/uz-translit/plugin.toml");
const WORDS: [&str; 6] = ["Toshkent shahri", "O'zbekiston Respublikasi", "yangi ish o'rni", "Xodimlar bo'limi", "mehnat shartnomasi", "ish haqi"];

fn workbook(rows: usize) -> Session {
    let mut s = Session::new();
    s.new_workbook();
    let values: Vec<Vec<Value>> =
        (0..rows).map(|i| (0..5).map(|c| if c % 5 == 4 { json!(i as f64) } else { json!(WORDS[(i + c) % WORDS.len()]) }).collect()).collect();
    s.execute("range.setValues", json!({"range": "A1", "values": values})).unwrap();
    s.execute("selection.set", json!({"range": "A1"})).unwrap();
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
    plugin_adapter::set_app("sheets");
    plugin_adapter::add(Plugin::new(manifest::parse(MANIFEST).unwrap(), plugin_adapter::native::wasm_guest(&wasm, 32).unwrap())).unwrap();
    sheets_plugins::register();
    for rows in [2000usize, 20_000] {
        let base = workbook(rows);
        for (builtin, plugin, label) in [("tools.translitCyrl", "plugin.uz-translit.toCyrillic", "to Cyrillic"), ("tools.translitLatn", "plugin.uz-translit.toLatin", "to Latin")] {
            // to Latin runs on Cyrillic text: convert the base once with the built-in
            let mut start = Session::new();
            start.add_document(base.doc().unwrap().clone());
            if label == "to Latin" {
                start.execute("tools.translitCyrl", json!({"scope": "workbook"})).unwrap();
            }
            let mut t = [vec![], vec![]];
            for round in 0..6 {
                for k in 0..2 {
                    let which = (k + round) % 2;
                    let mut s = Session::new();
                    s.add_document(start.doc().unwrap().clone());
                    let (id, scope) = [(builtin, "workbook"), (plugin, "document")][which];
                    let t0 = Instant::now();
                    s.execute(id, json!({"scope": scope})).unwrap();
                    if round > 0 {
                        t[which].push(t0.elapsed().as_secs_f64() * 1e3);
                    }
                }
            }
            let [b, p] = [median(&mut t[0]), median(&mut t[1])];
            println!("N1 sheets {rows}x5 {label}: command built-in {b:.0} ms / plugin {p:.0} ms (+{:.0}, {:+.1} %) | {} (median of 5, alternating)", p - b, (p / b - 1.0) * 100.0, if p <= b + 50.0 { "PASS" } else { "FAIL" });
        }
    }
    plugin_adapter::clear();
}
