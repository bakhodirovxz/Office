//! Engine-level tests of the Docs transliteration commands (headless `Session`, no UI).
use serde_json::json;
use uz_translit::{Direction, Options, convert};
use wordcraft_doc::para::OBJ;
use wordcraft_doc::props::CharProps;
use wordcraft_doc::{
    ControlKind, ControlLock, Document, InlineObject, Paragraph, Pos, StoryRef, para_block,
};
use wordcraft_engine::{ColumnBlock, Selection, Session};

const CYRL: &str = "tools.translitCyrl";
const LATN: &str = "tools.translitLatn";

fn bold() -> CharProps {
    CharProps {
        bold: Some(true),
        ..Default::default()
    }
}
fn italic() -> CharProps {
    CharProps {
        italic: Some(true),
        ..Default::default()
    }
}
fn lang(tag: &str) -> CharProps {
    CharProps {
        lang: Some(tag.into()),
        ..Default::default()
    }
}

/// A paragraph from `(text, props)` pieces.
fn para(pieces: &[(&str, CharProps)]) -> Paragraph {
    let mut p = Paragraph::new();
    for (t, props) in pieces {
        p.insert_text(p.len(), t, props).unwrap();
    }
    p
}

fn session(paras: Vec<Paragraph>) -> Session {
    let mut d = Document::new();
    d.body = paras.into_iter().map(para_block).collect();
    d.ensure_nonempty();
    let mut s = Session::new(d);
    docs_translit::install(&mut s);
    s
}

fn text(s: &Session, i: usize) -> String {
    s.doc
        .para(StoryRef::Body, &wordcraft_doc::Path::top(i))
        .unwrap()
        .text
        .clone()
}
fn p(s: &Session, i: usize) -> &Paragraph {
    s.doc
        .para(StoryRef::Body, &wordcraft_doc::Path::top(i))
        .unwrap()
}
fn cyr(t: &str) -> String {
    convert(t, Direction::ToCyrillic, &Options::default())
}
fn lat(t: &str) -> String {
    convert(t, Direction::ToLatin, &Options::default())
}
/// The runs that hold text, as (props, text).
fn runs(p: &Paragraph) -> Vec<(CharProps, String)> {
    p.run_ranges()
        .filter(|(r, _)| !r.is_empty())
        .map(|(r, c)| (c.clone(), p.text[r].to_string()))
        .collect()
}

#[test]
fn install_adds_the_commands_once() {
    let mut s = session(vec![para(&[("a", Default::default())])]);
    let n = s.registry.all().len();
    docs_translit::install(&mut s);
    assert_eq!(s.registry.all().len(), n);
    for id in [CYRL, LATN, "tools.translitAuto"] {
        assert!(s.registry.get(id).is_some_and(|c| c.mutates), "{id}");
    }
}

#[test]
fn selection_converts_only_the_selection_and_a_caret_converts_the_document() {
    let mut s = session(vec![
        para(&[("Shahar ", Default::default()), ("bola", bold())]),
        para(&[("Shahar bola", Default::default())]),
    ]);
    // Select "Shahar" in the first paragraph only (byte 0..6).
    s.sel = Selection {
        anchor: Pos::body(0, 0),
        focus: Pos::body(0, 6),
    };
    let r = s.run(CYRL, &json!({})).unwrap();
    assert_eq!(r["scope"], "selection");
    assert_eq!(text(&s, 0), format!("{} bola", cyr("Shahar")));
    assert_eq!(
        text(&s, 1),
        "Shahar bola",
        "text outside the selection is untouched"
    );
    assert_eq!(r["paragraphs"], 1);
    // The selection still covers the converted word.
    let (a, b) = s.sel.ordered();
    assert_eq!((a.off, b.off), (0, cyr("Shahar").len()));
    // A caret: the whole document (first paragraph again: already Cyrillic, so unchanged).
    s.sel = Selection::caret(Pos::body(1, 3));
    let r = s.run(CYRL, &json!({})).unwrap();
    assert_eq!(r["scope"], "document");
    assert_eq!(text(&s, 1), cyr("Shahar bola"));
    // scope "selection" with a caret does nothing.
    let before = s.doc.clone();
    s.sel = Selection::caret(Pos::body(1, 0));
    let r = s.run(LATN, &json!({"scope": "selection"})).unwrap();
    assert_eq!(r["paragraphs"], 0);
    assert_eq!(s.doc, before);
    // And both directions work.
    s.run(LATN, &json!({})).unwrap();
    assert_eq!(text(&s, 1), lat(&cyr("Shahar bola")));
}

