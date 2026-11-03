//! Tests with a fake in-process guest (no wasm): every action, the refusals, the undo step, the restore and the contributions.
//! The plugin set is process-wide, so every test takes `turn()` and starts from an empty set.
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use plugin_api::action::{Action, Answer, Call, CallKind, Next, ReadReq, ReadWhat, Scope as ApiScope, Widget};
use plugin_api::manifest::{self, Event};
use plugin_api::textmap::{Apply, TextMap};
use plugin_api::wire::Message;
use serde_json::{Value, json};
use wordcraft_doc::props::CharProps;
use wordcraft_doc::{Document, Paragraph, Pos, StoryRef, para_block};
use wordcraft_engine::{Selection, Session};

use crate::exec::{self, Invocation};
use crate::{Fetch, Guest, GuestSession, Plugin, textmap};

static TURN: Mutex<()> = Mutex::new(());

fn turn() -> MutexGuard<'static, ()> {
    let g = TURN.lock().unwrap_or_else(|e| e.into_inner());
    crate::clear();
    exec::reset_events();
    g
}

const MANIFEST: &str = r#"
id = "test-plugin"
version = "1.0.0"
api = 1
apps = ["docs"]
events = ["selectionChanged"]
publisher = "Test"
name.en = "Test plugin"

[permissions]
document = "write"
network = ["https://api.example.org"]

[[command]]
id = "go"
label.en = "Go"
label.ru = "Вперёд"
reads = ["textmap"]

[[command]]
id = "other"
label.en = "Other"
label.uz-Latn = "Boshqa"

[[toolbar]]
command = "go"
group.en = "Mine"
group.ru = "Мои"

[[menu]]
command = "other"
place = "context.text"

[[panel]]
id = "side"
title.en = "Side"
"#;

type Handler = dyn FnMut(&Call, &[Vec<u8>]) -> Result<Message, String> + Send;

struct Fake(Arc<Mutex<Box<Handler>>>);
struct FakeSession(Arc<Mutex<Box<Handler>>>);

impl Guest for Fake {
    fn start(&self) -> Result<Box<dyn GuestSession>, String> {
        Ok(Box::new(FakeSession(self.0.clone())))
    }
}

impl GuestSession for FakeSession {
    fn round(&mut self, m: &Message) -> Result<Message, String> {
        let call: Call = m.header_as().map_err(|e| e.to_string())?;
        (self.0.lock().unwrap_or_else(|e| e.into_inner()))(&call, &m.blobs)
    }
}

fn answer(actions: Vec<Action>, next: Option<Vec<ReadReq>>, blobs: Vec<Vec<u8>>) -> Result<Message, String> {
    let a = Answer { actions, next: next.map(|reads| Next { reads }) };
    Message::new(&a, blobs).map_err(|e| e.to_string())
}

fn plugin_with(toml: &str, f: impl FnMut(&Call, &[Vec<u8>]) -> Result<Message, String> + Send + 'static) -> Plugin {
    let m = manifest::parse(toml).expect("manifest");
    let mut p = Plugin::new(m, Arc::new(Fake(Arc::new(Mutex::new(Box::new(f))))));
    p.origins = vec!["https://api.example.org".into()];
    p
}

fn plugin(f: impl FnMut(&Call, &[Vec<u8>]) -> Result<Message, String> + Send + 'static) -> Plugin {
    plugin_with(MANIFEST, f)
}

fn para(t: &str) -> Paragraph {
    let mut p = Paragraph::new();
    p.insert_text(0, t, &CharProps::default()).unwrap();
    p
}

fn session(texts: &[&str]) -> Session {
    let mut d = Document::new();
    d.body = texts.iter().map(|t| para_block(para(t))).collect();
    d.ensure_nonempty();
    let mut s = Session::new(d);
    s.sel = Selection::caret(Pos::body(0, 0));
    s
}

fn text(s: &Session, i: usize) -> String {
    s.doc.para(StoryRef::Body, &wordcraft_doc::Path::top(i)).unwrap().text.clone()
}

fn go(s: &mut Session) -> wordcraft_engine::CmdResult {
    s.run("plugin.test-plugin.go", &json!({}))
}

/// A command whose single round runs `actions`.
fn once(actions: Vec<Action>) -> Plugin {
    plugin(move |_, _| answer(actions.clone(), None, vec![]))
}

fn run_action(id: &str, params: Value) -> Action {
    Action::Run { id: id.into(), params }
}

fn insert(t: &str) -> Action {
    run_action("text.insert", json!({"text": t}))
}

fn setup(p: Plugin, texts: &[&str]) -> Session {
    crate::add(p).unwrap();
    let mut s = session(texts);
    crate::register(&mut s, "en");
    s
}

