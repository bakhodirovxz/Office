//! The Slides side with the real engine and a fake in-process guest: the slot commands, the undo step (natively the slot's own
//! `Session::execute`, on the web the folded snapshot), the restore, the text map's pieces and flags, the content revision, the input
//! lock, and the malicious-plugin rows that reach the engine. The executor's own rules are tested in `plugin-adapter`.
use std::sync::{Arc, Mutex, MutexGuard};

use deckcraft_engine::office_hook::CommandHook;
use deckcraft_engine::{Session, TextSel};
use deckcraft_engine::model::ShapeId;
use plugin_adapter::plugin_api::action::{Action, Answer, Call, Next};
use plugin_adapter::plugin_api::manifest;
use plugin_adapter::plugin_api::textmap::{Apply, EDITABLE, OUTSIDE_SELECTION, TextMap};
use plugin_adapter::plugin_api::wire::Message;
use plugin_adapter::{Editor, Guest, GuestSession, Plugin, bridge};
use serde_json::{Value, json};

use crate::{Ed, Lock, register};

static TURN: Mutex<()> = Mutex::new(());

fn turn() -> MutexGuard<'static, ()> {
    let g = TURN.lock().unwrap_or_else(|e| e.into_inner());
    plugin_adapter::set_app("slides");
    plugin_adapter::clear();
    bridge::reset();
    g
}

const MANIFEST: &str = r#"
id = "slide-test"
version = "1.0.0"
api = 1
apps = ["slides"]
publisher = "Test"
name.en = "Slide test"

[permissions]
document = "write"

[[command]]
id = "go"
label.en = "Go"
reads = ["textmap"]
"#;

const GO: &str = "plugin.slide-test.go";

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

fn answer(actions: Vec<Action>, next: Option<Next>, blobs: Vec<Vec<u8>>) -> Result<Message, String> {
    Message::new(&Answer { actions, next }, blobs).map_err(|e| e.to_string())
}

fn add(f: impl FnMut(&Call, &[Vec<u8>]) -> Result<Message, String> + Send + 'static) {
    plugin_adapter::add(Plugin::new(manifest::parse(MANIFEST).unwrap(), Arc::new(Fake(Arc::new(Mutex::new(Box::new(f))))))).unwrap();
    register();
}

fn session(texts: &[&str]) -> (Session, Vec<ShapeId>) {
    let mut s = Session::with_new();
    let ids = texts
        .iter()
        .map(|t| {
            let r = s.execute("shape.insert", &json!({"preset": "rect", "rect": [80.0, 80.0, 420.0, 160.0], "text": t})).unwrap();
            ShapeId(r["id"].as_u64().unwrap() as _)
        })
        .collect();
    s.select(|_, sel| {
        sel.shapes.clear();
        sel.text = None;
    })
    .unwrap();
    (s, ids)
}

fn text_of(s: &Session, id: ShapeId) -> String {
    deckcraft_engine::model::find_shape(&s.doc().unwrap().doc.slides[0].shapes, id).unwrap().text.as_ref().unwrap().text()
}

fn upper(map: &TextMap) -> Apply {
    Apply { paragraphs: map.paragraphs.iter().map(|p| p.segments.iter().map(|g| (g.flags == EDITABLE).then(|| g.text.to_uppercase())).collect()).collect() }
}

fn apply_upper() -> impl FnMut(&Call, &[Vec<u8>]) -> Result<Message, String> + Send + 'static {
    |c, blobs| {
        let map = TextMap::decode(&blobs[c.reads["textmap"] as usize]).unwrap();
        answer(vec![Action::TextmapApply { blob: 0 }], None, vec![upper(&map).encode().unwrap()])
    }
}

#[test]
fn a_slot_command_is_one_undo_step_and_undo_restores() {
    let _t = turn();
    add(apply_upper());
    let (mut s, ids) = session(&["salom", "dunyo"]);
    let (doc, depth) = (s.doc().unwrap().doc.clone(), s.doc().unwrap().history.undo.len());
    s.execute(GO, &json!({})).unwrap();
    assert_eq!((text_of(&s, ids[0]), text_of(&s, ids[1])), ("SALOM".into(), "DUNYO".into()));
    assert_eq!(s.doc().unwrap().history.undo.len(), depth + 1, "two shapes, one undo step");
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(*s.doc().unwrap().doc, *doc);
}

