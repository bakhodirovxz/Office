// Added by the office toolbar patch queue. MIT OR Apache-2.0.
//! Tests of the simplified Slides toolbar (`office_toolbar`): the command ids still exist at this upstream, the row shows what
//! the design lists, the controls act, and the switch between the row and the full ribbon works.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use deckcraft_engine::Session;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;

use crate::office_toolbar as tb;
use crate::{Services, SlideApp};

/// A window in the given toolbar mode. `OFFICE_TOOLBAR_PNG_DIR` set: also writes `toolbar-<mode>-<lang>.png` there.
fn window(simple: bool, size: egui::Vec2, language: &str) -> Harness<'static, SlideApp> {
    let mut app = SlideApp::new(Session::with_new(), Services::default());
    app.ui.language = language.into();
    app.session.execute("slide.layout", &json!({"layout": "blank"})).ok();
    let mut h = Harness::builder().with_size(size).build_ui_state(
        |ui, app: &mut SlideApp| {
            app.logic(ui.ctx());
            app.ui(ui);
        },
        app,
    );
    tb::set_simple(&h.ctx, simple);
    h.run_steps(4);
    h
}

fn png(h: &mut Harness<'_, SlideApp>, name: &str) {
    if let (Ok(dir), Ok(img)) = (std::env::var("OFFICE_TOOLBAR_PNG_DIR"), h.render()) {
        let _ = std::fs::create_dir_all(&dir);
        let _ = img.save(std::path::Path::new(&dir).join(name));
    }
}

#[test]
fn office_toolbar_ids_exist() {
    let source = include_str!("office_toolbar.rs");
    for id in tb::IDS {
        assert!(crate::ribbon::known(id), "{id} is not a command at this upstream");
        assert!(source.contains(&format!("\"{id}\"")), "{id} is listed but the row never uses it");
    }
    // Ours are not registry commands: they must stay out of it, or the ids would mean two things.
    for id in tb::OURS {
        assert!(!crate::ribbon::known(id), "{id} is now an upstream command");
    }
}

/// Every icon the row names has its masks in `office_icons.bin`, and the file holds nothing else.
#[test]
fn the_icon_file_matches_the_list() {
    for name in tb::ICONS {
        let masks = tb::masks(name).unwrap_or_else(|| panic!("`{name}` is not in office_icons.bin"));
        assert!(masks[0].iter().any(|a| *a > 128), "`{name}` is empty");
    }
    let expected: usize = tb::ICONS.iter().map(|n| n.len() + 1 + tb::SIZES.iter().map(|s| s * s).sum::<usize>()).sum();
    assert_eq!(tb::BIN.len(), expected);
}

#[test]
fn the_simple_row_has_the_designed_controls_and_no_ribbon() {
    let h = window(true, egui::vec2(1500.0, 700.0), "en");
    for label in [
        "New Slide",
        "Layout",
        "Bold",
        "Italic",
        "Underline",
        "Font Color",
        "Bullets",
        "Numbering",
        "Align Left",
        "Center",
        "Align Right",
        "Text Box",
        "Picture",
        "Shapes",
        "Table",
        "Transliterate",
        "Version History",
        "Full ribbon",
    ] {
        assert!(h.query_by_label(label).is_some(), "missing {label}");
    }
    // The upstream ribbon's File tab is not drawn.
    assert!(h.query_by_label("File").is_none());
    // The split buttons' arrows are controls of their own and carry a name too.
    for label in ["New Slide \u{25be}", "Font Color \u{25be}"] {
        assert!(h.query_by_label(label).is_some(), "missing {label}");
    }
}

#[test]
fn a_toggle_acts_on_the_selection() {
    let mut h = window(true, egui::vec2(1500.0, 700.0), "en");
    let id = h.state_mut().session.execute("shape.insert", &json!({"preset": "rect", "rect": [100, 100, 500, 120], "text": "Salom"})).unwrap()["id"]
        .as_u64()
        .unwrap();
    h.state_mut().session.execute("edit.select", &json!({"ids": [id]})).unwrap();
    h.run_steps(3);
    h.get_by_label("Bold").click();
    h.run_steps(3);
    let st = h.state_mut().session.execute("format.state", &json!({})).unwrap();
    assert_eq!(st["bold"], true, "{st}");
}

#[test]
fn the_two_buttons_switch_between_the_row_and_the_ribbon() {
    let mut h = window(true, egui::vec2(1500.0, 700.0), "en");
    h.get_by_label("Full ribbon").click();
    h.run_steps(4);
    assert!(h.query_by_label("File").is_some(), "the upstream ribbon is back");
    assert!(h.query_by_label("Simple ribbon").is_some());
    h.get_by_label("Simple ribbon").click();
    h.run_steps(4);
    assert!(h.query_by_label("File").is_none(), "the row is back");
}

/// The pill sits right of the last tab, contextual ones included, at 1024 px.
#[test]
fn the_pill_does_not_cover_a_tab_at_1024() {
    let mut h = window(false, egui::vec2(1024.0, 700.0), "en");
    let id = h.state_mut().session.execute("shape.insert", &json!({"preset": "rect", "rect": [100, 100, 500, 120], "text": "Salom"})).unwrap()["id"]
        .as_u64()
        .unwrap();
    h.state_mut().session.execute("edit.select", &json!({"ids": [id]})).unwrap();
    h.run_steps(4);
    let pill = h.get_by_label("Simple ribbon").rect();
    let font = crate::theme::font(13.0);
    let tabs = ["File", "Home", "Insert", "Draw", "Design", "Transitions", "Animations", "Slide Show", "Record", "Review", "View", "Shape Format"];
    let ink = crate::theme::Tokens::get(&h.ctx).text;
    let end = h.ctx.fonts_mut(|f| tabs.iter().fold(12.0, |x, name| x + f.layout_no_wrap(name.to_string(), font.clone(), ink).size().x + 18.0 + 2.0));
    assert!(pill.min.x >= end, "pill starts at {} but the tabs end at {end}", pill.min.x);
}

#[test]
fn evidence_screenshots() {
    for (simple, mode) in [(true, "simple"), (false, "full")] {
        let mut h = window(simple, egui::vec2(1366.0, 760.0), "uz-latn");
        let id =
            h.state_mut().session.execute("shape.insert", &json!({"preset": "rect", "rect": [100, 100, 500, 120], "text": "Salom"})).unwrap()["id"]
                .as_u64()
                .unwrap();
        h.state_mut().session.execute("edit.select", &json!({"ids": [id]})).unwrap();
        h.run_steps(6);
        png(&mut h, &format!("slides-{mode}-uz-latn-1366.png"));
    }
}

/// A policy that refuses printing greys it out in the code path the Print page uses.
#[test]
fn hook_refuses_print() {
    struct NoPrint;
    impl deckcraft_engine::office_hook::CommandHook for NoPrint {
        fn before(&self, id: &str, _: &Session) -> Option<String> {
            (id == "file.print").then(|| "Printing is off".to_string())
        }
    }
    let mut app = SlideApp::new(Session::with_new(), Services::default());
    deckcraft_engine::office_hook::set_hook(Some(std::sync::Arc::new(NoPrint)));
    let printed = app.print_pdf(json!({}));
    deckcraft_engine::office_hook::set_hook(None);
    assert!(!printed);
}