#[test]
fn slot_commands_are_registered_labelled_and_follow_enable_and_language() {
    let _t = turn();
    let mut s = setup(once(vec![]), &["a"]);
    let c = s.registry.get("plugin.test-plugin.go").expect("registered");
    assert_eq!(c.label, "Go");
    assert!(c.mutates, "document = write");
    assert!(s.registry.describe().to_string().contains("plugin.test-plugin.other"), "the palette lists registry commands");
    crate::register(&mut s, "ru");
    assert_eq!(s.registry.get("plugin.test-plugin.go").unwrap().label, "Вперёд");
    crate::register(&mut s, "uz-latn");
    assert_eq!(s.registry.get("plugin.test-plugin.other").unwrap().label, "Boshqa", "language tags match case-insensitively");
    assert_eq!(s.registry.get("plugin.test-plugin.go").unwrap().label, "Go", "a missing language falls back to English");
    crate::set_enabled("test-plugin", false);
    crate::register(&mut s, "en");
    assert!(s.registry.get("plugin.test-plugin.go").is_none());
    assert!(go(&mut s).is_err());
    crate::set_enabled("test-plugin", true);
    crate::register(&mut s, "en");
    assert!(s.registry.get("plugin.test-plugin.go").is_some());
}

#[test]
fn frame_registers_on_load_and_on_language_change() {
    let _t = turn();
    let mut s = session(&["a"]);
    crate::frame(&mut s, "en");
    assert!(s.registry.get("plugin.test-plugin.go").is_none());
    crate::add(once(vec![])).unwrap();
    crate::frame(&mut s, "en");
    assert_eq!(s.registry.get("plugin.test-plugin.go").unwrap().label, "Go");
    crate::frame(&mut s, "ru");
    assert_eq!(s.registry.get("plugin.test-plugin.go").unwrap().label, "Вперёд");
    crate::remove("test-plugin");
    crate::frame(&mut s, "ru");
    assert!(s.registry.get("plugin.test-plugin.go").is_none());
}

#[test]
fn sixty_four_slots_and_an_overflow_list() {
    let _t = turn();
    let mut toml = String::from("id = \"big\"\nversion = \"1.0.0\"\napi = 1\napps = [\"docs\"]\npublisher = \"T\"\nname.en = \"Big\"\n[permissions]\ndocument = \"read\"\n");
    // Five plugins of 16 commands: 80 commands, 64 slots.
    for i in 0..5 {
        let mut t = toml.replace("big", &format!("big-{i}"));
        for c in 0..16 {
            t.push_str(&format!("[[command]]\nid = \"c{c}\"\nlabel.en = \"C{c}\"\n"));
        }
        crate::add(plugin_with(&t, |_, _| answer(vec![], None, vec![]))).unwrap();
    }
    toml.clear();
    let mut s = session(&["a"]);
    crate::register(&mut s, "en");
    assert_eq!(s.registry.all().iter().filter(|c| c.id.starts_with("plugin.")).count(), crate::SLOTS);
    assert_eq!(crate::overflow().len(), 16);
    assert!(s.run("plugin.big-0.c0", &json!({})).is_ok());
    assert!(s.run("plugin.big-4.c0", &json!({})).is_err(), "no slot, no command");
}

#[test]
fn run_action_is_one_undo_step_and_the_result_reaches_the_next_round() {
    let _t = turn();
    let seen = Arc::new(Mutex::new(Vec::<Value>::new()));
    let seen2 = seen.clone();
    let p = plugin(move |call, _| {
        if call.kind == CallKind::Resume {
            *seen2.lock().unwrap() = call.results.clone();
            return answer(vec![], None, vec![]);
        }
        answer(vec![insert("X"), insert("Y"), run_action("document.text", json!({}))], Some(vec![]), vec![])
    });
    let mut s = setup(p, &["abc"]);
    let before = s.doc.clone();
    let undo_before = s.can_undo();
    let r = go(&mut s).unwrap();
    assert_eq!(r["rounds"], 2);
    assert!(text(&s, 0).contains("XY"));
    let results = seen.lock().unwrap().clone();
    assert_eq!(results.len(), 3, "one result per action, in order");
    assert!(results[2].to_string().contains("XY"), "a pure read returns the document text: {results:?}");
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc, before, "the whole invocation is one undo step");
    assert_eq!(s.can_undo(), undo_before);
}

#[test]
fn a_refused_command_leaves_the_document_alone() {
    let _t = turn();
    for bad in [
        run_action("file.saveAs", json!({"path": "C:/x.docx"})),
        run_action("tools.macros", json!({})),
        run_action("insert.textFromFile", json!({"path": "C:/x.txt"})),
        run_action("text.insert", json!({"text": "x", "file": "C:/x"})),
        run_action("plugin.test-plugin.other", json!({})),
    ] {
        crate::clear();
        let mut s = setup(once(vec![insert("first"), bad.clone()]), &["abc"]);
        let before = s.doc.clone();
        let depth = s.can_undo();
        let e = go(&mut s).unwrap_err().to_string();
        assert!(e.contains("refused"), "{bad:?}: {e}");
        assert_eq!(s.doc, before, "{bad:?}: nothing of the answer ran (it is checked before the first action)");
        assert_eq!(s.can_undo(), depth);
    }
}

