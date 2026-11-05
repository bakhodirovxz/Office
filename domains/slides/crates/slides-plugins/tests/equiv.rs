//! PLAN section 6.3 for Slides (Phase 7 WP13): the transliteration done by the uz-translit plugin through the text map equals the
//! built-in command: the same presentation in memory, the same selection, and undo restores it. Two guests: the plugin's own code
//! compiled natively (`uz_translit_plugin::handle`, always), and its wasm module in the native host (`UZ_TRANSLIT_PLUGIN_WASM`, built
//! by `cargo xt plugin build uz-translit`; skipped with a note when unset).
//! Matrix: the two pptx fixtures of `domains/slides/tests/translit-fixtures` and a generated deck (fields, a break, a link, language
//! tags, a table, notes, a digraph across runs, 40 slides); to Cyrillic, then back to Latin on the result, then auto; scopes auto
//! with nothing selected (the deck), `slide`, `document` (= deck), the selected shapes, and a text range inside a shape.
use std::sync::{Arc, Mutex};

use deckcraft_engine::model::text::{Hyperlink, Run, RunKind, RunProps, TextBody};
use deckcraft_engine::model::{Paragraph, ShapeId};
use deckcraft_engine::{Session, TextSel};
use plugin_adapter::plugin_api::action::{Answer, Call};
use plugin_adapter::plugin_api::manifest;
use plugin_adapter::plugin_api::wire::Message;
use plugin_adapter::{Guest, GuestSession, Plugin};
use serde_json::json;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/translit-fixtures/pptx");
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

fn text_run(t: &str, props: RunProps) -> Run {
    Run { text: t.into(), props, kind: RunKind::Text }
}

fn generated() -> Session {
    let mut s = Session::with_new();
    let words = ["Toshkent shahri", "O'zbekiston Respublikasi", "yangi ish o'rni", "Xodimlar bo'limi", "mehnat shartnomasi"];
    for i in 0..40 {
        if i > 0 {
            s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
        }
        let r = s.execute("shape.insert", &json!({"preset": "rect", "rect": [40.0, 40.0, 400.0, 120.0], "text": "x"})).unwrap();
        let id = ShapeId(r["id"].as_u64().unwrap() as _);
        let tag = |l: &str| RunProps { lang: Some(l.into()), ..Default::default() };
        let bold = RunProps { bold: Some(true), ..Default::default() };
        let link = RunProps {
            link: Some(Hyperlink {
                action: deckcraft_engine::model::text::Action::Url { url: "https://example.org/shahar".into() },
                tooltip: String::new(),
                highlight_click: false,
                sound: None,
                stop_sound: false,
                zoom_return: None,
            }),
            ..Default::default()
        };
        let w = words[i % words.len()];
        s.edit(|doc, _| {
            let sl = Arc::make_mut(&mut doc.slides[i]);
            let sh = deckcraft_engine::model::find_shape_mut(&mut sl.shapes, id).unwrap();
            sh.text.as_mut().unwrap().paragraphs = vec![
                Paragraph {
                    runs: vec![
                        text_run("s", bold.clone()),
                        text_run(&format!("hahar {w} "), Default::default()),
                        Run { text: format!("{}", i + 1), props: bold.clone(), kind: RunKind::Field { field: "slidenum".into() } },
                        Run { text: String::new(), props: Default::default(), kind: RunKind::Break },
                        text_run("salom", link.clone()),
                    ],
                    ..Default::default()
                },
                Paragraph { runs: vec![text_run("privet ", tag("ru-RU")), text_run("hallo ", tag("de-DE")), text_run("dunyo", tag("uz-Latn"))], ..Default::default() },
            ];
            sl.notes = TextBody::from_text(&format!("{w} izoh"));
            Ok(())
        })
        .unwrap();
    }
    s.execute("slide.go", &json!({"index": 0})).unwrap();
    s
}