#[test]
fn auto_follows_the_dominant_script() {
    let mut s = session(vec![para(&[("Shahar bola", Default::default())])]);
    s.run("tools.translitAuto", &json!({})).unwrap();
    assert_eq!(text(&s, 0), cyr("Shahar bola"));
    s.run("tools.translitAuto", &json!({})).unwrap();
    assert_eq!(text(&s, 0), "Shahar bola");
}

#[test]
fn one_undo_step_restores_the_document_and_redo_repeats_it() {
    let mut s = session(vec![
        para(&[("Shahar ", Default::default()), ("bola", bold())]),
        para(&[("O'zbek tili", italic())]),
        para(&[("Choy", Default::default())]),
    ]);
    s.sel = Selection::caret(Pos::body(0, 0));
    let before = s.doc.clone();
    s.run(CYRL, &json!({})).unwrap();
    let after = s.doc.clone();
    assert_ne!(after, before);
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc, before, "one undo restores every paragraph");
    s.run("edit.redo", &json!({})).unwrap();
    assert_eq!(s.doc, after);
}

#[test]
fn formatting_is_preserved_run_by_run() {
    let orig = para(&[
        ("Shahar ", Default::default()),
        ("bola ", bold()),
        ("maktab", italic()),
        (" va ", lang("uz-Latn-UZ")),
        ("choy", bold()),
    ]);
    let mut s = session(vec![orig.clone()]);
    s.sel = Selection::caret(Pos::body(0, 0));
    s.run(CYRL, &json!({})).unwrap();
    let (before, after) = (runs(&orig), runs(p(&s, 0)));
    assert_eq!(before.len(), after.len());
    for ((bp, bt), (ap, at)) in before.iter().zip(&after) {
        assert_eq!(bp, ap, "run properties identical apart from the text");
        assert_eq!(*at, cyr(bt).to_string(), "run text converted");
    }
    assert_eq!(p(&s, 0).props, orig.props);
    assert_eq!(p(&s, 0).mark, orig.mark);
}

#[test]
fn a_digraph_split_across_bold_and_plain_runs_converts_as_one_letter() {
    // S|h, C|h and O|' across run boundaries.
    let pieces = [
        ("Birinchi s", Default::default()),
        ("h", bold()),
        ("ahar, c", Default::default()),
        ("h", italic()),
        ("oy, o", Default::default()),
        ("'", bold()),
        ("zbek", Default::default()),
    ];
    let whole: String = pieces.iter().map(|(t, _)| *t).collect();
    let mut s = session(vec![para(&pieces)]);
    s.sel = Selection::caret(Pos::body(0, 0));
    s.run(CYRL, &json!({})).unwrap();
    assert_eq!(
        text(&s, 0),
        cyr(&whole),
        "the same text as converting the unsplit paragraph"
    );
    assert!(
        !text(&s, 0).contains(|c: char| c.is_ascii_alphabetic()),
        "{}",
        text(&s, 0)
    );
    // Runs still cover the text exactly and none is empty after normalize.
    let total: usize = p(&s, 0).runs.iter().map(|r| r.len).sum();
    assert_eq!(total, p(&s, 0).text.len());
    assert!(p(&s, 0).runs.iter().all(|r| r.len > 0));
    // Back to Latin.
    s.run(LATN, &json!({})).unwrap();
    assert_eq!(text(&s, 0), lat(&cyr(&whole)));
}

#[test]
fn field_codes_and_hyperlink_targets_are_untouched() {
    let link = CharProps {
        link: Some("https://example.org/Shahar".into()),
        ..Default::default()
    };
    let mut para1 = para(&[("Shahar ", Default::default())]);
    let field = InlineObject::Field {
        instr: "REF Shahar".into(),
        result: "Shahar".into(),
        locked: false,
        dirty: false,
        code: false,
        form: String::new(),
    };
    para1
        .insert_object(para1.len(), field.clone(), &Default::default())
        .unwrap();
    para1
        .insert_text(para1.len(), " bola ", &Default::default())
        .unwrap();
    para1.insert_text(para1.len(), "Shahar", &link).unwrap();
    let mut s = session(vec![para1]);
    s.sel = Selection::caret(Pos::body(0, 0));
    s.run(CYRL, &json!({})).unwrap();
    let q = p(&s, 0);
    assert_eq!(
        q.objects,
        vec![field],
        "the field code and its cached result are not touched"
    );
    assert_eq!(
        q.text,
        format!("{}{OBJ}{}", cyr("Shahar "), cyr(" bola Shahar"))
    );
    let linked = runs(q).into_iter().find(|(c, _)| c.link.is_some()).unwrap();
    assert_eq!(
        linked.0.link.as_deref(),
        Some("https://example.org/Shahar"),
        "the hyperlink target is a prop, never converted"
    );
    assert_eq!(linked.1, cyr("Shahar"), "the display text converts");
}

