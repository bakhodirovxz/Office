//! Plugin #2 (PLAN phase 7 section 7): insert and check template fields. The field list is configuration only
//! (`settings.fields = [{name, label, kind}]`), so no field name lives in this code.
//!
//! Every call is stateless (a native call starts a fresh instance), so each answer redraws the whole panel: the filter text
//! comes with the `filter` widget's call, the caret's field with a `selectionChanged` event. An event may only answer with `ui.*`
//! (D8) and cannot run `developer.control`, so the caret's field is found from the selected text: it is highlighted while the
//! selection is a field's label (its placeholder).
use plugin_sdk::plugin_api::action::ListItem;
use plugin_sdk::{Action, Call, CallKind, Next, ReadReq, ReadWhat, Reply, Widget};
use serde_json::{Value, json};

/// `key, en, ru, uz-Latn, uz-Cyrl, review` per line; the first line is the header.
const STRINGS: &str = include_str!("../strings.tsv");
const PANEL: &str = "fields";
const MAX_FIELDS: usize = 256;
const SHOWN_NAMES: usize = 8;

fn tr(lang: &str, key: &str, args: &[(&str, &str)]) -> String {
    let col = match lang.to_ascii_lowercase().as_str() {
        "ru" => 2,
        "uz-latn" => 3,
        "uz-cyrl" => 4,
        _ => 1,
    };
    let mut t = STRINGS
        .lines()
        .skip(1)
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .find(|c| c[0] == key)
        .map_or_else(|| key.to_string(), |c| c[col].to_string());
    for (k, v) in args {
        t = t.replace(&format!("{{{k}}}"), v);
    }
    t
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Text,
    Bookmark,
}

struct Field {
    name: String,
    label: String,
    kind: Kind,
}

/// Panel text must hold no control or bidi character (docs/plugins.md section 6) or the host refuses the whole panel.
fn clean(s: &str, max: usize) -> String {
    s.chars()
        .filter(|c| !c.is_control() && !matches!(c, '\u{200B}'..='\u{200F}' | '\u{2028}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}'))
        .take(max)
        .collect()
}

/// The bookmark rule of the engine (`insert.bookmark`): a letter first, no spaces, at most 40 characters.
fn bookmark_ok(n: &str) -> bool {
    !n.is_empty()
        && !n.contains(' ')
        && n.len() <= 40
        && !n.starts_with(|c: char| c.is_ascii_digit())
}

/// The valid entries of `settings.fields` with their position in the list (the widget ids), and the number ignored.
fn fields(settings: &serde_json::Map<String, Value>) -> (Vec<(usize, Field)>, usize) {
    let list = settings
        .get("fields")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let (mut out, mut bad) = (Vec::new(), 0);
    for (i, e) in list.iter().enumerate().take(MAX_FIELDS) {
        let s = |k: &str| e.get(k).and_then(Value::as_str);
        let kind = match s("kind") {
            None | Some("text") => Kind::Text,
            Some("bookmark") => Kind::Bookmark,
            _ => {
                bad += 1;
                continue;
            }
        };
        match (s("name"), s("label")) {
            (Some(n), l)
                if !n.is_empty()
                    && n.chars().count() <= 64
                    && n == clean(n, 64)
                    && (kind == Kind::Text || bookmark_ok(n)) =>
            {
                let label = clean(l.unwrap_or(n), 100);
                out.push((
                    i,
                    Field {
                        name: n.to_string(),
                        label: if label.trim().is_empty() {
                            n.to_string()
                        } else {
                            label
                        },
                        kind,
                    },
                ));
            }
            _ => bad += 1,
        }
    }
    (out, bad)
}

/// The names for a text line: the first few, then "+N more".
fn names(lang: &str, v: &[String]) -> String {
    let mut s = v
        .iter()
        .take(SHOWN_NAMES)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if v.len() > SHOWN_NAMES {
        s += &format!(
            " ({})",
            tr(lang, "more", &[("n", &(v.len() - SHOWN_NAMES).to_string())])
        );
    }
    s
}

