//! Tests with a fake editor (a list of cells, an undo stack, a revision) and a fake in-process guest (no wasm): the slots, every
//! action, the refusals of the malicious-plugin suite (docs/phase7/malicious.md) under the Sheets allowlist, the undo step, the
//! restore, and the web bridge. The real engines are tested in `sheets-plugins` and `slides-plugins`.
//! The plugin set is process-wide, so every test takes `turn()` and starts from an empty set.
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use plugin_api::action::{Action, Answer, Call, CallKind, Next, ReadReq, ReadWhat};
use plugin_api::manifest;
use plugin_api::textmap::{Apply, EDITABLE, LOCKED_CONTROL, Paragraph, Segment, TextMap};
use plugin_api::wire::Message;
use serde_json::{Value, json};

use crate::bridge::{self, Answer as PageAnswer, Out};
use crate::{Editor, Fetch, Guest, GuestSession, Plugin, exec};

static TURN: Mutex<()> = Mutex::new(());

fn turn() -> MutexGuard<'static, ()> {
    let g = TURN.lock().unwrap_or_else(|e| e.into_inner());
    crate::set_app("sheets");
    crate::clear();
    bridge::reset();
    exec::set_invocation_ms(plugin_api::wire::INVOCATION_MS as f64);
    g
}

/// A workbook of one column: `cells[i]` is a text cell, `locked[i]` a locked cell of a protected sheet.
#[derive(Clone, Default)]
struct Book {
    cells: Vec<String>,
    locked: Vec<bool>,
    undo: Vec<Vec<String>>,
    rev: u64,
    doc: u64,
    protected: bool,
    steps: usize,
    sel: (usize, usize),
}

fn book(texts: &[&str]) -> Book {
    Book {
        cells: texts.iter().map(|t| t.to_string()).collect(),
        locked: vec![false; texts.len()],
        rev: 1,
        doc: 7,
        sel: (0, texts.len()),
        ..Default::default()
    }
}

impl Editor for Book {
    type Snap = (Vec<String>, usize, u64);
    type Map = Vec<usize>;
    fn app(&self) -> &'static str {
        "sheets"
    }
    fn doc_id(&self) -> Option<u64> {
        Some(self.doc)
    }
    fn rev(&self) -> u64 {
        self.rev
    }
    fn snapshot(&self) -> Option<Self::Snap> {
        Some((self.cells.clone(), self.undo.len(), self.rev))
    }
    fn restore(&mut self, (cells, undo, _): Self::Snap) {
        self.cells = cells;
        self.undo.truncate(undo);
        self.rev += 1;
    }
    fn commit(&mut self, (cells, undo, _): &Self::Snap, _label: &str, _in_command: bool) {
        self.undo.truncate(*undo);
        if self.cells != *cells {
            self.undo.push(cells.clone());
            self.steps += 1;
        }
    }
    fn run(&mut self, id: &str, p: &Value) -> Result<Value, String> {
        match id {
            "cell.set" => {
                let i = p["index"].as_u64().unwrap_or(0) as usize;
                let t = p["input"].as_str().unwrap_or("").to_string();
                if self.locked.get(i) == Some(&true) {
                    return Err("locked".into());
                }
                self.undo.push(self.cells.clone());
                self.cells[i] = t;
                self.rev += 1;
                Ok(Value::Null)
            }
            "sheet.read" => Ok(json!(self.cells.join("\n"))),
            "range.fill" => Err("no such range".into()),
            _ => Err(format!("unknown command {id}")),
        }
    }
    fn protected(&self) -> Option<&'static str> {
        self.protected.then_some(bridge::PROTECTED)
    }
    fn read_map(&mut self, scope: &str) -> Result<(TextMap, Vec<usize>), String> {
        let (a, b) = if scope == "selection" {
            self.sel
        } else {
            (0, self.cells.len())
        };
        let mut map = TextMap::default();
        let mut at = Vec::new();
        for i in a..b {
            let flags = if self.locked[i] {
                LOCKED_CONTROL
            } else {
                EDITABLE
            };
            map.paragraphs.push(Paragraph {
                index: i as u32,
                segments: vec![Segment {
                    flags,
                    lang: 0,
                    text: self.cells[i].clone(),
                }],
            });
            at.push(i);
        }
        Ok((map, at))
    }
    fn apply_map(&mut self, map: &TextMap, at: &Vec<usize>, ap: &Apply) -> Result<usize, String> {
        for (k, &i) in at.iter().enumerate() {
            if self.cells[i] != map.paragraphs[k].segments[0].text {
                return Err(format!("cell {i} changed since the text map was read"));
            }
        }
        let mut n = 0;
        for (k, &i) in at.iter().enumerate() {
            if let Some(t) = &ap.paragraphs[k][0] {
                self.cells[i] = t.clone();
                n += 1;
            }
        }
        self.rev += 1;
        Ok(n)
    }
    fn selection_text(&self) -> String {
        self.cells[self.sel.0..self.sel.1].join("\n")
    }
}

