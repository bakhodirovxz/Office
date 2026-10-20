//! Office overlay (patches/deckcraft/0120): the transliteration commands of `cmd/translit.rs`.
use std::sync::Arc;

use deckcraft_engine::{Session, TextSel};
use deckcraft_model::text::{Action, Hyperlink, Run, RunKind, RunProps, TextBody};
use deckcraft_model::{Paragraph, Shape, ShapeId, ShapeKind, find_shape, find_shape_mut};
use serde_json::json;
use uz_translit::{Direction, Options, convert};

const CYRL: &str = "tools.translitCyrl";
const LATN: &str = "tools.translitLatn";

fn cyr(t: &str) -> String {
    convert(t, Direction::ToCyrillic, &Options::default())
}

fn session() -> Session {
    Session::with_new()
}

fn add(s: &mut Session, text: &str) -> ShapeId {
    let r = s.execute("shape.insert", &json!({"preset": "rect", "rect": [80.0, 80.0, 420.0, 160.0], "text": text})).unwrap();
    ShapeId(r["id"].as_u64().unwrap() as _)
}

fn deselect(s: &mut Session) {
    s.select(|_, sel| {
        sel.shapes.clear();
        sel.text = None;
    })
    .unwrap();
}

fn with_shape(s: &mut Session, slide: usize, id: ShapeId, f: impl FnOnce(&mut Shape)) {
    s.edit(|doc, _| {
        let sl = Arc::make_mut(&mut doc.slides[slide]);
        f(find_shape_mut(&mut sl.shapes, id).unwrap());
        Ok(())
    })
    .unwrap();
}

fn text_of(s: &Session, slide: usize, id: ShapeId) -> String {
    find_shape(&s.doc().unwrap().doc.slides[slide].shapes, id).unwrap().text.as_ref().unwrap().text()
}

fn notes(s: &mut Session, slide: usize, text: &str) {
    s.edit(|doc, _| {
        Arc::make_mut(&mut doc.slides[slide]).notes = TextBody::from_text(text);
        Ok(())
    })
    .unwrap();
}

fn run(text: &str, props: RunProps) -> Run {
    Run { text: text.into(), props, kind: RunKind::Text }
}

#[test]
fn the_ids_are_registered() {
    let s = session();
    for id in [CYRL, LATN, "tools.translitAuto"] {
        assert!(s.commands().iter().any(|c| c.id == id), "{id}");
    }
}

#[test]
fn the_deck_converts_every_shape_and_the_notes_when_nothing_is_selected() {
    let mut s = session();
    let a = add(&mut s, "Toshkent shahri");
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    let b = add(&mut s, "yangi ish o'rni");
    notes(&mut s, 0, "salom dunyo");
    deselect(&mut s);
    let r = s.execute(CYRL, &json!({})).unwrap();
    assert_eq!((r["scope"].as_str(), r["direction"].as_str()), (Some("deck"), Some("cyrl")));
    assert_eq!(text_of(&s, 0, a), cyr("Toshkent shahri"));
    assert_eq!(text_of(&s, 1, b), cyr("yangi ish o'rni"));
    assert_eq!(s.doc().unwrap().doc.slides[0].notes.text(), cyr("salom dunyo"));
    assert!(r["paragraphs"].as_u64().unwrap() >= 3);
    // and back, with the direction found from the text
    let r = s.execute("tools.translitAuto", &json!({})).unwrap();
    assert_eq!(r["direction"], "latn");
    assert_eq!(s.doc().unwrap().doc.slides[0].notes.text(), "salom dunyo");
}

#[test]
fn selected_shapes_convert_and_the_rest_stays() {
    let mut s = session();
    let a = add(&mut s, "salom");
    let b = add(&mut s, "dunyo");
    s.select(|_, sel| {
        sel.shapes = vec![a];
        sel.text = None;
    })
    .unwrap();
    let r = s.execute(CYRL, &json!({})).unwrap();
    assert_eq!(r["scope"], "selection");
    assert_eq!((text_of(&s, 0, a), text_of(&s, 0, b)), (cyr("salom"), "dunyo".to_string()));
    // the slide scope reaches the other one
    s.execute(CYRL, &json!({"scope": "slide"})).unwrap();
    assert_eq!(text_of(&s, 0, b), cyr("dunyo"));
}