/// The whole panel: heading, notes (check results, ignored entries), filter, check button, the caret's field and one insert button
/// per field that matches the filter.
fn panel(
    lang: &str,
    list: &[(usize, Field)],
    filter: &str,
    here: Option<usize>,
    notes: &[String],
) -> Action {
    let mut w = vec![Widget::Heading {
        text: tr(lang, "heading", &[]),
    }];
    w.extend(notes.iter().map(|t| Widget::Text {
        text: clean_text(t),
    }));
    w.push(Widget::TextField {
        id: "filter".into(),
        value: clean(filter, 100),
        placeholder: Some(tr(lang, "filter", &[])),
    });
    w.push(Widget::Button {
        id: "check".into(),
        label: tr(lang, "check", &[]),
    });
    if let Some((_, f)) = here.and_then(|i| list.iter().find(|(j, _)| *j == i)) {
        w.push(Widget::Text {
            text: tr(lang, "here", &[("name", &f.label)]),
        });
    }
    let q = filter.trim().to_lowercase();
    let items: Vec<ListItem> = list
        .iter()
        .filter(|(_, f)| {
            q.is_empty()
                || f.label.to_lowercase().contains(&q)
                || f.name.to_lowercase().contains(&q)
        })
        .map(|(i, f)| ListItem {
            id: Some(format!("ins-{i}")),
            text: format!(
                "{}{}: {}",
                if here == Some(*i) { "\u{25BA} " } else { "" },
                tr(lang, "insert", &[]),
                f.label
            ),
        })
        .collect();
    if list.is_empty() {
        w.push(Widget::Text {
            text: tr(lang, "empty", &[]),
        });
    } else if items.is_empty() {
        w.push(Widget::Text {
            text: tr(lang, "nomatch", &[]),
        });
    } else {
        w.push(Widget::List { items });
    }
    Action::UiPanel {
        panel: PANEL.into(),
        widgets: w,
    }
}

fn clean_text(t: &str) -> String {
    t.chars().take(1024).collect()
}

fn run(id: &str, params: Value) -> Action {
    Action::Run {
        id: id.into(),
        params,
    }
}

/// The actions that put field `f` at the caret (PLAN 7): a plain text content control tagged `name` with the label as its
/// placeholder and delete-locked; a bookmark kind inserts the label and bookmarks exactly those characters.
fn insert(f: &Field) -> Vec<Action> {
    match f.kind {
        Kind::Text => vec![
            run(
                "developer.plainText",
                json!({"title": f.label, "tag": f.name}),
            ),
            run(
                "developer.properties",
                json!({"placeholder": f.label, "lockDelete": true}),
            ),
        ],
        Kind::Bookmark => {
            let mut a = vec![run("text.insert", json!({"text": f.label}))];
            a.extend(
                (0..f.label.chars().count()).map(|_| run("caret.left", json!({"extend": true}))),
            );
            a.push(run("insert.bookmark", json!({"name": f.name})));
            a
        }
    }
}

/// Tags of the content controls and names of the bookmarks in a `document.inspect` result (body paragraphs: the engine's read stops
/// at 2,000 blocks and does not list objects inside tables).
fn scan(v: &Value, tags: &mut Vec<String>, marks: &mut Vec<String>) {
    match v {
        Value::Object(o) => {
            match o.get("type").and_then(Value::as_str) {
                Some("controlStart") => {
                    if let Some(t) = o
                        .get("control")
                        .and_then(|c| c.get("tag"))
                        .and_then(Value::as_str)
                        .filter(|t| !t.is_empty())
                    {
                        tags.push(t.to_string());
                    }
                }
                Some("bookmarkStart") => {
                    marks.extend(o.get("name").and_then(Value::as_str).map(str::to_string))
                }
                _ => {}
            }
            o.values().for_each(|x| scan(x, tags, marks));
        }
        Value::Array(a) => a.iter().for_each(|x| scan(x, tags, marks)),
        _ => {}
    }
}

fn check(lang: &str, list: &[(usize, Field)], bad: usize, inspect: &Value) -> Vec<String> {
    let (mut tags, mut marks) = (Vec::new(), Vec::new());
    scan(inspect, &mut tags, &mut marks);
    let missing: Vec<String> = list
        .iter()
        .filter(|(_, f)| !(if f.kind == Kind::Text { &tags } else { &marks }).contains(&f.name))
        .map(|(_, f)| f.name.clone())
        .collect();
    let mut unknown: Vec<String> = tags
        .into_iter()
        .filter(|t| {
            !list
                .iter()
                .any(|(_, f)| f.kind == Kind::Text && &f.name == t)
        })
        .collect();
    unknown.sort();
    unknown.dedup();
    let mut out = Vec::new();
    if missing.is_empty() && unknown.is_empty() {
        out.push(tr(lang, "allok", &[("n", &list.len().to_string())]));
    }
    if !missing.is_empty() {
        out.push(tr(lang, "missing", &[("names", &names(lang, &missing))]));
    }
    if !unknown.is_empty() {
        out.push(tr(lang, "unknown", &[("names", &names(lang, &unknown))]));
    }
    if bad > 0 {
        out.push(tr(lang, "ignored", &[("n", &bad.to_string())]));
    }
    out
}

