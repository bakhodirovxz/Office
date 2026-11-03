//! Phase 7 WP7 (PLAN section 7), the done-check on the native host: the real `plugins/template-fields` module in wasmtime inserts a
//! content control (and a bookmark), highlights the caret's field, checks the document, and the saved docx carries `w:sdt` with
//! `w:tag` = the name and round-trips through open/save. The module comes from `cargo xt plugin build template-fields`;
//! `TEMPLATE_FIELDS_PLUGIN_WASM` names the file (ci.sh sets it). The desktop app run waits for the native guest of p7-native.
use std::io::Read;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use docs_plugins::{Guest, GuestSession, Plugin, exec};
use plugin_api::action::Widget;
use plugin_api::manifest::Event;
use plugin_api::wire::Message;
use serde_json::json;
use wordcraft_doc::Document;
use wordcraft_engine::{Session, io};

const MANIFEST: &str = include_str!("../../../../../plugins/template-fields/plugin.toml");

struct Wasm(plugin_host::Plugin);
struct Sess(plugin_host::Session);

impl Guest for Wasm {
    fn start(&self) -> Result<Box<dyn GuestSession>, String> {
        self.0
            .start()
            .map(|s| Box::new(Sess(s)) as Box<dyn GuestSession>)
            .map_err(|e| e.to_string())
    }
}

impl GuestSession for Sess {
    fn round(&mut self, call: &Message) -> Result<Message, String> {
        self.0.round(call).map_err(|e| e.to_string())
    }
}

/// The plugin registry is process-wide: the tests take turns.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn session() -> Option<(MutexGuard<'static, ()>, Session, Arc<Plugin>)> {
    let turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let Some(path) = std::env::var_os("TEMPLATE_FIELDS_PLUGIN_WASM") else {
        eprintln!(
            "TEMPLATE_FIELDS_PLUGIN_WASM is not set: run `cargo xt plugin build template-fields`; test skipped"
        );
        return None;
    };
    let wasm = std::fs::read(&path).unwrap();
    let manifest = plugin_api::manifest::parse(MANIFEST).unwrap();
    let plugin = plugin_host::Host::new()
        .unwrap()
        .load(
            &wasm,
            manifest.permissions.memory_mb,
            Duration::from_secs(5),
        )
        .unwrap();
    docs_plugins::clear();
    docs_plugins::add(Plugin::new(manifest, Arc::new(Wasm(plugin)))).unwrap();
    let mut s = Session::new(Document::from_text("Dear\nEnd"));
    docs_plugins::register(&mut s, "en");
    let p = docs_plugins::plugins()
        .into_iter()
        .find(|p| p.manifest.id == "template-fields")
        .unwrap();
    Some((turn, s, p))
}

fn texts(p: &Plugin) -> Vec<String> {
    let ui = p.ui.lock().unwrap();
    ui.panels
        .get("fields")
        .map(|w| {
            w.iter()
                .flat_map(|w| match w {
                    Widget::Text { text } | Widget::Heading { text } => vec![text.clone()],
                    Widget::List { items } => items.iter().map(|i| i.text.clone()).collect(),
                    _ => vec![],
                })
                .collect()
        })
        .unwrap_or_default()
}

fn document_xml(bytes: &[u8]) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut x = String::new();
    z.by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut x)
        .unwrap();
    x
}