#[test]
fn a_failing_plugin_leaves_the_presentation_and_the_history_as_they_were() {
    let _t = turn();
    add(|c, blobs| {
        if c.results.is_empty() {
            let map = TextMap::decode(&blobs[0]).unwrap();
            let mut a = vec![Action::TextmapApply { blob: 0 }];
            a.push(Action::Run { id: "format.bold".into(), params: json!({}) });
            answer(a, Some(Next { reads: vec![] }), vec![upper(&map).encode().unwrap()])
        } else {
            answer(vec![Action::Run { id: "text.no-such".into(), params: json!({}) }], None, vec![])
        }
    });
    let (mut s, ids) = session(&["salom"]);
    let (doc, depth) = (s.doc().unwrap().doc.clone(), s.doc().unwrap().history.undo.len());
    assert!(s.execute(GO, &json!({})).is_err());
    assert_eq!(*s.doc().unwrap().doc, *doc);
    assert_eq!(text_of(&s, ids[0]), "salom");
    assert_eq!(s.doc().unwrap().history.undo.len(), depth);
}

#[test]
fn the_text_map_cuts_at_the_selected_range_and_writes_through_the_builtin() {
    let _t = turn();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen2 = seen.clone();
    add(move |c, blobs| {
        let map = TextMap::decode(&blobs[c.reads["textmap"] as usize]).unwrap();
        *seen2.lock().unwrap() = map.paragraphs[0].segments.iter().map(|g| (g.flags, g.text.clone())).collect::<Vec<_>>();
        answer(vec![Action::TextmapApply { blob: 0 }], None, vec![upper(&map).encode().unwrap()])
    });
    let (mut s, ids) = session(&["salom dunyo yangi"]);
    s.select(|_, sel| {
        sel.shapes = vec![ids[0]];
        sel.text = Some(TextSel { shape: ids[0], anchor: (0, 6), caret: (0, 11), ..Default::default() });
    })
    .unwrap();
    s.execute(GO, &json!({})).unwrap();
    assert_eq!(
        *seen.lock().unwrap(),
        [(OUTSIDE_SELECTION, "salom ".to_string()), (EDITABLE, "dunyo".to_string()), (OUTSIDE_SELECTION, " yangi".to_string())]
    );
    assert_eq!(text_of(&s, ids[0]), "salom DUNYO yangi");
    let t = s.doc().unwrap().selection.text.clone().unwrap();
    assert_eq!((t.anchor, t.caret), ((0, 6), (0, 11)), "the selection stays on the same text");
}

#[test]
fn the_content_revision_ignores_a_selection_change_and_moves_with_an_edit() {
    let _t = turn();
    let (mut s, ids) = session(&["salom"]);
    let r0 = Ed(&mut s).rev();
    s.select(|_, sel| sel.shapes = vec![ids[0]]).unwrap();
    assert_eq!(Ed(&mut s).rev(), r0, "a selection change is not a content change");
    s.execute("format.bold", &json!({})).unwrap();
    assert_ne!(Ed(&mut s).rev(), r0);
}

#[test]
fn m5_refused_commands_and_hyperlinks_leave_the_presentation_alone() {
    let _t = turn();
    let path = std::env::temp_dir().join(format!("slides-plugins-m5-{}.pptx", std::process::id()));
    let p = path.to_string_lossy().to_string();
    for bad in [
        Action::Run { id: "file.saveAs".into(), params: json!({"path": p}) },
        Action::Run { id: "file.export".into(), params: json!({"path": p}) },
        Action::Run { id: "insert.picture".into(), params: json!({"path": p}) },
        Action::Run { id: "shape.action".into(), params: json!({}) },
        Action::Run { id: "text.insert".into(), params: json!({"text": "x", "props": {"link": {"action": {"url": "file:///C:/x"}}}}) },
        Action::Run { id: "text.insert".into(), params: json!({"text": "x", "props": {"hover": {"action": {"program": "calc.exe"}}}}) },
        Action::Run { id: "format.fontDialog".into(), params: json!({}) },
    ] {
        plugin_adapter::clear();
        let b = bad.clone();
        add(move |_, blobs| {
            let map = TextMap::decode(&blobs[0]).unwrap();
            answer(vec![Action::TextmapApply { blob: 0 }, b.clone()], None, vec![upper(&map).encode().unwrap()])
        });
        let (mut s, ids) = session(&["salom"]);
        let e = s.execute(GO, &json!({})).unwrap_err().to_string();
        assert!(e.contains("refused"), "{bad:?}: {e}");
        assert_eq!(text_of(&s, ids[0]), "salom");
        assert!(!path.exists(), "{bad:?} wrote a file");
    }
}