#[test]
fn a_text_range_converts_only_the_range_and_the_selection_follows() {
    let mut s = session();
    let a = add(&mut s, "salom dunyo yangi");
    s.select(|_, sel| {
        sel.shapes = vec![a];
        sel.text = Some(TextSel { shape: a, anchor: (0, 6), caret: (0, 11), ..Default::default() });
    })
    .unwrap();
    s.execute(CYRL, &json!({})).unwrap();
    assert_eq!(text_of(&s, 0, a), format!("salom {} yangi", cyr("dunyo")));
    let t = s.doc().unwrap().selection.text.clone().unwrap();
    assert_eq!((t.anchor, t.caret), ((0, 6), (0, 6 + cyr("dunyo").chars().count())));
    // a digraph shortens the text: the end moves back
    let b = add(&mut s, "shahar bor");
    s.select(|_, sel| {
        sel.shapes = vec![b];
        sel.text = Some(TextSel { shape: b, anchor: (0, 0), caret: (0, 6), ..Default::default() });
    })
    .unwrap();
    s.execute(CYRL, &json!({})).unwrap();
    let t = s.doc().unwrap().selection.text.clone().unwrap();
    assert_eq!(t.caret.1, cyr("shahar").chars().count());
    assert!(text_of(&s, 0, b).ends_with(" bor"));
}

#[test]
fn run_properties_fields_breaks_and_link_targets_are_untouched() {
    let mut s = session();
    let a = add(&mut s, "x");
    let bold = RunProps { bold: Some(true), ..Default::default() };
    let link = RunProps {
        link: Some(Hyperlink {
            action: Action::Url { url: "https://example.org/shahar".into() },
            tooltip: String::new(),
            highlight_click: false,
            sound: None,
            stop_sound: false,
            zoom_return: None,
        }),
        lang: Some("ru-RU".into()),
        ..Default::default()
    };
    with_shape(&mut s, 0, a, |sh| {
        sh.text.as_mut().unwrap().paragraphs = vec![Paragraph {
            runs: vec![
                run("s", bold.clone()), // the digraph sh spans two runs
                run("hahar ", Default::default()),
                Run { text: "17".into(), props: bold.clone(), kind: RunKind::Field { field: "slidenum".into() } },
                Run { text: String::new(), props: Default::default(), kind: RunKind::Break },
                run("salom", link.clone()),
            ],
            ..Default::default()
        }];
    });
    let before = s.doc().unwrap().doc.clone();
    let r = s.execute(CYRL, &json!({"scope": "slide"})).unwrap();
    assert_eq!(r["skipped"]["field"], 1);
    let para = &find_shape(&s.doc().unwrap().doc.slides[0].shapes, a).unwrap().text.as_ref().unwrap().paragraphs[0];
    let kinds: Vec<&RunKind> = para.runs.iter().map(|r| &r.kind).collect();
    assert_eq!(kinds.iter().filter(|k| ***k == RunKind::Text).count(), 3);
    assert!(para.runs.iter().any(|r| matches!(&r.kind, RunKind::Field { field } if field == "slidenum") && r.text == "17"));
    assert!(para.runs.iter().any(|r| r.kind == RunKind::Break));
    // the "sh" typed over a bold and a plain run became one letter, in the first run's format
    assert_eq!(format!("{}{}", para.runs[0].text, para.runs[1].text), cyr("shahar "));
    assert_eq!(para.runs[0].text, "ш");
    assert_eq!(para.runs[0].props, bold);
    let l = para.runs.last().unwrap();
    assert_eq!((l.text.as_str(), l.props.clone()), (cyr("salom").as_str(), link));
    // nothing but text changed
    let was = find_shape(&before.slides[0].shapes, a).unwrap();
    assert_eq!(find_shape(&s.doc().unwrap().doc.slides[0].shapes, a).unwrap().geom, was.geom);
}