#[test]
fn a_denied_permission_is_refused() {
    let _t = turn();
    let toml = MANIFEST.replace("document = \"write\"", "document = \"read\"");
    crate::add(plugin_with(&toml, |_, _| answer(vec![insert("x")], None, vec![]))).unwrap();
    let mut s = session(&["abc"]);
    crate::register(&mut s, "en");
    let before = s.doc.clone();
    assert!(go(&mut s).is_err());
    assert_eq!(s.doc, before);
    assert!(!s.registry.get("plugin.test-plugin.go").unwrap().mutates, "read access is not a mutating command");
}

#[test]
fn an_error_in_a_later_round_restores_everything() {
    let _t = turn();
    let p = plugin(|call, _| match call.kind {
        CallKind::Resume => answer(vec![run_action("no.such.command", json!({}))], None, vec![]),
        _ => answer(vec![insert("X")], Some(vec![]), vec![]),
    });
    let mut s = setup(p, &["abc"]);
    let before = s.doc.clone();
    let undo = s.can_undo();
    let sel = s.sel.clone();
    assert!(go(&mut s).is_err());
    assert_eq!(s.doc, before, "round 1's insert is taken back");
    assert_eq!(s.sel, sel);
    assert_eq!(s.can_undo(), undo, "no undo step is left behind");
}