fn ask_selection() -> Reply {
    Reply {
        next: Some(Next {
            reads: vec![ReadReq {
                what: ReadWhat::Selection,
                scope: None,
                index: None,
            }],
        }),
        ..Default::default()
    }
}

fn ask_inspect() -> Reply {
    Reply {
        actions: vec![run("document.inspect", json!({"text": false}))],
        next: Some(Next { reads: vec![] }),
        ..Default::default()
    }
}

pub fn handle(call: Call, blobs: Vec<Vec<u8>>) -> Result<Reply, String> {
    let lang = call.lang.clone();
    let (list, bad) = fields(&call.settings);
    let ignored = |bad: usize| {
        if bad > 0 {
            vec![tr(&lang, "ignored", &[("n", &bad.to_string())])]
        } else {
            vec![]
        }
    };
    let show = |filter: &str, here: Option<usize>, notes: &[String]| Reply {
        actions: vec![panel(&lang, &list, filter, here, notes)],
        ..Default::default()
    };
    let is = |s: &Option<String>, v: &str| s.as_deref() == Some(v);
    match call.kind {
        CallKind::Command if is(&call.command, "open") => Ok(show("", None, &ignored(bad))),
        CallKind::Command if is(&call.command, "check") => Ok(ask_inspect()),
        CallKind::Command => Err(format!("unknown command {:?}", call.command)),
        CallKind::Ui => {
            let w = call.widget.as_deref().unwrap_or_default();
            if w == "filter" {
                let v = call
                    .params
                    .get("value")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                Ok(show(v, None, &ignored(bad)))
            } else if w == "check" {
                Ok(ask_inspect())
            } else if let Some(i) = w.strip_prefix("ins-").and_then(|i| i.parse::<usize>().ok()) {
                let (_, f) = list
                    .iter()
                    .find(|(j, _)| *j == i)
                    .ok_or_else(|| tr(&lang, "unknownfield", &[]))?;
                let mut r = Reply {
                    actions: insert(f),
                    ..Default::default()
                };
                r.actions.push(Action::UiStatus {
                    text: tr(&lang, "inserted", &[("name", &f.label)]),
                });
                Ok(r)
            } else {
                Err(format!("unknown widget {w:?}"))
            }
        }
        CallKind::Event if is(&call.event, "selectionChanged") => Ok(ask_selection()),
        CallKind::Event => Ok(Reply::default()),
        CallKind::Resume if call.reads.contains_key("selection") => {
            // The read is the selected paragraphs' text: object marks (U+FFFC: a control's start and end) and line ends are not the label.
            let text = String::from_utf8_lossy(plugin_sdk::read(&call, &blobs, "selection")?)
                .replace('\u{fffc}', "")
                .trim()
                .to_string();
            let here = (!text.is_empty())
                .then(|| list.iter().find(|(_, f)| f.label == text).map(|(i, _)| *i))
                .flatten();
            Ok(show("", here, &ignored(bad)))
        }
        CallKind::Resume => {
            let inspect = call
                .results
                .first()
                .ok_or("document.inspect gave no result")?;
            let notes = check(&lang, &list, bad, inspect);
            let mut r = show("", None, &notes);
            r.actions.push(Action::UiStatus {
                text: clean(&notes.join(". "), 256),
            });
            Ok(r)
        }
    }
}

plugin_sdk::export!(handle);

#[cfg(test)]
mod tests {
    use super::*;

    fn call(kind: &str, extra: Value) -> Call {
        let mut v = json!({"kind": kind, "api": 1, "api_minor": 0, "app": "docs", "lang": "en",
        "settings": {"fields": [
            {"name": "field_a", "label": "Sample text"},
            {"name": "field_b", "label": "Sample date", "kind": "text"},
            {"name": "mark_a", "label": "Sample mark", "kind": "bookmark"},
            {"name": "bad name", "label": "x", "kind": "bookmark"},
            {"label": "no name"}
        ]}});
        v.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        serde_json::from_value(v).unwrap()
    }

    fn widgets(r: &Reply) -> &Vec<Widget> {
        let Some(Action::UiPanel { widgets, .. }) = r.actions.first() else {
            panic!("{:?}", r.actions)
        };
        widgets
    }