#[test]
fn m8_a_changed_object_segment_or_a_moved_text_is_refused() {
    let _t = turn();
    add(|_, blobs| {
        let map = TextMap::decode(&blobs[0]).unwrap();
        let ap = Apply { paragraphs: map.paragraphs.iter().map(|p| p.segments.iter().map(|_| Some("Z".to_string())).collect()).collect() };
        answer(vec![Action::TextmapApply { blob: 0 }], None, vec![ap.encode().unwrap()])
    });
    let (mut s, ids) = session(&["salom dunyo"]);
    s.select(|_, sel| {
        sel.shapes = vec![ids[0]];
        sel.text = Some(TextSel { shape: ids[0], anchor: (0, 0), caret: (0, 5), ..Default::default() });
    })
    .unwrap();
    assert!(s.execute(GO, &json!({})).is_err(), "a segment outside the selection was changed");
    assert_eq!(text_of(&s, ids[0]), "salom dunyo");
    // the live text no longer matches the read (direct check of the apply)
    let mut ed = Ed(&mut s);
    let (map, at) = ed.read_map("deck").unwrap();
    ed.0.edit(|doc, _| {
        let sl = Arc::make_mut(&mut doc.slides[0]);
        let sh = deckcraft_engine::model::find_shape_mut(&mut sl.shapes, ids[0]).unwrap();
        sh.text.as_mut().unwrap().paragraphs[0].runs[0].text = "moved".into();
        Ok(())
    })
    .unwrap();
    assert!(ed.apply_map(&map, &at, &upper(&map)).unwrap_err().contains("changed"));
    assert_eq!(text_of(&s, ids[0]), "moved");
}

#[test]
fn the_web_lock_refuses_changes_and_the_answer_is_one_undo_step() {
    let _t = turn();
    plugin_adapter::add(Plugin::new(manifest::parse(MANIFEST).unwrap(), Arc::new(bridge::PageGuest))).unwrap();
    register();
    let (mut s, ids) = session(&["salom"]);
    s.select(|_, sel| sel.shapes = vec![ids[0]]).unwrap();
    let depth = s.doc().unwrap().history.undo.len();
    let r = s.execute(GO, &json!({})).unwrap();
    assert!(r["pending"].is_number());
    assert_eq!(s.doc().unwrap().history.undo.len(), depth, "nothing yet");
    assert!(bridge::tick(&mut Ed(&mut s)));
    let blob = match bridge::take_out().remove(0) {
        bridge::Out::Call { reads, .. } => reads[0].1.clone(),
        _ => panic!("a call"),
    };
    for (id, refused) in [("text.insert", true), ("format.bold", true), ("edit.undo", true), ("file.save", true), (GO, true), ("slide.next", false)] {
        assert_eq!(Lock.before(id, &s).is_some(), refused, "{id}");
    }
    let map = TextMap::decode(&blob).unwrap();
    bridge::answer(
        &mut Ed(&mut s),
        bridge::Answer {
            call: json!(1),
            plugin: None,
            actions: json!([{"type": "textmap.apply", "blob": 0}, {"type": "run", "id": "format.bold", "params": {}}]),
            blobs: vec![upper(&map).encode().unwrap()],
            reads: Value::Null,
            more: false,
            abort: false,
        },
    );
    assert!(Lock.before("text.insert", &s).is_none(), "unlocked when the answer is in");
    assert_eq!(text_of(&s, ids[0]), "SALOM");
    assert_eq!(s.doc().unwrap().history.undo.len(), depth + 1, "the apply and the bold are one step");
    assert_eq!(s.doc().unwrap().history.undo.last().unwrap().label, "Go");
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(text_of(&s, ids[0]), "salom");
}

/// PLAN 5.2 for slides: every exception and read of the allowlist is a real command, no allowed command's params speak of a path,
/// file, URL, bytes or a link, and the refused families stay refused.
#[test]
fn the_allowlist_matches_the_registry() {
    use plugin_adapter::plugin_api::allow::{SLIDES, check_run_in};
    let specs = deckcraft_engine::command_specs();
    let has = |id: &str| specs.iter().any(|c| c.id == id);
    for (id, _) in SLIDES.exceptions {
        assert!(has(id), "exception {id} is not a command");
        assert!(SLIDES.prefixes.iter().any(|p| id.starts_with(p)), "exception {id} is outside the prefixes");
    }
    for id in SLIDES.reads {
        assert!(has(id), "read {id} is not a command");
    }
    let mut allowed = 0;
    for c in specs {
        let ok = check_run_in(&SLIDES, c.id, &json!({}));
        if ok.is_ok() {
            allowed += 1;
            let p = c.params.split('→').next().unwrap_or_default().to_ascii_lowercase();
            for w in ["path", "url", "file", "base64", "bytes", "href", "link", "folder"] {
                assert!(!p.contains(w), "{} is allowed but its params mention {w}: {}", c.id, c.params);
            }
        }
        if !SLIDES.reads.contains(&c.id) && ["file.", "edit.", "tools.", "view.", "office.", "plugin.", "insert.", "media.", "shape.", "slide.", "review.", "font."].iter().any(|f| c.id.starts_with(f)) {
            assert!(ok.is_err(), "{} must not be runnable by a plugin", c.id);
        }
    }
    println!("slides plugin allowlist: {allowed} of {} registry commands", specs.len());
    assert!(allowed > 20, "only {allowed} commands are allowed");
}