const MANIFEST: &str = r#"
id = "test-plugin"
version = "1.0.0"
api = 1
apps = ["sheets", "slides"]
publisher = "Test"
name.en = "Test plugin"
name.ru = "Тест"

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

fn answer(
    actions: Vec<Action>,
    next: Option<Vec<ReadReq>>,
    blobs: Vec<Vec<u8>>,
) -> Result<Message, String> {
    Message::new(
        &Answer {
            actions,
            next: next.map(|reads| Next { reads }),
        },
        blobs,
    )
    .map_err(|e| e.to_string())
}

fn plugin_with(
    toml: &str,
    f: impl FnMut(&Call, &[Vec<u8>]) -> Result<Message, String> + Send + 'static,
) -> Plugin {
    let mut p = Plugin::new(
        manifest::parse(toml).expect("manifest"),
        Arc::new(Fake(Arc::new(Mutex::new(Box::new(f))))),
    );
    p.origins = vec!["https://api.example.org".into()];
    p
}

fn plugin(f: impl FnMut(&Call, &[Vec<u8>]) -> Result<Message, String> + Send + 'static) -> Plugin {
    plugin_with(MANIFEST, f)
}

fn once(actions: Vec<Action>) -> Plugin {
    plugin(move |_, _| answer(actions.clone(), None, vec![]))
}

fn set(i: usize, t: &str) -> Action {
    Action::Run {
        id: "cell.set".into(),
        params: json!({"index": i, "input": t}),
    }
}

fn run_action(id: &str, params: Value) -> Action {
    Action::Run {
        id: id.into(),
        params,
    }
}

/// Add `p` and run its first command, the way the slot command does.
fn go(b: &mut Book) -> Result<Value, String> {
    let p = crate::plugins().into_iter().next().expect("a plugin");
    exec::run_command(b, &p, 0, &json!({}))
}

fn setup(p: Plugin, texts: &[&str]) -> Book {
    crate::add(p).unwrap();
    book(texts)
}

/// Upper-case every editable segment.
fn upper(map: &TextMap) -> Apply {
    Apply {
        paragraphs: map
            .paragraphs
            .iter()
            .map(|p| {
                p.segments
                    .iter()
                    .map(|g| (g.flags == EDITABLE).then(|| g.text.to_uppercase()))
                    .collect()
            })
            .collect(),
    }
}

