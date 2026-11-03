//! PLAN section 6.3 (output): the transliteration done by a plugin through the text map equals the built-in command: the same
//! document in memory, and undo restores it. The fake guest holds the plugin's whole logic: it is the same `uz-translit` crate
//! applied to the segments of the map, with `lang_converts` per segment, which is what `plugins/uz-translit` will hold.
use std::sync::{Arc, Mutex};

use docs_plugins::{Guest, GuestSession, Plugin};
use plugin_api::action::{Action, Answer, Call};
use plugin_api::textmap::{Apply, TextMap};
use plugin_api::wire::Message;
use serde_json::json;
use uz_translit::{Direction, LangPolicy, Options, Run, lang_converts};
use wordcraft_doc::props::CharProps;
use wordcraft_doc::{Document, Paragraph, Pos, StoryRef, para_block};
use wordcraft_engine::{Selection, Session};

struct Translit;
struct Sess;

impl Guest for Translit {
    fn start(&self) -> Result<Box<dyn GuestSession>, String> {
        Ok(Box::new(Sess))
    }
}

impl GuestSession for Sess {
    fn round(&mut self, m: &Message) -> Result<Message, String> {
        let call: Call = m.header_as().map_err(|e| e.to_string())?;
        let map = TextMap::decode(&m.blobs[call.reads["textmap"] as usize]).map_err(|e| e.to_string())?;
        let policy = if call.params.get("skipLangs").and_then(|v| v.as_bool()) == Some(true) { LangPolicy::SkipRuEn } else { LangPolicy::ConvertRuEn };
        let dir = if call.command.as_deref() == Some("toLatin") { Direction::ToLatin } else { Direction::ToCyrillic };
        let mut ap = Apply::default();
        for p in &map.paragraphs {
            let runs: Vec<Run> =
                p.segments.iter().map(|g| Run { text: &g.text, convert: g.editable() && lang_converts(Some(map.langs[g.lang as usize].as_str()), policy) }).collect();
            let out = uz_translit::convert_runs(&runs, dir, &Options::default());
            ap.paragraphs.push(out.into_iter().map(|c| match c {
                std::borrow::Cow::Owned(o) => Some(o),
                std::borrow::Cow::Borrowed(_) => None,
            }).collect());
        }
        let a = Answer { actions: vec![Action::TextmapApply { blob: 0 }], next: None };
        Message::new(&a, vec![ap.encode().map_err(|e| e.to_string())?]).map_err(|e| e.to_string())
    }
}

const MANIFEST: &str = r#"
id = "uz-translit"
version = "1.0.0"
api = 1
apps = ["docs"]
publisher = "Test"
name.en = "Transliteration (plugin)"

[permissions]
document = "write"

[[command]]
id = "toCyrillic"
label.en = "Latin to Cyrillic"
reads = ["textmap"]

[[command]]
id = "toLatin"
label.en = "Cyrillic to Latin"
reads = ["textmap"]
"#;

fn props(f: impl Fn(&mut CharProps)) -> CharProps {
    let mut c = CharProps { lang: Some("uz-Latn".into()), ..Default::default() };
    f(&mut c);
    c
}

fn session() -> Session {
    let mut d = Document::new();
    let pieces: Vec<Vec<(&str, CharProps)>> = vec![
        vec![("Shahar ", props(|_| {})), ("bola", props(|c| c.bold = Some(true)))],
        vec![("O'zbek ", props(|c| c.italic = Some(true))), ("tili", props(|_| {}))],
        vec![("Privet mir ", props(|c| c.lang = Some("ru-RU".into()))), ("choy", props(|_| {}))],
        // a digraph split across runs
        vec![("s", props(|c| c.bold = Some(true))), ("hahar", props(|_| {}))],
    ];
    d.body = pieces
        .into_iter()
        .map(|ps| {
            let mut p = Paragraph::new();
            for (t, c) in ps {
                p.insert_text(p.len(), t, &c).unwrap();
            }
            para_block(p)
        })
        .collect();
    d.ensure_nonempty();
    let mut s = Session::new(d);
    docs_translit::install(&mut s);
    s.sel = Selection::caret(Pos::body(0, 0));
    s
}

static TURN: Mutex<()> = Mutex::new(());

#[test]
fn a_plugin_converts_exactly_like_the_builtin() {
    let _g = TURN.lock().unwrap_or_else(|e| e.into_inner());
    docs_plugins::clear();
    docs_plugins::add(Plugin::new(plugin_api::manifest::parse(MANIFEST).unwrap(), Arc::new(Translit))).unwrap();
    for (builtin, plugin, params) in [
        ("tools.translitCyrl", "plugin.uz-translit.toCyrillic", json!({})),
        ("tools.translitCyrl", "plugin.uz-translit.toCyrillic", json!({"skipLangs": true})),
        ("tools.translitCyrl", "plugin.uz-translit.toCyrillic", json!({"scope": "selection"})),
    ] {
        for tracked in [false, true] {
            let (mut a, mut b) = (session(), session());
            docs_plugins::register(&mut b, "en");
            for s in [&mut a, &mut b] {
                s.doc.settings.track_changes = tracked;
                if params.get("scope").is_some() {
                    s.sel = Selection { anchor: Pos::body(0, 2), focus: Pos::body(1, 5) };
                }
            }
            let before = b.doc.clone();
            a.run(builtin, &params).unwrap();
            b.run(plugin, &params).unwrap();
            assert_eq!(a.doc, b.doc, "{plugin} {params} tracked={tracked}: the plugin's document equals the built-in's");
            assert_ne!(b.doc, before);
            assert_eq!(a.sel, b.sel, "the selection is mapped the same way");
            b.run("edit.undo", &json!({})).unwrap();
            assert_eq!(b.doc, before, "one undo restores the document");
        }
    }
    // and back
    let (mut a, mut b) = (session(), session());
    docs_plugins::register(&mut b, "en");
    a.run("tools.translitCyrl", &json!({})).unwrap();
    b.run("plugin.uz-translit.toCyrillic", &json!({})).unwrap();
    a.run("tools.translitLatn", &json!({})).unwrap();
    b.run("plugin.uz-translit.toLatin", &json!({})).unwrap();
    assert_eq!(a.doc, b.doc);
    let _ = StoryRef::Body;
    docs_plugins::clear();
}
