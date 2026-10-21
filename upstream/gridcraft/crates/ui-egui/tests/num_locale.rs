//! Office overlay (patch 0130): the interface language sets the number locale of typed input. A
//! date typed `12.10.2026` is 12 October (not 10 December), `3,5` is a number, and opening a cell
//! for edit and confirming it unchanged never changes its value. The engine only sees en-US text.
use gridcraft_engine::{
    Session,
    core::{CellRef, NumLocale, Value},
};
use gridcraft_ui_egui::{SheetApp, i18n::Language};
use serde_json::json;

fn app(language: Language) -> SheetApp {
    let mut session = Session::new();
    session.new_workbook();
    let mut app = SheetApp::new(session, Default::default());
    app.ui.language = language;
    app
}

fn value(app: &SheetApp, a: &str) -> Value {
    app.session.active().unwrap().wb.active().unwrap().value(CellRef::parse(a).unwrap())
}

fn enter(app: &mut SheetApp, cell: &str, text: &str) {
    app.run("selection.set", json!({"range": cell})).unwrap();
    app.begin_edit(Some(text.into()), false);
    assert!(app.commit_edit(0, 0, false, false));
}

#[test]
fn language_maps_to_the_number_locale() {
    assert_eq!(Language::En.num_locale(), NumLocale::EnUs);
    assert_eq!(Language::Es.num_locale(), NumLocale::EnUs);
    assert_eq!(Language::Ru.num_locale(), NumLocale::Ru);
}

#[test]
fn russian_typing_is_read_in_russian() {
    let mut app = app(Language::Ru);
    enter(&mut app, "A1", "12.10.2026");
    enter(&mut app, "A2", "3,5");
    enter(&mut app, "A3", "1 234,5");
    enter(&mut app, "A4", "1.234");
    enter(&mut app, "A5", "25,5%");
    enter(&mut app, "A6", "12 окт 2026");
    enter(&mut app, "A7", "10.13.2026");
    assert_eq!(value(&app, "A1"), Value::Number(46307.0), "12 October, not 10 December");
    assert_eq!(value(&app, "A2"), Value::Number(3.5));
    assert_eq!(value(&app, "A3"), Value::Number(1234.5));
    assert_eq!(value(&app, "A4"), Value::text("1.234"));
    assert_eq!(value(&app, "A5"), Value::Number(0.255));
    assert_eq!(value(&app, "A6"), Value::Number(46307.0));
    assert_eq!(value(&app, "A7"), Value::text("10.13.2026"));
    // engine commands stay en-US whatever the language
    app.run("cell.set", json!({"cell": "B1", "input": "10/12/2026"})).unwrap();
    app.run("cell.set", json!({"cell": "B2", "input": "3.5"})).unwrap();
    assert_eq!(value(&app, "B1"), Value::Number(46307.0));
    assert_eq!(value(&app, "B2"), Value::Number(3.5));
}

#[test]
fn english_typing_is_unchanged() {
    let mut app = app(Language::En);
    enter(&mut app, "A1", "12.10.2026");
    enter(&mut app, "A2", "3,5");
    enter(&mut app, "A3", "10/12/2026");
    assert_eq!(value(&app, "A1"), Value::Number(46366.0), "en-US still reads D.M.Y as M.D.Y");
    assert_eq!(value(&app, "A2"), Value::text("3,5"));
    assert_eq!(value(&app, "A3"), Value::Number(46307.0));
}

#[test]
fn opening_a_cell_for_edit_and_confirming_keeps_the_value() {
    for language in [Language::Ru, Language::En] {
        let mut app = app(language);
        for (cell, input) in [("A1", "10/12/2026"), ("A2", "3500000.5"), ("A3", "0.255"), ("A4", "46307.75"), ("A5", "hello"), ("A6", "1.234E+20")] {
            app.run("cell.set", json!({"cell": cell, "input": input})).unwrap();
        }
        app.run("home.formatCells", json!({"range": "A4", "style": {"num_fmt": "dd.mm.yyyy hh:mm"}})).unwrap();
        app.run("home.formatCells", json!({"range": "A3", "style": {"num_fmt": "0.0%"}})).unwrap();
        for cell in ["A1", "A2", "A3", "A4", "A5", "A6"] {
            let before = value(&app, cell);
            app.run("selection.set", json!({"range": cell})).unwrap();
            app.begin_edit(None, false);
            assert!(app.commit_edit(0, 0, false, false));
            assert_eq!(value(&app, cell), before, "{cell} in {language:?}");
        }
    }
}

#[test]
fn the_editor_shows_the_locale_form() {
    let mut app = app(Language::Ru);
    app.run("cell.set", json!({"cell": "A1", "input": "10/12/2026"})).unwrap();
    app.run("cell.set", json!({"cell": "A2", "input": "3500000.5"})).unwrap();
    app.run("selection.set", json!({"range": "A1"})).unwrap();
    app.begin_edit(None, false);
    assert_eq!(app.editor.as_ref().unwrap().text, "12.10.2026");
    app.cancel_edit();
    app.run("selection.set", json!({"range": "A2"})).unwrap();
    app.begin_edit(None, false);
    assert_eq!(app.editor.as_ref().unwrap().text, "3500000,5");
}

#[test]
fn a_text_formatted_cell_keeps_what_was_typed() {
    let mut app = app(Language::Ru);
    app.run("home.formatCells", json!({"range": "A1", "style": {"num_fmt": "@"}})).unwrap();
    enter(&mut app, "A1", "12.10.2026");
    assert_eq!(value(&app, "A1"), Value::text("12.10.2026"));
}