fn decks() -> Vec<(String, Session)> {
    let mut out = Vec::new();
    for name in ["01-shapes-notes.pptx", "02-languages.pptx"] {
        let mut s = Session::new();
        s.execute("file.open", &json!({"path": format!("{FIXTURES}/{name}")})).unwrap();
        out.push((name.to_string(), s));
    }
    out.push(("generated 40 slides".into(), generated()));
    out
}

/// The first shape of the current slide that has text.
fn text_shape(s: &Session) -> Option<(ShapeId, usize)> {
    s.doc().unwrap().shapes().iter().find_map(|sh| sh.text.as_ref().filter(|t| t.text().chars().count() > 6).map(|t| (sh.id, t.text().chars().count())))
}

/// Put the selection for `case` in `s`; returns the `scope` param for the built-in and for the plugin.
fn select(s: &mut Session, case: &str) -> (&'static str, &'static str) {
    let shapes: Vec<ShapeId> = s.doc().unwrap().shapes().iter().map(|sh| sh.id).collect();
    let range = text_shape(s);
    s.select(|_, sel| {
        sel.shapes.clear();
        sel.text = None;
        match case {
            "shapes" => sel.shapes = shapes.clone(),
            "range" => {
                if let Some((id, _)) = range {
                    sel.shapes = vec![id];
                    sel.text = Some(TextSel { shape: id, anchor: (0, 2), caret: (0, 6), ..Default::default() });
                }
            }
            _ => {}
        }
    })
    .unwrap();
    match case {
        "slide" => ("slide", "slide"),
        "deck" => ("deck", "document"),
        _ => ("auto", "auto"),
    }
}

#[test]
fn the_plugin_converts_exactly_like_the_builtin() {
    let _t = TURN.lock().unwrap_or_else(|e| e.into_inner());
    plugin_adapter::set_app("slides");
    let (mut cases, mut same) = (0, 0);
    for (guest, g) in guests() {
        plugin_adapter::clear();
        plugin_adapter::add(Plugin::new(manifest::parse(MANIFEST).unwrap(), g)).unwrap();
        slides_plugins::register();
        for (name, base) in decks() {
            for case in ["auto", "slide", "deck", "shapes", "range"] {
                let (mut a, mut b) = (Session::new(), Session::new());
                a.add_document(base.doc().unwrap().clone());
                b.add_document(base.doc().unwrap().clone());
                let (builtin_scope, plugin_scope) = select(&mut a, case);
                select(&mut b, case);
                let before = b.doc().unwrap().doc.clone();
                let depth = b.doc().unwrap().history.undo.len();
                for (builtin, plugin) in
                    [("tools.translitCyrl", "plugin.uz-translit.toCyrillic"), ("tools.translitLatn", "plugin.uz-translit.toLatin"), ("tools.translitAuto", "plugin.uz-translit.auto")]
                {
                    cases += 1;
                    let ra = a.execute(builtin, &json!({"scope": builtin_scope})).unwrap();
                    let rb = b.execute(plugin, &json!({"scope": plugin_scope})).unwrap();
                    assert!(a.doc().unwrap().doc == b.doc().unwrap().doc, "{guest} {name} {case} {plugin}: the presentations differ");
                    assert_eq!(a.doc().unwrap().selection, b.doc().unwrap().selection, "{guest} {name} {case} {plugin}: selection");
                    assert_eq!(rb["paragraphs"], ra["paragraphs"], "{guest} {name} {case} {plugin}: paragraphs written");
                    same += 1;
                }
                if name.starts_with("generated") && case != "range" {
                    assert!(*a.doc().unwrap().doc != *before, "{name} {case}: something converted");
                }
                while b.doc().unwrap().history.undo.len() > depth {
                    b.execute("edit.undo", &json!({})).unwrap();
                }
                assert!(*b.doc().unwrap().doc == *before, "{guest} {name} {case}: undo restores the presentation");
            }
        }
    }
    eprintln!("slides plugin equivalence: {same} of {cases} cases identical");
    plugin_adapter::clear();
}