#[test]
fn tables_and_groups_are_reached() {
    let mut s = session();
    s.execute("insert.table", &json!({"rows": 2, "cols": 2})).unwrap();
    let tid = s.doc().unwrap().selection.shapes[0];
    with_shape(&mut s, 0, tid, |sh| {
        let ShapeKind::Table(t) = &mut sh.kind else { panic!("not a table") };
        t.rows[1].cells[0].text = TextBody::from_text("yangi");
    });
    let a = add(&mut s, "salom");
    let b = add(&mut s, "dunyo");
    s.execute("arrange.group", &json!({"ids": [a.0, b.0]})).unwrap();
    deselect(&mut s);
    s.execute(CYRL, &json!({})).unwrap();
    let sl = &s.doc().unwrap().doc.slides[0];
    let ShapeKind::Table(t) = &find_shape(&sl.shapes, tid).unwrap().kind else { panic!() };
    assert_eq!(t.rows[1].cells[0].text.text(), cyr("yangi"));
    assert_eq!(find_shape(&sl.shapes, a).unwrap().text.as_ref().unwrap().text(), cyr("salom"));
    assert_eq!(find_shape(&sl.shapes, b).unwrap().text.as_ref().unwrap().text(), cyr("dunyo"));
}

#[test]
fn one_command_is_one_undo_step_and_redo_works() {
    let mut s = session();
    let a = add(&mut s, "salom");
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    let b = add(&mut s, "dunyo");
    notes(&mut s, 1, "yangi");
    deselect(&mut s);
    let before = s.doc().unwrap().doc.clone();
    let steps = s.doc().unwrap().history.undo.len();
    s.execute(CYRL, &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 1, "three bodies on two slides, one step");
    let after = s.doc().unwrap().doc.clone();
    assert_ne!(*after, *before);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(*s.doc().unwrap().doc, *before);
    assert_eq!(text_of(&s, 0, a), "salom");
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(*s.doc().unwrap().doc, *after);
    assert_eq!(text_of(&s, 1, b), cyr("dunyo"));
    // nothing left to convert: no step
    s.execute(CYRL, &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 1);
}

#[test]
fn a_bad_scope_is_an_error() {
    let mut s = session();
    assert!(s.execute(CYRL, &json!({"scope": "sheet"})).is_err());
}

/// Release timing for docs/phase2/perf.md: `cargo test --release -p deckcraft-engine --test translit -- --ignored --nocapture`.
#[test]
#[ignore]
fn timing_on_a_large_deck() {
    let words = ["Toshkent shahri", "O'zbekiston Respublikasi", "yangi ish o'rni", "Xodimlar bo'limi", "mehnat shartnomasi", "ish haqi"];
    for slides in [100usize, 500] {
        let mut s = session();
        s.edit(|doc, _| {
            let base = (*doc.slides[0]).clone();
            for i in 0..slides {
                let mut sl = base.clone();
                sl.id = deckcraft_model::SlideId(doc.alloc_id().unwrap());
                for k in 0..8 {
                    let mut sh = Shape::default();
                    sh.id = ShapeId(doc.alloc_id().unwrap());
                    sh.text = Some(TextBody::from_text(&format!("{}\n{}", words[(i + k) % 6], words[(i + k + 1) % 6])));
                    sl.shapes.push(sh);
                }
                sl.notes = TextBody::from_text(words[i % 6]);
                doc.slides.push(Arc::new(sl));
            }
            Ok(())
        })
        .unwrap();
        deselect(&mut s);
        let t = std::time::Instant::now();
        let r = s.execute(CYRL, &json!({})).unwrap();
        let total = t.elapsed().as_secs_f64() * 1e3;
        let t = std::time::Instant::now();
        s.execute("edit.undo", &json!({})).unwrap();
        println!(
            "{slides} slides: {} paragraphs, {} runs, total {total:.1} ms, convert {:.1} ms, apply {:.1} ms, undo {:.2} ms",
            r["paragraphs"],
            r["runs"],
            r["ms"]["convert"],
            r["ms"]["apply"],
            t.elapsed().as_secs_f64() * 1e3
        );
    }
}