#[test]
fn insert_highlight_check_and_round_trip() {
    let Some((_turn, mut s, p)) = session() else {
        return;
    };
    let c = |s: &mut Session, id: &str| {
        s.run(&format!("plugin.template-fields.{id}"), &json!({}))
            .unwrap()
    };

    // the panel lists the three default (synthetic) fields
    c(&mut s, "open");
    assert!(
        texts(&p).contains(&"Insert: Sample date".to_string()),
        "{:?}",
        texts(&p)
    );

    // filter box
    exec::ui_event(&mut s, &p, "filter", Some("mark")).unwrap();
    assert_eq!(
        texts(&p)
            .iter()
            .filter(|t| t.starts_with("Insert"))
            .collect::<Vec<_>>(),
        ["Insert: Sample mark"]
    );

    // Insert: a plain text content control tagged field_b, delete-locked, label as placeholder
    s.run("caret.docEnd", &json!({})).unwrap();
    let before = s.doc.clone();
    exec::ui_event(&mut s, &p, "ins-1", None).unwrap();
    let docx = io::save_bytes("out.docx", &s.doc).unwrap();
    let xml = document_xml(&docx);
    assert!(
        xml.contains("<w:sdt>") && xml.contains(r#"<w:tag w:val="field_b"/>"#),
        "{xml}"
    );
    assert!(
        xml.contains("Sample date") && xml.contains("<w:lock w:val=\"sdtLocked\"/>"),
        "{xml}"
    );
    // one undo step takes it back
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc, before);
    s.run("edit.redo", &json!({})).unwrap();

    // Insert a bookmark field after it
    s.run("caret.docEnd", &json!({})).unwrap();
    exec::ui_event(&mut s, &p, "ins-2", None).unwrap();
    let xml = document_xml(&io::save_bytes("out.docx", &s.doc).unwrap());
    assert!(xml.contains(r#"w:name="mark_a""#), "{xml}");

    // highlight: the selection is the bookmarked label; the panel must be open for an event to refresh it
    c(&mut s, "open");
    s.run("select.text", &json!({"text": "Sample mark"}))
        .unwrap();
    exec::emit(&mut s, Event::SelectionChanged, "en");
    let t = texts(&p);
    assert!(
        t.contains(&"Caret is in: Sample mark".to_string())
            && t.contains(&"\u{25BA} Insert: Sample mark".to_string()),
        "{t:?}"
    );
    // an event never opens a closed panel
    p.ui.lock().unwrap().panels.clear();
    exec::emit(&mut s, Event::SelectionChanged, "en");
    assert!(p.ui.lock().unwrap().panels.is_empty());

    // round trip: open the saved bytes, the control and the bookmark are still there, a second save is byte-identical
    let saved = io::save_bytes("out.docx", &s.doc).unwrap();
    let (doc, _) = io::open_bytes_with("out.docx", &saved, None).unwrap();
    let mut s2 = Session::new(doc);
    docs_plugins::register(&mut s2, "en");
    assert_eq!(
        io::save_bytes("out.docx", &s2.doc).unwrap(),
        saved,
        "second save differs"
    );

    // Check fields: field_a is missing, the other two are present, nothing is unknown
    s2.run("plugin.template-fields.check", &json!({})).unwrap();
    let t = texts(&p);
    assert!(
        t.contains(&"Missing from the document: field_a".to_string()),
        "{t:?}"
    );
    assert!(
        !t.iter().any(|x| x.starts_with("Not in the field list")),
        "{t:?}"
    );
    // a stray tag is reported as unknown
    s2.run("developer.plainText", &json!({"tag": "stray_tag"}))
        .unwrap();
    s2.run("plugin.template-fields.check", &json!({})).unwrap();
    assert!(
        texts(&p).contains(&"Not in the field list: stray_tag".to_string()),
        "{:?}",
        texts(&p)
    );
}

#[test]
fn settings_replace_the_default_list() {
    let Some((_turn, mut s, p)) = session() else {
        return;
    };
    // an administrator's list (docserver provider entry) replaces the manifest default; invalid entries are counted, not fatal
    let mut q = Plugin::new(p.manifest.clone(), p.guest.clone());
    q.settings =
        json!({"fields": [{"name": "field_z", "label": "Sample zeta"}, {"label": "no name"}]})
            .as_object()
            .unwrap()
            .clone();
    docs_plugins::clear();
    docs_plugins::add(q).unwrap();
    docs_plugins::register(&mut s, "en");
    let p = docs_plugins::plugins().into_iter().next().unwrap();
    s.run("plugin.template-fields.open", &json!({})).unwrap();
    let t = texts(&p);
    assert!(
        t.contains(&"Insert: Sample zeta".to_string())
            && !t.contains(&"Insert: Sample date".to_string()),
        "{t:?}"
    );
    assert!(t.iter().any(|x| x.starts_with("1 invalid")), "{t:?}");
}