#[test]
fn malformed_answers_are_refused_and_restore() {
    let _t = turn();
    let cases: Vec<Box<dyn FnMut(&Call, &[Vec<u8>]) -> Result<Message, String> + Send>> = vec![
        Box::new(|_, _| Ok(Message { header: "not json".into(), blobs: vec![] })),
        Box::new(|_, _| Ok(Message { header: r#"{"actions":[{"type":"explode"}]}"#.into(), blobs: vec![] })),
        Box::new(|_, _| Err("trap".into())),
        // a text map apply that changes the paragraph count
        Box::new(|_, blobs| {
            let map = TextMap::decode(&blobs[0]).unwrap();
            let mut ap = Apply { paragraphs: map.paragraphs.iter().map(|p| vec![None; p.segments.len()]).collect() };
            ap.paragraphs.push(vec![]);
            answer(vec![insert("X"), Action::TextmapApply { blob: 0 }], None, vec![ap.encode().unwrap()])
        }),
        // a text map apply with a control character in the new text
        Box::new(|_, blobs| {
            let map = TextMap::decode(&blobs[0]).unwrap();
            let ap = Apply { paragraphs: map.paragraphs.iter().map(|p| vec![Some("a\u{0}b".to_string()); p.segments.len()]).collect() };
            answer(vec![Action::TextmapApply { blob: 0 }], None, vec![ap.encode().unwrap()])
        }),
        // a blob that is not an Apply
        Box::new(|_, _| answer(vec![Action::TextmapApply { blob: 0 }], None, vec![vec![1, 2, 3]])),
    ];
    for f in cases {
        crate::clear();
        let mut s = setup(plugin(f), &["abc", "def"]);
        let before = s.doc.clone();
        assert!(go(&mut s).is_err());
        assert_eq!(s.doc, before);
    }
}

#[test]
fn textmap_read_and_apply_edit_through_the_builtin_apply() {
    let _t = turn();
    let p = plugin(|call, blobs| {
        assert_eq!(call.kind, CallKind::Command);
        assert_eq!(call.reads.get("textmap"), Some(&0));
        let map = TextMap::decode(&blobs[0]).unwrap();
        let ap = Apply {
            paragraphs: map
                .paragraphs
                .iter()
                .map(|p| p.segments.iter().map(|g| g.editable().then(|| g.text.to_uppercase())).collect())
                .collect(),
        };
        answer(vec![Action::TextmapApply { blob: 0 }], None, vec![ap.encode().unwrap()])
    });
    let mut s = setup(p, &["abc", "def"]);
    let before = s.doc.clone();
    let r = go(&mut s).unwrap();
    assert_eq!(r["paragraphs"], 2);
    assert_eq!((text(&s, 0), text(&s, 1)), ("ABC".into(), "DEF".into()));
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc, before, "one undo step");
}

#[test]
fn the_text_map_marks_what_may_not_change_and_a_selection_leaves_the_rest_out() {
    let _t = turn();
    let mut s = session(&["abc def", "ghi"]);
    s.sel = Selection { anchor: Pos::body(0, 4), focus: Pos::body(0, 7) };
    let map = exec::read_map(&mut s, textmap::Scope::Auto, true).unwrap();
    assert_eq!(map.paragraphs.len(), 1, "only the paragraph with the selection");
    let segs = &map.paragraphs[0].segments;
    assert_eq!(segs.iter().map(|g| (g.editable(), g.text.as_str())).collect::<Vec<_>>(), [(false, "abc "), (true, "def")]);
    assert!(segs[0].flags & plugin_api::textmap::OUTSIDE_SELECTION != 0);
    let whole = exec::read_map(&mut s, textmap::Scope::Document, true).unwrap();
    assert_eq!(whole.paragraphs.len(), 2);
    assert!(whole.paragraphs.iter().all(|p| p.segments.iter().all(|g| g.editable())));
}

#[test]
fn a_stale_revision_is_refused() {
    let _t = turn();
    // The caller named a revision the document has moved past.
    crate::add(once(vec![insert("x")])).unwrap();
    let mut s = session(&["abc"]);
    let p = crate::plugins().remove(0);
    let before = s.doc.clone();
    let stale = s.rev();
    s.run("text.insert", &json!({"text": "!"})).unwrap();
    let after_edit = s.doc.clone();
    let inv = Invocation {
        plugin: p.clone(),
        kind: CallKind::Command,
        command: Some("go".into()),
        widget: None,
        event: None,
        params: Default::default(),
        lang: "en".into(),
        reads: vec![],
        expect_rev: Some(stale),
        in_command: false,
        label: "x".into(),
    };
    let e = exec::invoke(&mut s, inv).unwrap_err().to_string();
    assert!(e.contains("stale"), "{e}");
    assert_eq!(s.doc, after_edit, "nothing ran");
    assert_ne!(s.doc, before);

    // A text map applied after a mutation of the same invocation is stale as well.
    crate::clear();
    let p = plugin(|call, blobs| {
        if call.kind == CallKind::Resume {
            return answer(vec![], None, vec![]);
        }
        let map = TextMap::decode(&blobs[0]).unwrap();
        let ap = Apply { paragraphs: map.paragraphs.iter().map(|p| p.segments.iter().map(|g| Some(g.text.to_uppercase())).collect()).collect() };
        answer(vec![insert("!"), Action::TextmapApply { blob: 0 }], None, vec![ap.encode().unwrap()])
    });
    let mut s = setup(p, &["abc"]);
    let before = s.doc.clone();
    let e = go(&mut s).unwrap_err().to_string();
    assert!(e.contains("stale"), "{e}");
    assert_eq!(s.doc, before);
}

#[test]
fn read_actions_return_blobs_for_the_next_round() {
    let _t = turn();
    let got = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
    let got2 = got.clone();
    let p = plugin(move |call, blobs| {
        if call.kind == CallKind::Resume {
            for (k, i) in &call.reads {
                got2.lock().unwrap().push((k.clone(), String::from_utf8_lossy(&blobs[*i as usize]).into_owned()));
            }
            return answer(vec![], None, vec![]);
        }
        let rd = |what, scope, index| Action::Read(ReadReq { what, scope, index });
        answer(vec![rd(ReadWhat::Selection, None, None), rd(ReadWhat::Paragraph, None, Some(0))], Some(vec![]), vec![])
    });
    let mut s = setup(p, &["abc", "def"]);
    s.sel = Selection { anchor: Pos::body(0, 1), focus: Pos::body(0, 3) };
    go(&mut s).unwrap();
    let got = got.lock().unwrap().clone();
    assert!(got.contains(&("selection".into(), "bc\n".into())), "{got:?}");
    assert!(got.contains(&("paragraph".into(), "abc".into())), "{got:?}");

    // A text map read by a `next` request.
    crate::clear();
    let n = Arc::new(Mutex::new(0usize));
    let n2 = n.clone();
    let p = plugin(move |call, blobs| {
        if call.kind == CallKind::Resume {
            *n2.lock().unwrap() = TextMap::decode(&blobs[call.reads["textmap"] as usize]).unwrap().paragraphs.len();
            return answer(vec![], None, vec![]);
        }
        answer(vec![], Some(vec![ReadReq { what: ReadWhat::Textmap, scope: Some(ApiScope::Document), index: None }]), vec![])
    });
    let mut s = setup(p, &["abc", "def", "ghi"]);
    go(&mut s).unwrap();
    assert_eq!(*n.lock().unwrap(), 3);
}

#[test]
fn next_reads_need_the_document_permission() {
    let _t = turn();
    let toml = MANIFEST.replace("document = \"write\"", "document = \"none\"").replace("reads = [\"textmap\"]", "reads = [\"none\"]");
    let p = plugin_with(&toml, |_, _| {
        answer(vec![], Some(vec![ReadReq { what: ReadWhat::Selection, scope: None, index: None }]), vec![])
    });
    let mut s = setup(p, &["abc"]);
    let e = go(&mut s).unwrap_err().to_string();
    assert!(e.contains("document permission"), "{e}");
}

struct FakeNet;

impl Fetch for FakeNet {
    fn fetch(&self, url: &str, method: &str, _: &BTreeMap<String, String>, body: Option<&str>) -> Result<Value, String> {
        Ok(json!({"status": 200, "url": url, "method": method, "body": body}))
    }
}

#[test]
fn fetch_needs_a_granted_origin_and_the_host_does_it() {
    let _t = turn();
    let seen = Arc::new(Mutex::new(Value::Null));
    let seen2 = seen.clone();
    let make = move |url: &'static str| {
        let seen = seen2.clone();
        move |call: &Call, _: &[Vec<u8>]| {
            if call.kind == CallKind::Resume {
                *seen.lock().unwrap() = call.results[0].clone();
                return answer(vec![], None, vec![]);
            }
            answer(
                vec![Action::Fetch { url: url.into(), method: "POST".into(), headers: Default::default(), body: Some("hi".into()) }],
                Some(vec![]),
                vec![],
            )
        }
    };
    let mut p = plugin(make("https://api.example.org/v1/x"));
    p.fetcher = Some(Arc::new(FakeNet));
    let mut s = setup(p, &["a"]);
    go(&mut s).unwrap();
    assert_eq!(seen.lock().unwrap()["status"], 200);
    assert_eq!(seen.lock().unwrap()["method"], "POST");

    // Another origin, no grant at all, and a plain http URL are refused before anything is sent.
    for (url, granted) in [("https://evil.example.com/", true), ("https://api.example.org/", false), ("http://api.example.org/", true)] {
        crate::clear();
        *seen.lock().unwrap() = Value::Null;
        let mut p = plugin(make(url));
        p.fetcher = Some(Arc::new(FakeNet));
        if !granted {
            p.origins.clear();
        }
        let mut s = setup(p, &["a"]);
        assert!(go(&mut s).is_err(), "{url}");
        assert_eq!(*seen.lock().unwrap(), Value::Null, "{url}: no request was made");
    }

    // A granted origin with a host that has no network answers with an error value, it does not abort.
    crate::clear();
    let p = plugin(make("https://api.example.org/"));
    let mut s = setup(p, &["a"]);
    go(&mut s).unwrap();
    assert!(seen.lock().unwrap()["error"].is_string());
}

fn panel_widgets() -> Vec<Widget> {
    vec![Widget::Heading { text: "Fields".into() }, Widget::Button { id: "ok".into(), label: "OK".into() }, Widget::Text { text: "<img src=x onerror=alert(1)>".into() }]
}

#[test]
fn ui_actions_set_the_panel_and_the_status_line_and_commit_only_on_success() {
    let _t = turn();
    let w = panel_widgets();
    let p = plugin(move |_, _| {
        answer(vec![Action::UiPanel { panel: "side".into(), widgets: w.clone() }, Action::UiStatus { text: "3 paragraphs converted".into() }], None, vec![])
    });
    let mut s = setup(p, &["abc"]);
    let before = s.doc.clone();
    go(&mut s).unwrap();
    assert_eq!(s.status, "3 paragraphs converted");
    assert_eq!(s.doc, before, "ui actions do not touch the document");
    let p = crate::plugins().remove(0);
    assert_eq!(p.ui.lock().unwrap().panels["side"].len(), 3);

    // An undeclared panel, an over-long label and a later error: the old panel stays, the status does not change.
    for acts in [
        vec![Action::UiPanel { panel: "nope".into(), widgets: vec![] }],
        vec![Action::UiPanel { panel: "side".into(), widgets: vec![Widget::Text { text: "x".repeat(10_000) }] }],
        vec![Action::UiStatus { text: "late".into() }, run_action("no.such.command", json!({}))],
    ] {
        crate::clear();
        let _ = crate::add(once(acts));
        let mut s = session(&["abc"]);
        crate::register(&mut s, "en");
        s.status = "old".into();
        assert!(go(&mut s).is_err());
        assert_ne!(s.status, "late", "the status line of a failed invocation is not applied");
    }
}

#[test]
fn a_panel_click_is_a_ui_call_and_one_undo_step() {
    let _t = turn();
    let calls = Arc::new(Mutex::new(Vec::<(CallKind, Option<String>, Value)>::new()));
    let calls2 = calls.clone();
    let p = plugin(move |call, _| {
        calls2.lock().unwrap().push((call.kind, call.widget.clone(), Value::Object(call.params.clone())));
        match call.kind {
            CallKind::Ui => answer(vec![insert("A"), insert("B")], None, vec![]),
            _ => answer(vec![], None, vec![]),
        }
    });
    let mut s = setup(p, &["abc"]);
    let before = s.doc.clone();
    let p = crate::plugins().remove(0);
    exec::ui_event(&mut s, &p, "ok", Some("typed")).unwrap();
    assert!(text(&s, 0).contains("AB"));
    let c = calls.lock().unwrap().clone();
    assert_eq!(c[0].0, CallKind::Ui);
    assert_eq!(c[0].1.as_deref(), Some("ok"));
    assert_eq!(c[0].2["value"], "typed");
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc, before, "the click's edits are one step");
}