#[test]
fn field_results_follow_the_option() {
    let mk = || {
        let mut q = Paragraph::new();
        q.insert_text(0, "Shahar ", &Default::default()).unwrap();
        q.insert_object(
            q.len(),
            InlineObject::FieldStart {
                instr: "TOC \\o".into(),
                locked: false,
                dirty: false,
            },
            &Default::default(),
        )
        .unwrap();
        q.insert_text(q.len(), "Shahar", &Default::default())
            .unwrap();
        q.insert_object(q.len(), InlineObject::FieldEnd, &Default::default())
            .unwrap();
        q.insert_text(q.len(), " Shahar", &Default::default())
            .unwrap();
        q
    };
    let mut s = session(vec![mk()]);
    s.sel = Selection::caret(Pos::body(0, 0));
    let r = s.run(CYRL, &json!({})).unwrap();
    assert_eq!(r["skipped"]["field"], 0);
    assert_eq!(
        text(&s, 0).matches(&cyr("Shahar")).count(),
        3,
        "the result between FieldStart and FieldEnd converts by default"
    );
    assert!(
        matches!(p(&s, 0).objects[0], InlineObject::FieldStart { ref instr, .. } if instr == "TOC \\o")
    );

    let mut s = session(vec![mk()]);
    s.sel = Selection::caret(Pos::body(0, 0));
    let r = s.run(CYRL, &json!({"fieldResults": false})).unwrap();
    assert_eq!(r["skipped"]["field"], 1);
    let t = text(&s, 0);
    assert_eq!(t.matches("Shahar").count(), 1, "{t}");
    assert_eq!(t.matches(&cyr("Shahar")).count(), 2, "{t}");
}

#[test]
fn language_tags_follow_the_owner_decision() {
    // The first piece names Uzbek: untagged text takes the document default, which is en-US here.
    let pieces = [
        ("Shahar ", lang("uz-Latn-UZ")),
        ("shahar ", lang("ru-RU")),
        ("shahar ", lang("en-US")),
        ("shahar ", lang("de-DE")),
        ("shahar", lang("uz-Cyrl-UZ")),
    ];
    let mut s = session(vec![para(&pieces)]);
    s.sel = Selection::caret(Pos::body(0, 0));
    let r = s.run(CYRL, &json!({})).unwrap();
    let t = text(&s, 0);
    // ru and en are converted, de is skipped and counted.
    assert_eq!(t.matches("shahar").count(), 1, "{t}");
    assert_eq!(r["skipped"]["lang"], 1);
    // skipLangs also skips ru and en.
    let mut s = session(vec![para(&pieces)]);
    s.sel = Selection::caret(Pos::body(0, 0));
    let r = s.run(CYRL, &json!({"skipLangs": true})).unwrap();
    assert_eq!(text(&s, 0).matches("shahar").count(), 3);
    assert_eq!(r["skipped"]["lang"], 3);
    // The language prop itself never changes.
    assert_eq!(
        runs(p(&s, 0))
            .iter()
            .filter_map(|(c, _)| c.lang.clone())
            .collect::<Vec<_>>(),
        ["uz-Latn-UZ", "ru-RU", "en-US", "de-DE", "uz-Cyrl-UZ"]
    );
}

#[test]
fn a_column_selection_converts_only_the_block_and_never_the_whole_document() {
    let mut s = session(vec![
        para(&[("Shahar bola", Default::default())]),
        para(&[("Shahar bola", Default::default())]),
        para(&[("Shahar bola", Default::default())]),
    ]);
    // A column over bytes 0..6 of every line.
    let segs: Vec<(Pos, Pos)> = (0..3).map(|i| (Pos::body(i, 0), Pos::body(i, 6))).collect();
    s.sel = Selection {
        anchor: Pos::body(0, 0),
        focus: Pos::body(2, 6),
    };
    s.column = Some(ColumnBlock {
        sel: s.sel.clone(),
        anchor_x: 0.0,
        focus_x: 40.0,
        segments: segs,
        cells: None,
    });
    let r = s.run(CYRL, &json!({}));
    assert!(r.is_ok(), "{r:?}");
    for i in 0..3 {
        assert_eq!(
            text(&s, i),
            format!("{} bola", cyr("Shahar")),
            "row {i}: the block converts, the rest stays"
        );
    }
}