#[test]
fn a_plugin_for_another_app_or_api_is_not_added() {
    let _t = turn();
    assert!(
        crate::add(plugin_with(
            &MANIFEST.replace(r#"apps = ["sheets", "slides"]"#, r#"apps = ["docs"]"#),
            |_, _| answer(vec![], None, vec![])
        ))
        .is_err()
    );
    assert!(
        manifest::parse(&MANIFEST.replace("api = 1", "api = 2")).is_err(),
        "an api this host does not run is refused at parse"
    );
    assert!(crate::add(once(vec![])).is_ok());
}

#[test]
fn slots_are_bound_once_per_id_and_follow_enable() {
    let _t = turn();
    crate::add(once(vec![])).unwrap();
    let first = crate::bind();
    let ids: Vec<&str> = crate::bind().iter().map(|x| x.1).collect();
    assert!(ids.is_empty(), "a bound id is not bound again");
    let n = first
        .iter()
        .find(|x| x.1 == "plugin.test-plugin.go")
        .map(|x| x.0)
        .or_else(|| {
            // bound by an earlier test of this process: find its slot
            (0..crate::SLOTS)
                .find(|&n| crate::slot(n).is_some_and(|(p, c)| p.manifest.commands[c].id == "go"))
        });
    let n = n.expect("go has a slot");
    assert!(crate::slot(n).is_some());
    let menu = crate::menu("ru");
    assert_eq!(menu[0].0, "Тест");
    assert!(
        menu[0]
            .1
            .iter()
            .any(|(l, id)| l == "Вперёд" && *id == "plugin.test-plugin.go")
    );
    crate::set_enabled("test-plugin", false);
    assert!(
        crate::slot(n).is_none(),
        "a disabled plugin's slot runs nothing"
    );
    assert!(crate::menu("en").is_empty());
    crate::set_enabled("test-plugin", true);
    crate::remove("test-plugin");
    assert!(
        crate::slot(n).is_none(),
        "a removed plugin's slot runs nothing"
    );
}

#[test]
fn run_actions_are_one_undo_step_and_the_result_reaches_the_next_round() {
    let _t = turn();
    let seen = Arc::new(Mutex::new(Vec::<Value>::new()));
    let seen2 = seen.clone();
    let p = plugin(move |call, _| {
        if call.kind == CallKind::Resume {
            *seen2.lock().unwrap() = call.results.clone();
            return answer(vec![], None, vec![]);
        }
        answer(
            vec![
                set(0, "X"),
                set(1, "Y"),
                run_action("sheet.read", json!({})),
            ],
            Some(vec![]),
            vec![],
        )
    });
    let mut b = setup(p, &["a", "b"]);
    let r = go(&mut b).unwrap();
    assert_eq!(r["rounds"], 2);
    assert_eq!(b.cells, ["X", "Y"]);
    let results = seen.lock().unwrap().clone();
    assert_eq!(results.len(), 3);
    assert_eq!(results[2], json!("X\nY"), "a pure read returns the cells");
    assert_eq!((b.undo.len(), b.steps), (1, 1), "two edits, one undo step");
    assert_eq!(b.undo[0], ["a", "b"]);
}

#[test]
fn m5_refused_commands_and_params_leave_the_document_alone() {
    let _t = turn();
    for bad in [
        run_action("file.saveAs", json!({"path": "C:/Users/Public/x.xlsx"})),
        run_action("file.open", json!({"path": "C:/x.xlsx"})),
        run_action("tools.translitCyrl", json!({})),
        run_action("edit.copy", json!({})),
        run_action("data.getData", json!({"path": "C:/x.csv"})),
        run_action("automate.run", json!({})),
        run_action("home.addIns", json!({})),
        run_action(
            "cell.set",
            json!({"index": 0, "input": "=WEBSERVICE(\"https://x.org\")"}),
        ),
        run_action(
            "cell.set",
            json!({"index": 0, "input": "+cmd|' /c calc'!A0"}),
        ),
        run_action("range.setValues", json!({"values": [["ok", "  =1+1"]]})),
        run_action(
            "cell.set",
            json!({"index": 0, "input": "x", "file": "C:/x"}),
        ),
        run_action("plugin.test-plugin.other", json!({})),
        run_action("office.history", json!({})),
    ] {
        crate::clear();
        let mut b = setup(once(vec![set(0, "first"), bad.clone()]), &["abc"]);
        let e = go(&mut b).unwrap_err();
        assert!(e.contains("refused"), "{bad:?}: {e}");
        assert_eq!(
            b.cells,
            ["abc"],
            "{bad:?}: nothing of the answer ran (checked before the first action)"
        );
        assert!(b.undo.is_empty());
    }
    // a number and plain text pass
    crate::clear();
    let mut b = setup(once(vec![set(0, "-5"), set(0, "a = b")]), &["abc"]);
    go(&mut b).unwrap();
    assert_eq!(b.cells, ["a = b"]);
}

#[test]
fn a_denied_permission_is_refused() {
    let _t = turn();
    let toml = MANIFEST.replace("document = \"write\"", "document = \"read\"");
    crate::add(plugin_with(&toml, |_, _| {
        answer(vec![set(0, "x")], None, vec![])
    }))
    .unwrap();
    let mut b = book(&["abc"]);
    assert!(go(&mut b).is_err());
    assert_eq!(b.cells, ["abc"]);
    let toml = MANIFEST
        .replace("document = \"write\"", "document = \"none\"")
        .replace("reads = [\"textmap\"]\n", "");
    crate::clear();
    crate::add(plugin_with(&toml, |_, _| {
        answer(
            vec![Action::Read(ReadReq {
                what: ReadWhat::Textmap,
                scope: None,
                index: None,
            })],
            None,
            vec![],
        )
    }))
    .unwrap();
    assert!(go(&mut b).unwrap_err().contains("refused"));
    assert_eq!(b.cells, ["abc"]);
}

#[test]
fn an_error_in_a_later_round_restores_everything() {
    let _t = turn();
    let p = plugin(|call, _| match call.kind {
        CallKind::Resume => answer(vec![run_action("range.fill", json!({}))], None, vec![]),
        _ => answer(vec![set(0, "X")], Some(vec![]), vec![]),
    });
    let mut b = setup(p, &["abc"]);
    assert!(go(&mut b).is_err());
    assert_eq!(b.cells, ["abc"], "round 1's edit is taken back");
    assert!(b.undo.is_empty(), "no undo step is left behind");
    assert!(crate::plugins()[0].ui.lock().unwrap().last_error.is_some());
}

#[test]
fn m8_malformed_answers_are_refused_and_restore() {
    let _t = turn();
    let cases: Vec<Box<Handler>> = vec![
        Box::new(|_, _| {
            Ok(Message {
                header: "not json".into(),
                blobs: vec![],
            })
        }),
        Box::new(|_, _| {
            Ok(Message {
                header: r#"{"actions":[{"type":"explode"}]}"#.into(),
                blobs: vec![],
            })
        }),
        Box::new(|_, _| Err("trap".into())),
        // a text map with an extra paragraph
        Box::new(|_, blobs| {
            let map = TextMap::decode(&blobs[0]).unwrap();
            let mut ap = upper(&map);
            ap.paragraphs.push(vec![]);
            answer(
                vec![set(0, "X"), Action::TextmapApply { blob: 0 }],
                None,
                vec![ap.encode().unwrap()],
            )
        }),
        // an extra segment
        Box::new(|_, blobs| {
            let map = TextMap::decode(&blobs[0]).unwrap();
            let mut ap = upper(&map);
            ap.paragraphs[0].push(Some("x".into()));
            answer(
                vec![Action::TextmapApply { blob: 0 }],
                None,
                vec![ap.encode().unwrap()],
            )
        }),
        // a changed segment that is not editable (the locked cell)
        Box::new(|_, blobs| {
            let map = TextMap::decode(&blobs[0]).unwrap();
            let ap = Apply {
                paragraphs: map
                    .paragraphs
                    .iter()
                    .map(|_| vec![Some("Z".to_string())])
                    .collect(),
            };
            answer(
                vec![Action::TextmapApply { blob: 0 }],
                None,
                vec![ap.encode().unwrap()],
            )
        }),
        // a control character in the new text
        Box::new(|_, blobs| {
            let map = TextMap::decode(&blobs[0]).unwrap();
            let ap = Apply {
                paragraphs: map
                    .paragraphs
                    .iter()
                    .map(|_| vec![Some("a\u{0}b".to_string())])
                    .collect(),
            };
            answer(
                vec![Action::TextmapApply { blob: 0 }],
                None,
                vec![ap.encode().unwrap()],
            )
        }),
        // a blob that is not an Apply, and a blob index out of range
        Box::new(|_, _| {
            answer(
                vec![Action::TextmapApply { blob: 0 }],
                None,
                vec![vec![1, 2, 3]],
            )
        }),
        Box::new(|_, _| answer(vec![Action::TextmapApply { blob: 3 }], None, vec![])),
        // apply without a read of this invocation
        Box::new(|call, blobs| {
            let map = TextMap::decode(&blobs[call.reads["textmap"] as usize]).unwrap();
            let ap = upper(&map).encode().unwrap();
            answer(
                vec![
                    Action::TextmapApply { blob: 0 },
                    Action::TextmapApply { blob: 0 },
                ],
                None,
                vec![ap],
            )
        }),
    ];
    for f in cases {
        crate::clear();
        let mut b = setup(plugin(f), &["abc", "def"]);
        b.locked[1] = true;
        assert!(go(&mut b).is_err());
        assert_eq!(b.cells, ["abc", "def"]);
        assert!(b.undo.is_empty());
    }
}

#[test]
fn the_text_map_is_read_with_the_command_and_applied_as_one_step() {
    let _t = turn();
    let p = plugin(|call, blobs| {
        assert_eq!(call.app, "sheets");
        let map = TextMap::decode(&blobs[call.reads["textmap"] as usize]).unwrap();
        assert_eq!(map.paragraphs.len(), 3);
        assert_eq!(map.paragraphs[1].segments[0].flags, LOCKED_CONTROL);
        answer(
            vec![
                Action::TextmapApply { blob: 0 },
                Action::UiStatus {
                    text: "done".into(),
                },
            ],
            None,
            vec![upper(&map).encode().unwrap()],
        )
    });
    let mut b = setup(p, &["abc", "def", "ghi"]);
    b.locked[1] = true;
    let r = go(&mut b).unwrap();
    assert_eq!(b.cells, ["ABC", "def", "GHI"]);
    assert_eq!(
        (r["paragraphs"].as_u64(), r["status"].as_str()),
        (Some(2), Some("done"))
    );
    assert_eq!(b.undo.len(), 1);
}

#[test]
fn a_stale_revision_is_refused() {
    let _t = turn();
    // the cell moved between the read and the apply (a `run` in the same answer)
    let p = plugin(|_, blobs| {
        let map = TextMap::decode(&blobs[0]).unwrap();
        answer(
            vec![set(0, "moved"), Action::TextmapApply { blob: 0 }],
            None,
            vec![upper(&map).encode().unwrap()],
        )
    });
    let mut b = setup(p, &["abc"]);
    let e = go(&mut b).unwrap_err();
    assert!(e.contains("stale"), "{e}");
    assert_eq!(b.cells, ["abc"]);
}

#[test]
fn r1_more_than_16_reads_in_a_round_are_refused_before_any_is_done() {
    let _t = turn();
    let reads: Vec<ReadReq> = (0..17)
        .map(|_| ReadReq {
            what: ReadWhat::Selection,
            scope: None,
            index: None,
        })
        .collect();
    let mut b = setup(
        plugin(move |call, _| {
            if call.kind == CallKind::Resume {
                answer(vec![], None, vec![])
            } else {
                answer(vec![set(0, "x")], Some(reads.clone()), vec![])
            }
        }),
        &["abc"],
    );
    let e = go(&mut b).unwrap_err();
    assert!(e.contains("more than 16 reads"), "{e}");
    assert_eq!(b.cells, ["abc"]);
}

#[test]
fn more_than_16_rounds_are_refused() {
    let _t = turn();
    let mut b = setup(
        plugin(|_, _| answer(vec![], Some(vec![]), vec![])),
        &["abc"],
    );
    assert!(go(&mut b).unwrap_err().contains("16 rounds"));
}

struct Net(Arc<Mutex<usize>>, u64);

impl Fetch for Net {
    fn fetch(
        &self,
        _: &str,
        _: &str,
        _: &BTreeMap<String, String>,
        _: Option<&str>,
    ) -> Result<Value, String> {
        *self.0.lock().unwrap() += 1;
        std::thread::sleep(std::time::Duration::from_millis(self.1));
        Ok(json!({"status": 200, "body": "ok"}))
    }
}

fn fetch_to(url: &str) -> Action {
    Action::Fetch {
        url: url.into(),
        method: "GET".into(),
        headers: Default::default(),
        body: None,
    }
}

#[test]
fn m3_m4_fetch_needs_a_granted_origin_and_the_host_does_it() {
    let _t = turn();
    let n = Arc::new(Mutex::new(0));
    for bad in [
        "https://example.org/x",
        "http://api.example.org/x",
        "https://api.example.org.evil.org/x",
    ] {
        crate::clear();
        let mut p = once(vec![set(0, "X"), fetch_to(bad)]);
        p.fetcher = Some(Arc::new(Net(n.clone(), 0)));
        let mut b = setup(p, &["abc"]);
        assert!(go(&mut b).unwrap_err().contains("refused"), "{bad}");
        assert_eq!(b.cells, ["abc"]);
    }
    assert_eq!(*n.lock().unwrap(), 0, "nothing was fetched");
    crate::clear();
    let mut p = once(vec![fetch_to("https://api.example.org/x")]);
    p.fetcher = Some(Arc::new(Net(n.clone(), 0)));
    let mut b = setup(p, &["abc"]);
    go(&mut b).unwrap();
    assert_eq!(*n.lock().unwrap(), 1);
}

#[test]
fn s2_a_slow_invocation_hits_the_deadline_and_the_fetch_cap_holds() {
    let _t = turn();
    exec::set_invocation_ms(150.0);
    let n = Arc::new(Mutex::new(0));
    let mut acts = vec![set(0, "X")];
    acts.extend((0..6).map(|_| fetch_to("https://api.example.org/x")));
    let mut p = once(acts);
    p.fetcher = Some(Arc::new(Net(n.clone(), 60)));
    let mut b = setup(p, &["abc"]);
    let e = go(&mut b).unwrap_err();
    exec::set_invocation_ms(plugin_api::wire::INVOCATION_MS as f64);
    assert!(e.contains("time budget exceeded"), "{e}");
    assert!(*n.lock().unwrap() < 6);
    assert_eq!(b.cells, ["abc"]);
    crate::clear();
    *n.lock().unwrap() = 0;
    let mut p = once(
        (0..9)
            .map(|_| fetch_to("https://api.example.org/x"))
            .collect(),
    );
    p.fetcher = Some(Arc::new(Net(n.clone(), 0)));
    let mut b = setup(p, &["abc"]);
    assert!(go(&mut b).unwrap_err().contains("more than 8 fetches"));
    assert_eq!(*n.lock().unwrap(), 8);
}

#[test]
fn a_plugin_that_times_out_three_times_is_switched_off() {
    let _t = turn();
    let mut b = setup(
        plugin(|_, _| Err("plugin stopped: time budget exceeded".into())),
        &["abc"],
    );
    for i in 1..=3 {
        let e = go(&mut b).unwrap_err();
        assert_eq!(crate::plugins()[0].enabled(), i < 3, "after {i}");
        assert_eq!(e.contains("switched off"), i == 3, "{e}");
    }
    assert!(go(&mut b).unwrap_err().contains("not enabled"));
}

#[test]
fn a_protected_document_is_refused_for_a_writing_plugin_only() {
    let _t = turn();
    let mut b = setup(once(vec![set(0, "X")]), &["abc"]);
    b.protected = true;
    assert_eq!(go(&mut b).unwrap_err(), bridge::PROTECTED);
    assert_eq!(b.cells, ["abc"]);
    crate::clear();
    let toml = MANIFEST.replace("document = \"write\"", "document = \"read\"");
    crate::add(plugin_with(&toml, |_, _| {
        answer(
            vec![Action::UiStatus {
                text: "seen".into(),
            }],
            None,
            vec![],
        )
    }))
    .unwrap();
    assert_eq!(go(&mut b).unwrap()["status"], "seen");
}

#[test]
fn panels_are_kept_and_an_undeclared_panel_is_refused() {
    let _t = turn();
    let w = vec![plugin_api::action::Widget::Text { text: "hi".into() }];
    let mut b = setup(
        once(vec![Action::UiPanel {
            panel: "side".into(),
            widgets: w.clone(),
        }]),
        &["abc"],
    );
    go(&mut b).unwrap();
    assert_eq!(crate::plugins()[0].ui.lock().unwrap().panels["side"], w);
    crate::clear();
    let mut b = setup(
        once(vec![Action::UiPanel {
            panel: "nope".into(),
            widgets: w,
        }]),
        &["abc"],
    );
    assert!(go(&mut b).is_err());
}

// ------------------------------------------------------------------------------------------------ the web bridge

mod web_bridge {
    use super::*;

    fn page(b: &mut Book) {
        let mut p = Plugin::new(
            manifest::parse(MANIFEST).unwrap(),
            Arc::new(bridge::PageGuest),
        );
        p.origins = vec![];
        crate::add(p).unwrap();
        let _ = b;
    }

    fn started(b: &mut Book) -> (Value, usize) {
        let id = go(b).unwrap()["pending"].clone();
        assert!(bridge::tick(b));
        match bridge::take_out().remove(0) {
            Out::Call {
                call,
                command,
                reads,
                ..
            } => {
                assert_eq!((call.clone(), command.as_str()), (id, "go"));
                (
                    call,
                    reads
                        .iter()
                        .find(|(n, _)| *n == "textmap")
                        .map(|(_, b)| b.len())
                        .unwrap_or(0),
                )
            }
            _ => panic!("a call first"),
        }
    }

    fn ans(call: &Value, actions: Value, more: bool) -> PageAnswer {
        PageAnswer {
            call: call.clone(),
            plugin: None,
            actions,
            blobs: vec![],
            reads: Value::Null,
            more,
            abort: false,
        }
    }

    fn sets(i: usize, t: &str) -> Value {
        json!([{"type": "run", "id": "cell.set", "params": {"index": i, "input": t}}])
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
        let mut b = book(&["a", "b"]);
        page(&mut b);
        let (call, map) = started(&mut b);
        assert!(map > 8, "the text map is a blob");
        assert!(bridge::locked() && bridge::input_locked());
        assert!(
            bridge::refuses("cell.set", true)
                && bridge::refuses("file.save", false)
                && bridge::refuses("edit.undo", false)
        );
        assert!(
            !bridge::refuses("selection.move", false),
            "moving around stays possible"
        );
        assert_eq!(
            go(&mut b).unwrap_err(),
            bridge::BUSY,
            "a second plugin command is refused"
        );
        let mut a = ans(
            &call,
            json!([{"type": "run", "id": "cell.set", "params": {"index": 0, "input": "X"}}, {"type": "run", "id": "cell.set", "params": {"index": 1, "input": "Y"}}]),
            false,
        );
        a.plugin = Some("test-plugin".into());
        bridge::answer(&mut b, a);
        assert!(!bridge::locked() && !bridge::input_locked());
        assert_eq!(b.cells, ["X", "Y"]);
        assert_eq!(error_of(bridge::take_out()), None);
        assert_eq!((b.undo.len(), b.steps), (1, 1), "one undo step");
    }

    #[test]
    fn a_second_round_keeps_the_lock_and_requested_reads_come_back() {
        let _t = turn();
        let mut b = book(&["a"]);
        page(&mut b);
        let (call, _) = started(&mut b);
        let mut a = ans(&call, sets(0, "1"), true);
        a.reads = json!([{"what": "textmap"}, {"what": "selection"}]);
        bridge::answer(&mut b, a);
        assert!(bridge::input_locked(), "locked again between rounds");
        match bridge::take_out().remove(0) {
            Out::Results {
                results,
                reads,
                error: None,
                ..
            } => assert_eq!((results.len(), reads.len()), (1, 2)),
            _ => panic!("results"),
        }
        bridge::answer(&mut b, ans(&call, sets(0, "2"), false));
        assert_eq!(b.cells, ["2"]);
        assert_eq!(b.undo.len(), 1, "two rounds, one undo step");
    }

    #[test]
    fn a_document_that_moved_is_stale_and_restored() {
        let _t = turn();
        let mut b = book(&["a"]);
        page(&mut b);
        let (call, _) = started(&mut b);
        bridge::answer(&mut b, ans(&call, sets(0, "1"), true));
        let _ = bridge::take_out();
        b.rev += 1; // something changed the document behind the lock
        bridge::answer(&mut b, ans(&call, sets(0, "2"), false));
        assert!(error_of(bridge::take_out()).unwrap().contains("stale"));
        assert_eq!(b.cells, ["a"], "round 1 is taken back");
        assert!(!bridge::input_locked());
    }

    #[test]
    fn no_answer_in_time_restores_and_a_late_answer_gets_an_error() {
        let _t = turn();
        let mut b = book(&["a"]);
        page(&mut b);
        crate::bridge::set_plugins(vec![], Some(1.0));
        page(&mut b);
        let (call, _) = started(&mut b);
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(!bridge::tick(&mut b));
        assert!(!bridge::input_locked());
        assert!(bridge::take_status()[0].contains("no answer in time"));
        bridge::answer(&mut b, ans(&call, sets(0, "late"), false));
        assert!(error_of(bridge::take_out()).unwrap().contains("ended"));
        assert_eq!(b.cells, ["a"]);
    }

    #[test]
    fn an_abort_and_a_refused_action_restore() {
        let _t = turn();
        let mut b = book(&["a"]);
        page(&mut b);
        let (call, _) = started(&mut b);
        bridge::answer(&mut b, ans(&call, sets(0, "1"), true));
        let _ = bridge::take_out();
        let mut a = ans(&call, json!([]), false);
        a.abort = true;
        bridge::answer(&mut b, a);
        assert_eq!(b.cells, ["a"]);
        let (call, _) = started(&mut b);
        bridge::answer(
            &mut b,
            ans(
                &call,
                json!([{"type": "run", "id": "file.saveAs", "params": {"path": "C:/x.xlsx"}}]),
                false,
            ),
        );
        assert!(error_of(bridge::take_out()).unwrap().contains("refused"));
        assert_eq!(b.cells, ["a"]);
        assert!(!bridge::input_locked());
    }

    #[test]
    fn a_panel_click_starts_at_its_first_round_and_is_refused_when_busy_protected_or_ended() {
        let _t = turn();
        let mut b = book(&["a"]);
        page(&mut b);
        let mut a = ans(&json!("h1"), sets(0, "P"), false);
        a.plugin = Some("test-plugin".into());
        bridge::answer(&mut b, a);
        assert_eq!(error_of(bridge::take_out()), None);
        assert_eq!((b.cells[0].as_str(), b.undo.len()), ("P", 1));
        // the same id again: it has ended
        let mut a = ans(&json!("h1"), sets(0, "Q"), false);
        a.plugin = Some("test-plugin".into());
        bridge::answer(&mut b, a);
        assert!(error_of(bridge::take_out()).unwrap().contains("ended"));
        // while a command is pending
        let (_call, _) = started(&mut b);
        let mut a = ans(&json!("h2"), sets(0, "R"), false);
        a.plugin = Some("test-plugin".into());
        bridge::answer(&mut b, a);
        assert_eq!(error_of(bridge::take_out()).as_deref(), Some(bridge::BUSY));
        bridge::reset();
        // on a protected document
        b.protected = true;
        let mut a = ans(&json!("h3"), sets(0, "S"), false);
        a.plugin = Some("test-plugin".into());
        bridge::answer(&mut b, a);
        assert_eq!(
            error_of(bridge::take_out()).as_deref(),
            Some(bridge::PROTECTED)
        );
        assert_eq!(b.cells, ["P"]);
    }

    #[test]
    fn a_protected_document_is_not_started_on() {
        let _t = turn();
        let mut b = book(&["a"]);
        page(&mut b);
        b.protected = true;
        assert_eq!(go(&mut b).unwrap_err(), bridge::PROTECTED);
        assert!(!bridge::locked());
    }

    #[test]
    fn replacing_the_document_drops_the_invocation_without_touching_the_new_one() {
        let _t = turn();
        let mut b = book(&["a"]);
        page(&mut b);
        let _ = started(&mut b);
        b.doc = 8;
        b.cells = vec!["new".into()];
        assert!(!bridge::tick(&mut b));
        assert_eq!(b.cells, ["new"]);
        assert!(!bridge::input_locked());
    }

    #[test]
    fn registering_again_with_a_shorter_list_unregisters_and_a_bad_entry_is_refused() {
        let _t = turn();
        let m: Value = json!({
            "id": "web-one", "version": "1.0.0", "api": 1, "apps": ["sheets"], "publisher": "T", "name": {"en": "One"},
            "permissions": {"document": "write"}, "command": [{"id": "go", "label": {"en": "Go"}, "reads": ["textmap"]}]
        });
        let docs_only = json!({"id": "docs-one", "version": "1.0.0", "api": 1, "apps": ["docs"], "publisher": "T", "name": {"en": "D"}});
        let (ids, errors) = bridge::set_plugins(
            vec![
                bridge::Registration {
                    manifest: m,
                    settings: None,
                    enabled: true,
                },
                bridge::Registration {
                    manifest: docs_only,
                    settings: None,
                    enabled: true,
                },
            ],
            None,
        );
        assert_eq!(ids, ["web-one"]);
        assert_eq!(
            errors.len(),
            1,
            "a Docs-only plugin is refused in Sheets: {errors:?}"
        );
        assert_eq!(crate::plugins().len(), 1);
        let (ids, _) = bridge::set_plugins(vec![], None);
        assert!(ids.is_empty());
        assert!(crate::plugins().is_empty());
    }
}