#[test]
fn events_may_only_touch_the_plugins_own_ui() {
    let _t = turn();
    let p = plugin(|call, _| {
        assert_eq!(call.kind, CallKind::Event);
        assert_eq!(call.event.as_deref(), Some("selectionChanged"));
        answer(vec![insert("evil")], None, vec![])
    });
    let mut s = setup(p, &["abc"]);
    let before = s.doc.clone();
    exec::emit(&mut s, Event::SelectionChanged, "en");
    assert_eq!(s.doc, before, "an edit from an event is refused (D8)");
    let p = crate::plugins().remove(0);
    assert!(p.ui.lock().unwrap().last_error.as_deref().is_some_and(|e| e.contains("EventOnlyUi")));

    // Only subscribed events are delivered.
    crate::clear();
    let n = Arc::new(Mutex::new(0));
    let n2 = n.clone();
    let mut s = setup(plugin(move |_, _| { *n2.lock().unwrap() += 1; answer(vec![], None, vec![]) }), &["abc"]);
    exec::emit(&mut s, Event::DocumentSaved, "en");
    assert_eq!(*n.lock().unwrap(), 0);
    exec::emit(&mut s, Event::SelectionChanged, "en");
    assert_eq!(*n.lock().unwrap(), 1);
}

#[test]
fn the_frame_debounces_selection_events_and_reports_open_and_save() {
    let _t = turn();
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let seen2 = seen.clone();
    let toml = MANIFEST.replace("events = [\"selectionChanged\"]", "events = [\"selectionChanged\", \"documentOpened\", \"documentSaved\"]");
    crate::add(plugin_with(&toml, move |call, _| {
        seen2.lock().unwrap().push(call.event.clone().unwrap_or_default());
        answer(vec![], None, vec![])
    }))
    .unwrap();
    let mut s = session(&["abc"]);
    crate::frame(&mut s, "en");
    assert_eq!(*seen.lock().unwrap(), ["documentOpened"]);
    s.sel = Selection { anchor: Pos::body(0, 0), focus: Pos::body(0, 2) };
    crate::frame(&mut s, "en");
    crate::frame(&mut s, "en");
    assert_eq!(seen.lock().unwrap().len(), 1, "still inside the 200 ms window");
    std::thread::sleep(std::time::Duration::from_millis(260));
    crate::frame(&mut s, "en");
    crate::frame(&mut s, "en");
    assert_eq!(*seen.lock().unwrap(), ["documentOpened", "selectionChanged"], "fired once, after the selection was still");
    s.dirty = true;
    crate::frame(&mut s, "en");
    s.dirty = false;
    crate::frame(&mut s, "en");
    assert_eq!(seen.lock().unwrap().last().map(String::as_str), Some("documentSaved"));
}