    fn items(r: &Reply) -> Vec<String> {
        widgets(r)
            .iter()
            .find_map(|w| {
                if let Widget::List { items } = w {
                    Some(items.iter().map(|i| i.text.clone()).collect())
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }

    #[test]
    fn open_lists_valid_fields_and_counts_ignored() {
        let r = handle(call("command", json!({"command": "open"})), vec![]).unwrap();
        assert_eq!(
            items(&r),
            [
                "Insert: Sample text",
                "Insert: Sample date",
                "Insert: Sample mark"
            ]
        );
        assert!(
            widgets(&r)
                .iter()
                .any(|w| matches!(w, Widget::Text { text } if text.starts_with("2 invalid")))
        );
    }

    #[test]
    fn filter_narrows_the_list() {
        let r = handle(
            call(
                "ui",
                json!({"widget": "filter", "params": {"value": "DATE"}}),
            ),
            vec![],
        )
        .unwrap();
        assert_eq!(items(&r), ["Insert: Sample date"]);
        let r = handle(
            call(
                "ui",
                json!({"widget": "filter", "params": {"value": "zzz"}}),
            ),
            vec![],
        )
        .unwrap();
        assert!(
            widgets(&r)
                .iter()
                .any(|w| matches!(w, Widget::Text { text } if text.contains("No field matches")))
        );
    }

    #[test]
    fn insert_runs_the_content_control_commands() {
        let r = handle(call("ui", json!({"widget": "ins-1"})), vec![]).unwrap();
        assert_eq!(
            r.actions[..2],
            [
                run(
                    "developer.plainText",
                    json!({"title": "Sample date", "tag": "field_b"})
                ),
                run(
                    "developer.properties",
                    json!({"placeholder": "Sample date", "lockDelete": true})
                ),
            ]
        );
        assert!(matches!(r.actions[2], Action::UiStatus { .. }));
    }

    #[test]
    fn bookmark_kind_selects_the_label_then_bookmarks_it() {
        let r = handle(call("ui", json!({"widget": "ins-2"})), vec![]).unwrap();
        let n = "Sample mark".chars().count();
        assert_eq!(
            r.actions[0],
            run("text.insert", json!({"text": "Sample mark"}))
        );
        assert_eq!(r.actions.len(), n + 3);
        assert_eq!(
            r.actions[n + 1],
            run("insert.bookmark", json!({"name": "mark_a"}))
        );
    }

    #[test]
    fn unknown_field_is_an_error() {
        assert!(handle(call("ui", json!({"widget": "ins-9"})), vec![]).is_err());
    }

    #[test]
    fn selection_event_asks_for_the_selection_and_highlights() {
        let r = handle(call("event", json!({"event": "selectionChanged"})), vec![]).unwrap();
        assert!(r.actions.is_empty() && r.next.is_some());
        let mut c = call(
            "resume",
            json!({"event": "selectionChanged", "reads": {"selection": 0}}),
        );
        c.reads.insert("selection".into(), 0);
        let sel = "\u{fffc}Sample date\u{fffc}\n".as_bytes().to_vec();
        let r = handle(c, vec![sel]).unwrap();
        assert_eq!(items(&r)[1], "\u{25BA} Insert: Sample date");
        assert!(
            widgets(&r)
                .iter()
                .any(|w| matches!(w, Widget::Text { text } if text == "Caret is in: Sample date"))
        );
    }

    #[test]
    fn check_lists_missing_and_unknown_tags() {
        let doc = json!({"blocks": [
            {"objects": [{"type": "controlStart", "control": {"tag": "field_a"}}, {"type": "controlEnd"}, {"type": "controlStart", "control": {"tag": "stray"}}]},
            {"objects": [{"type": "bookmarkStart", "name": "other"}]}
        ]});
        let r = handle(
            call("resume", json!({"command": "check", "results": [doc]})),
            vec![],
        )
        .unwrap();
        let texts: Vec<_> = widgets(&r)
            .iter()
            .filter_map(|w| {
                if let Widget::Text { text } = w {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert!(texts.contains(&"Missing from the document: field_b, mark_a"));
        assert!(texts.contains(&"Not in the field list: stray"));
    }

    #[test]
    fn four_languages_exist_for_every_key() {
        for l in STRINGS.lines().skip(1) {
            let c: Vec<_> = l.split('\t').collect();
            assert_eq!(c.len(), 6, "{l}");
            assert!(c[1..5].iter().all(|x| !x.is_empty()), "{l}");
        }
    }
}