#[test]
fn a_locked_control_is_skipped() {
    use wordcraft_doc::ContentControl;
    let mut q = Paragraph::new();
    q.insert_text(0, "Shahar ", &Default::default()).unwrap();
    let mut c = ContentControl::new(ControlKind::RichText);
    c.lock = ControlLock::ContentLocked;
    q.insert_object(
        q.len(),
        InlineObject::ControlStart {
            control: Box::new(c),
        },
        &Default::default(),
    )
    .unwrap();
    q.insert_text(q.len(), "Shahar", &Default::default())
        .unwrap();
    q.insert_object(q.len(), InlineObject::ControlEnd, &Default::default())
        .unwrap();
    q.insert_text(q.len(), " Shahar", &Default::default())
        .unwrap();
    let mut s = session(vec![q]);
    s.sel = Selection::caret(Pos::body(0, 0));
    let r = s.run(CYRL, &json!({})).unwrap();
    assert_eq!(r["skipped"]["locked"], 1);
    let t = text(&s, 0);
    assert_eq!(
        t.matches("Shahar").count(),
        1,
        "the locked content stays: {t}"
    );
    assert_eq!(t.matches(&cyr("Shahar")).count(), 2, "{t}");
}

#[test]
fn tracked_changes_record_one_delete_and_one_insert_revision() {
    let mut s = session(vec![
        para(&[("Shahar ", Default::default()), ("bola", bold())]),
        para(&[("Choy", Default::default())]),
    ]);
    s.doc.settings.track_changes = true;
    s.sel = Selection::caret(Pos::body(0, 0));
    let r = s.run(CYRL, &json!({})).unwrap();
    assert_eq!(r["tracked"], true);
    assert_eq!(
        s.doc.revisions.len(),
        2,
        "one Delete and one Insert, however many runs changed"
    );
    // The old text is still there, marked deleted; the new text is an insertion with the same props.
    let q = p(&s, 0);
    let (mut deleted, mut inserted) = (String::new(), String::new());
    for (r, c) in q.run_ranges() {
        if c.del.is_some() {
            deleted.push_str(&q.text[r]);
        } else if c.ins.is_some() {
            inserted.push_str(&q.text[r]);
        }
    }
    assert_eq!(deleted, "Shahar bola");
    assert_eq!(inserted, cyr("Shahar bola"));
    assert!(
        runs(q)
            .iter()
            .any(|(c, t)| c.bold == Some(true) && c.ins.is_some() && *t == cyr("bola")),
        "the inserted text keeps the run's bold"
    );
    // Undo is still one step.
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(text(&s, 0), "Shahar bola");
    assert!(s.doc.revisions.is_empty());
}

#[test]
fn headers_and_text_boxes_are_part_of_the_document_scope() {
    use wordcraft_doc::PartKind;
    use wordcraft_doc::para::ShapeKind;
    let mut d = Document::new();
    let h = d.add_part(
        PartKind::Header,
        vec![para_block(para(&[("Shahar", Default::default())]))],
    );
    let t = d.add_part(
        PartKind::TextBox,
        vec![para_block(para(&[("Shahar", Default::default())]))],
    );
    let shape = InlineObject::Shape {
        kind: ShapeKind::TextBox,
        w: 144.0,
        h: 72.0,
        fill: None,
        stroke: None,
        stroke_width: 0.75,
        float: Default::default(),
        story: Some(t),
        effects: Default::default(),
        freeform: None,
        extra: Default::default(),
    };
    let mut body = para(&[("Shahar", Default::default())]);
    body.insert_object(0, shape, &Default::default()).unwrap();
    d.body = vec![para_block(body)];
    let mut s = Session::new(d);
    docs_translit::install(&mut s);
    s.sel = Selection::caret(Pos::body(0, 0));
    s.run(CYRL, &json!({})).unwrap();
    for id in [h, t] {
        assert_eq!(
            s.doc
                .para(StoryRef::Part(id), &wordcraft_doc::Path::top(0))
                .unwrap()
                .text,
            cyr("Shahar"),
            "part {id}"
        );
    }
}

/// The palette looks a command up by its label, so each label must be a row of the canonical catalog (the ribbon uses the same text).
#[test]
fn command_labels_are_catalog_rows() {
    let tsv = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../i18n/shared.tsv"
    ))
    .unwrap();
    let sources: Vec<&str> = tsv.lines().filter_map(|l| l.split('\t').nth(2)).collect();
    for spec in docs_translit::specs() {
        assert!(
            sources.contains(&&*spec.label),
            "{} is not a row of i18n/shared.tsv",
            spec.label
        );
    }
}

#[test]
fn latin_alphabet_param_writes_2026_and_rejects_unknown_values() {
    let mut s = session(vec![para(&[("Шаҳар ўғил", Default::default())])]);
    s.run("tools.translitLatn", &json!({"latinAlphabet": "2026"}))
        .unwrap();
    assert_eq!(text(&s, 0), "Şahar öğil");
    assert!(
        s.run("tools.translitLatn", &json!({"latinAlphabet": "1999"}))
            .is_err()
    );
    let mut d = session(vec![para(&[("Шаҳар ўғил", Default::default())])]);
    d.run("tools.translitLatn", &json!({})).unwrap();
    assert_eq!(text(&d, 0), lat("Шаҳар ўғил"));
}