#[test]
fn contributions_follow_the_enabled_plugins() {
    let _t = turn();
    assert!(crate::ribbon_group("en", "Plugins").is_none());
    assert!(crate::context_commands("en").is_empty());
    crate::add(once(vec![])).unwrap();
    assert_eq!(crate::ribbon_group("en", "Plugins"), Some(("Mine".into(), vec!["plugin.test-plugin.go"])));
    assert_eq!(crate::ribbon_group("ru", "Plugins").unwrap().0, "Мои");
    assert_eq!(crate::context_commands("uz-latn"), [("plugin.test-plugin.other", "Boshqa".to_string())]);
    crate::set_enabled("test-plugin", false);
    assert!(crate::ribbon_group("en", "Plugins").is_none());
    assert!(crate::context_commands("en").is_empty());
}

#[test]
fn a_plugin_for_another_app_or_api_is_not_added() {
    let _t = turn();
    let sheets = MANIFEST.replace("apps = [\"docs\"]", "apps = [\"sheets\"]");
    assert!(crate::add(plugin_with(&sheets, |_, _| answer(vec![], None, vec![]))).is_err());
    assert!(crate::plugins().is_empty());
}

#[test]
fn the_panel_and_the_context_items_draw() {
    let _t = turn();
    let w = panel_widgets();
    let p = plugin(move |_, _| answer(vec![Action::UiPanel { panel: "side".into(), widgets: w.clone() }], None, vec![]));
    let mut s = setup(p, &["abc"]);
    go(&mut s).unwrap();
    crate::frame(&mut s, "en");
    let ctx = egui::Context::default();
    let mut clicked = None;
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        crate::show_panel(&mut s, ui);
        clicked = crate::context_items(&s, ui);
    });
    out.textures_delta.clear();
    assert!(clicked.is_none(), "nothing was clicked");
    // The id of the item the menu would run exists in the registry.
    assert!(s.registry.get(crate::context_commands("en")[0].0).is_some());
}


mod web_bridge {
    //! The page-hosted plugin: pending state, input lock, transferred blobs, stale, timeout and abort (PLAN section 4.6).
    use super::*;
    use crate::bridge::{self, Answer as Page, Out, Registration};

