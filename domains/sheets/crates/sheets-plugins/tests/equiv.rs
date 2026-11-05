//! PLAN section 6.3 for Sheets (Phase 7 WP13): the transliteration done by the uz-translit plugin through the text map equals the
//! built-in command: the same workbook in memory, the same saved xlsx bytes, and one undo restores it. Two guests: the plugin's own
//! code compiled natively (`uz_translit_plugin::handle`, always), and its wasm module in the native host (`UZ_TRANSLIT_PLUGIN_WASM`,
//! built by `cargo xt plugin build uz-translit`; skipped with a note when unset).
//! Matrix: the two xlsx fixtures of `domains/sheets/tests/translit-fixtures` and a generated 2,000 x 5 sheet; to Cyrillic, then back
//! to Latin on the result, then auto; scopes auto (one cell selected = the sheet), selection (a block) and workbook (`document`).
use std::sync::{Arc, Mutex};

use gridcraft_engine::{DocState, Session, io};
use plugin_adapter::plugin_api::action::{Answer, Call};
use plugin_adapter::plugin_api::manifest;
use plugin_adapter::plugin_api::wire::Message;
use plugin_adapter::{Guest, GuestSession, Plugin};
use serde_json::{Value, json};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/translit-fixtures/xlsx");
const MANIFEST: &str = include_str!("../../../../../plugins/uz-translit/plugin.toml");

struct Native;
struct NativeSession;

impl Guest for Native {
    fn start(&self) -> Result<Box<dyn GuestSession>, String> {
        Ok(Box::new(NativeSession))
    }
}

impl GuestSession for NativeSession {
    fn round(&mut self, m: &Message) -> Result<Message, String> {
        let call: Call = m.header_as().map_err(|e| e.to_string())?;
        let r = uz_translit_plugin::handle(call, m.blobs.clone())?;
        Message::new(&Answer { actions: r.actions, next: r.next }, r.blobs).map_err(|e| e.to_string())
    }
}

static TURN: Mutex<()> = Mutex::new(());

fn guests() -> Vec<(&'static str, Arc<dyn Guest>)> {
    let mut g: Vec<(&'static str, Arc<dyn Guest>)> = vec![("native", Arc::new(Native))];
    match std::env::var_os("UZ_TRANSLIT_PLUGIN_WASM").map(std::fs::read) {
        Some(Ok(w)) => g.push(("wasm", plugin_adapter::native::wasm_guest(&w, 32).expect("the module loads"))),
        Some(Err(e)) => panic!("UZ_TRANSLIT_PLUGIN_WASM: {e}"),
        None => eprintln!("UZ_TRANSLIT_PLUGIN_WASM is not set: the wasm module was not run (cargo xt plugin build uz-translit)"),
    }
    g
}

const WORDS: [&str; 6] = ["Toshkent shahri", "O'zbekiston Respublikasi", "yangi ish o'rni", "Xodimlar bo'limi", "mehnat shartnomasi", "ish haqi"];

fn workbooks() -> Vec<(String, Session)> {
    let mut out = Vec::new();
    for name in ["01-styles.xlsx", "02-table-rich.xlsx"] {
        let bytes = std::fs::read(format!("{FIXTURES}/{name}")).unwrap();
        let (wb, _) = io::open_bytes(name, &bytes).unwrap();
        let mut s = Session::new();
        s.add_document(DocState::open(wb, None, name.into(), false));
        out.push((name.to_string(), s));
    }
    let mut s = Session::new();
    s.new_workbook();
    let values: Vec<Vec<Value>> =
        (0..2000).map(|i| (0..5).map(|c| if c % 5 == 4 { json!(i as f64) } else { json!(WORDS[(i + c) % WORDS.len()]) }).collect()).collect();
    s.execute("range.setValues", json!({"range": "A1", "values": values})).unwrap();
    s.execute("home.insertSheet", json!({"name": "Ikkinchi"})).unwrap();
    s.execute("range.setValues", json!({"range": "B2", "values": [["salom dunyo", "=A1"], ["Ўзбекистон", 5]]})).unwrap();
    s.execute("sheet.activate", json!({"sheet": 0})).unwrap();
    out.push(("generated 2000x5 + 2 sheets".into(), s));
    out
}

fn select(s: &mut Session, scope: &str) {
    let range = if scope == "selection" { "A1:C3" } else { "A1" };
    s.execute("selection.set", json!({"range": range})).unwrap();
}

fn saved(s: &Session) -> Vec<u8> {
    io::save_bytes(&s.doc().unwrap().wb, "x.xlsx").unwrap()
}

#[test]
fn the_plugin_converts_exactly_like_the_builtin() {
    let _t = TURN.lock().unwrap_or_else(|e| e.into_inner());
    plugin_adapter::set_app("sheets");
    let (mut cases, mut same) = (0, 0);
    for (guest, g) in guests() {
        plugin_adapter::clear();
        plugin_adapter::add(Plugin::new(manifest::parse(MANIFEST).unwrap(), g)).unwrap();
        sheets_plugins::register();
        for (name, base) in workbooks() {
            for scope in ["auto", "selection", "workbook"] {
                let (plugin_scope, builtin_scope) = if scope == "workbook" { ("document", "workbook") } else { (scope, scope) };
                let (mut a, mut b) = (Session::new(), Session::new());
                a.add_document(base.doc().unwrap().clone());
                b.add_document(base.doc().unwrap().clone());
                select(&mut a, scope);
                select(&mut b, scope);
                let before = b.doc().unwrap().wb.clone();
                for (builtin, plugin) in
                    [("tools.translitCyrl", "plugin.uz-translit.toCyrillic"), ("tools.translitLatn", "plugin.uz-translit.toLatin"), ("tools.translitAuto", "plugin.uz-translit.auto")]
                {
                    cases += 1;
                    let ra = a.execute(builtin, json!({"scope": builtin_scope})).unwrap();
                    let depth = b.doc().unwrap().undo.len();
                    let rb = b.execute(plugin, json!({"scope": plugin_scope})).unwrap();
                    let (wa, wb) = (&a.doc().unwrap().wb, &b.doc().unwrap().wb);
                    assert!(**wa == **wb, "{guest} {name} {scope} {plugin}: the workbooks differ");
                    assert_eq!(saved(&a), saved(&b), "{guest} {name} {scope} {plugin}: the saved xlsx differs");
                    assert_eq!(a.doc().unwrap().selection, b.doc().unwrap().selection);
                    assert_eq!(rb["paragraphs"], ra["cells"], "{guest} {name} {scope} {plugin}: cells written");
                    if ra["cells"].as_u64() > Some(0) {
                        assert_eq!(b.doc().unwrap().undo.len(), depth + 1, "one undo step");
                    }
                    same += 1;
                }
                if name.starts_with("generated") {
                    assert!(*a.doc().unwrap().wb != *before, "{name} {scope}: something converted");
                }
                // undo walks back to the opened workbook, one step per command that changed something
                while b.doc().unwrap().undo.len() > base.doc().unwrap().undo.len() {
                    b.execute("edit.undo", json!({})).unwrap();
                }
                assert!(*b.doc().unwrap().wb == *before, "{guest} {name} {scope}: undo restores the workbook");
            }
        }
    }
    eprintln!("sheets plugin equivalence: {same} of {cases} cases identical");
    plugin_adapter::clear();
}