    const JSON: &str = r#"{"id":"test-plugin","version":"1.0.0","api":1,"apps":["docs"],"events":["selectionChanged"],"publisher":"Test",
        "name":{"en":"Test plugin"},"permissions":{"document":"write"},"command":[{"id":"go","label":{"en":"Go"},"reads":["textmap"]}]}"#;

    fn page(timeout: Option<f64>) -> Session {
        bridge::reset();
        bridge::set_plugins(vec![Registration { manifest: serde_json::from_str(JSON).unwrap(), settings: None, enabled: true }], timeout);
        let mut s = session(&["abc"]);
        crate::register(&mut s, "en");
        s
    }

    fn ans(call: &Value, actions: Value, more: bool) -> Page {
        Page { call: call.clone(), plugin: None, actions, blobs: vec![], reads: Value::Null, more, abort: false }
    }

    fn ins(t: &str) -> Value {
        json!([{"type": "run", "id": "text.insert", "params": {"text": t}}])
    }

    /// Start `go`, tick, and return the call id, the command and the size of the text map blob.
    fn started(s: &mut Session) -> (Value, String, usize) {
        let id = go(s).unwrap()["pending"].clone();
        assert!(bridge::tick(s));
        match bridge::take_out().remove(0) {
            Out::Call { call, command, reads, .. } => {
                assert_eq!(call, id);
                (call, command, reads.iter().find(|(n, _)| *n == "textmap").map(|(_, b)| b.len()).unwrap_or(0))
            }
            _ => panic!("a call first"),
        }
    }

    fn error_of(out: Vec<Out>) -> Option<String> {
        match out.as_slice() {
            [Out::Results { error, .. }] => error.clone(),
            _ => panic!("one result expected"),
        }
    }

    #[test]
    fn a_command_is_pending_locks_input_and_one_answer_finishes_it_as_one_undo_step() {
        let _t = turn();
        let mut s = page(None);
        let before = s.doc.clone();
        let (call, command, map) = started(&mut s);
        assert_eq!(command, "go");
        assert!(map > 8, "the text map is a blob, not JSON");
        assert!(bridge::locked());
        assert!(s.run("text.insert", &json!({"text": "x"})).is_err(), "input is locked while the page works");
        bridge::answer(&mut s, ans(&call, ins("Z"), false));
        assert!(!bridge::locked());
        assert_eq!(s.doc.settings.protection, None);
        assert!(text(&s, 0).contains('Z'));
        assert_eq!(error_of(bridge::take_out()), None);
        s.run("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc, before, "the whole invocation is one undo step");
    }

    fn saved_protection(s: &Session) -> Option<String> {
        let bytes = wordcraft_engine::io::save_bytes("x.docx", &s.doc).unwrap();
        wordcraft_engine::io::open_bytes("x.docx", &bytes).unwrap().settings.protection
    }

    #[test]
    fn a_save_while_a_call_is_pending_does_not_carry_the_input_lock() {
        let _t = turn();
        let mut s = page(None);
        let (call, ..) = started(&mut s);
        assert_eq!(s.doc.settings.protection.as_deref(), Some("readOnly"), "the live document is locked");
        assert_eq!(saved_protection(&s), None, "the saved file and the autosave have no bridge lock");
        assert_eq!(s.doc.settings.protection.as_deref(), Some("readOnly"), "saving leaves the live lock alone");
        bridge::answer(&mut s, ans(&call, ins("Z"), false));
        assert_eq!(saved_protection(&s), None);
    }

    #[test]
    fn a_save_while_a_call_is_pending_keeps_the_protection_the_user_had() {
        let _t = turn();
        let mut s = page(None);
        s.doc.settings.protection = Some("trackedChanges".into());
        let _ = started(&mut s);
        assert_eq!(saved_protection(&s).as_deref(), Some("trackedChanges"));
        bridge::reset();
        assert_eq!(saved_protection(&s).as_deref(), Some("trackedChanges"));
    }

    #[test]
    fn a_second_round_keeps_the_lock_and_requested_reads_come_back_as_buffers() {
        let _t = turn();
        let mut s = page(None);
        let (call, ..) = started(&mut s);
        let mut a = ans(&call, ins("1"), true);
        a.reads = json!([{"what": "textmap"}, {"what": "selection"}]);
        bridge::answer(&mut s, a);
        assert!(bridge::locked(), "locked again between rounds");
        match bridge::take_out().remove(0) {
            Out::Results { results, reads, error: None, .. } => {
                assert_eq!(results.len(), 1);
                assert_eq!(reads.len(), 2);
                assert!(reads[0].len() > 8);
            }
            _ => panic!("results"),
        }
        bridge::answer(&mut s, ans(&call, ins("2"), false));
        assert!(!bridge::locked());
        assert!(text(&s, 0).contains('1') && text(&s, 0).contains('2'));
    }

    #[test]
    fn a_document_that_moved_under_the_plugin_is_stale_and_restored() {
        let _t = turn();
        let mut s = page(None);
        let before = s.doc.clone();
        let (call, ..) = started(&mut s);
        s.doc.settings.protection = None; // a lock that did not hold
        s.run("text.insert", &json!({"text": "!"})).unwrap();
        bridge::answer(&mut s, ans(&call, ins("Z"), false));
        assert!(error_of(bridge::take_out()).is_some_and(|e| e.contains("stale")));
        assert_eq!(s.doc, before, "the snapshot of the start is restored");
        assert!(!bridge::locked());
        assert!(s.status.contains("Plugin stopped"));
    }

    #[test]
    fn no_answer_in_time_restores_and_releases_the_lock_and_a_late_answer_gets_an_error() {
        let _t = turn();
        let mut s = page(Some(10.0));
        let before = s.doc.clone();
        let (call, ..) = started(&mut s);
        std::thread::sleep(std::time::Duration::from_millis(40));
        assert!(!bridge::tick(&mut s));
        assert!(!bridge::locked());
        assert_eq!(s.doc.settings.protection, None);
        assert_eq!(s.doc, before);
        assert!(s.status.contains("no answer"));
        assert!(s.run("text.insert", &json!({"text": "x"})).is_ok(), "typing works again");
        bridge::answer(&mut s, ans(&call, ins("Z"), false));
        assert!(error_of(bridge::take_out()).is_some_and(|e| e.contains("ended")));
        assert!(!text(&s, 0).contains('Z'));
    }

    #[test]
    fn an_abort_and_a_denied_action_restore() {
        let _t = turn();
        let mut s = page(None);
        let before = s.doc.clone();
        let (call, ..) = started(&mut s);
        bridge::answer(&mut s, ans(&call, ins("A"), true));
        let _ = bridge::take_out();
        assert_ne!(s.doc, before);
        let mut ab = ans(&call, json!([]), false);
        ab.abort = true;
        bridge::answer(&mut s, ab);
        assert_eq!(s.doc, before);
        assert!(!bridge::locked() && bridge::take_out().is_empty(), "an abort is not answered");
        let (call, ..) = started(&mut s);
        let two = json!([{"type": "run", "id": "text.insert", "params": {"text": "A"}}, {"type": "run", "id": "file.save", "params": {}}]);
        bridge::answer(&mut s, ans(&call, two, false));
        assert!(error_of(bridge::take_out()).is_some_and(|e| e.contains("refused")));
        assert_eq!(s.doc, before);
        assert!(!bridge::locked());
    }

    #[test]
    fn events_are_queued_once_for_the_page_and_do_not_lock() {
        let _t = turn();
        let mut s = page(None);
        exec::emit(&mut s, Event::SelectionChanged, "en");
        exec::emit(&mut s, Event::SelectionChanged, "en");
        assert!(!bridge::locked());
        assert!(matches!(bridge::take_out().as_slice(), [Out::Event { event }] if event == "selectionChanged"));
    }

    #[test]
    fn a_panel_click_round_starts_at_the_first_forwarded_answer() {
        let _t = turn();
        let mut s = page(None);
        let before = s.doc.clone();
        let mut a = ans(&json!("h1"), ins("U"), false);
        a.plugin = Some("test-plugin".into());
        bridge::answer(&mut s, a);
        assert!(text(&s, 0).contains('U'));
        assert_eq!(error_of(bridge::take_out()), None);
        assert!(!bridge::locked());
        s.run("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc, before, "one undo step");
        // a click whose later round fails puts the document back
        let mut a = ans(&json!("h2"), ins("V"), true);
        a.plugin = Some("test-plugin".into());
        bridge::answer(&mut s, a);
        let _ = bridge::take_out();
        assert!(bridge::locked());
        bridge::answer(&mut s, ans(&json!("h2"), json!([{"type": "run", "id": "file.save", "params": {}}]), false));
        assert!(error_of(bridge::take_out()).is_some());
        assert_eq!(s.doc, before);
        // an unknown plugin is refused
        let mut a = ans(&json!("h3"), ins("W"), false);
        a.plugin = Some("nope".into());
        bridge::answer(&mut s, a);
        assert!(error_of(bridge::take_out()).is_some());
    }

    #[test]
    fn replacing_the_document_drops_the_invocation_without_touching_the_new_one() {
        let _t = turn();
        let mut s = page(None);
        started(&mut s);
        s.set_document(session(&["new"]).doc);
        assert!(!bridge::tick(&mut s));
        assert_eq!(s.doc.settings.protection, None);
        assert_eq!(text(&s, 0), "new");
        assert!(!bridge::locked());
    }

    #[test]
    fn registering_again_with_a_shorter_list_unregisters() {
        let _t = turn();
        let mut s = page(None);
        assert!(crate::plugins().iter().any(|p| p.manifest.id == "test-plugin"));
        let (ids, errors) = bridge::set_plugins(vec![Registration { manifest: json!({"id": "x"}), settings: None, enabled: true }], None);
        assert!(ids.is_empty() && errors.len() == 1, "a bad manifest is refused: {errors:?}");
        assert!(crate::plugins().is_empty(), "the plugin that is no longer listed is gone");
        crate::frame(&mut s, "en");
        assert!(s.registry.get("plugin.test-plugin.go").is_none());
    }
}
