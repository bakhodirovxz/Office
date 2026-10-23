// Added by the office overlay patch queue. MIT OR Apache-2.0.
//! The simplified Sheets toolbar (patch 0140; docs/phase4/toolbar-design.md section 4): every command id is still registered,
//! the row runs commands, Excel's keytip sequences reach the five picker controls, the layout follows the window width, and
//! the toggle switches and remembers the mode. The mode is process-wide, so the tests take a lock.

use std::sync::{Mutex, MutexGuard};

use egui::{Event, Key, Modifiers, os::OperatingSystem};
use egui_kittest::kittest::Queryable as _;
use gridcraft_engine::Session;
use gridcraft_engine::core::CellRef;
use gridcraft_ui_egui::i18n::Language;
use gridcraft_ui_egui::keytips::keytip_table;
use gridcraft_ui_egui::{SheetApp, office_toolbar};

type Harness = egui_kittest::Harness<'static, SheetApp>;

static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn settings_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("office-toolbar-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("settings dir");
    office_toolbar::use_settings_dir(dir.clone());
    dir
}

fn harness(width: f32, language: Language) -> Harness {
    let mut s = Session::new();
    s.new_workbook();
    let mut app = SheetApp::new(s, Default::default());
    app.ui.language = language;
    let mut h = egui_kittest::Harness::builder().with_os(OperatingSystem::Nix).with_size(egui::vec2(width, 800.0)).build_ui_state(
        |ui, app: &mut SheetApp| {
            app.logic(ui.ctx());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    h
}

fn key_event(key: Key, pressed: bool) -> Event {
    Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE }
}

fn key(h: &mut Harness, k: Key) {
    h.input_mut().events.extend([key_event(k, true), key_event(k, false)]);
    h.run_steps(2);
}

fn sequence(h: &mut Harness, letters: &[Key]) {
    key(h, Key::F10);
    for &k in letters {
        h.input_mut().events.extend([key_event(k, true), Event::Text(k.name().to_ascii_lowercase()), key_event(k, false)]);
        h.run_steps(2);
    }
    h.run_steps(3);
}

fn style_of(h: &Harness, cell: &str) -> gridcraft_engine::model::Style {
    let d = h.state().session.active().expect("doc");
    let sh = d.wb.active().expect("sheet");
    d.wb.styles.get(sh.style_id(CellRef::parse(cell).expect("cell"))).clone()
}

/// The check that makes a sync fail CI instead of conflicting: every id the row runs is a registered command, every icon is
/// embedded, and the five picker controls still have keytip sequences. Ids that are ours (`office.*`) are WP10's.
#[test]
fn office_toolbar_ids_exist() {
    let ids: Vec<&str> = gridcraft_engine::command_specs().iter().map(|c| c.id).collect();
    let mut checked = 0;
    for id in office_toolbar::command_ids() {
        if id.starts_with("office.") {
            continue;
        }
        assert!(ids.contains(&id), "the toolbar runs `{id}`, which is not registered any more");
        checked += 1;
    }
    assert_eq!(checked, 32, "the design lists 32 upstream ids (toolbar-design.md A.2); update both together");
    for name in office_toolbar::icon_names() {
        assert!(office_toolbar::has_icon(name), "icon `{name}` is not embedded");
    }
    for id in ["home.fontName", "home.fontSize", "home.fillColor", "home.fontColor", "home.numberFormat"] {
        assert!(!keytip_table::sequences_for(id).is_empty(), "no keytip sequence reaches `{id}`");
    }
}

#[test]
fn the_row_replaces_the_ribbon_and_runs_commands() {
    let _g = lock();
    settings_dir("run");
    office_toolbar::set_simple(true);
    let mut h = harness(1600.0, Language::En);
    assert!(h.query_all_by_label("Insert").count() >= 1, "the row has the Insert menu");
    assert_eq!(h.query_all_by_label("Draw").count(), 0, "no ribbon tab");
    h.get_by_label("Bold").click();
    h.run_steps(3);
    assert!(style_of(&h, "A1").font.bold, "Bold in the row bolds the cell");
    h.get_by_label("Center").click();
    h.run_steps(3);
    assert_eq!(style_of(&h, "A1").align.h, gridcraft_engine::model::HAlign::Center);
    h.get_by_label("Show Formulas").click();
    h.run_steps(3);
    assert!(h.state().session.active().expect("doc").wb.active().expect("sheet").show_formulas);
    assert!(h.state().message.is_none() && h.state().toast.is_none());
}

#[test]
fn excels_keytips_reach_the_five_pickers() {
    let _g = lock();
    settings_dir("keys");
    office_toolbar::set_simple(true);
    // Alt,H,F,F: the font list opens ("Calibri" is on the picker button and in the list)
    let mut h = harness(1600.0, Language::En);
    sequence(&mut h, &[Key::H, Key::F, Key::F]);
    assert!(h.query_all_by_label("Calibri").count() >= 2, "HFF must open the font list in the simple row");
    // Alt,H,F,S: the size picker takes a typed size
    let mut h = harness(1600.0, Language::En);
    sequence(&mut h, &[Key::H, Key::F, Key::S]);
    h.input_mut().events.push(Event::Text("13.5".into()));
    h.run_steps(2);
    key(&mut h, Key::Enter);
    h.run_steps(3);
    assert_eq!(style_of(&h, "A1").font.size, 13.5);
    // Alt,H,H: the fill palette opens, an arrow walk and Enter apply the highlighted swatch
    let mut h = harness(1600.0, Language::En);
    sequence(&mut h, &[Key::H, Key::H]);
    key(&mut h, Key::ArrowDown);
    key(&mut h, Key::ArrowRight);
    key(&mut h, Key::Enter);
    h.run_steps(3);
    assert_eq!(h.state().grid.last_fill, "#000000", "HH must reach the fill palette");
    // Alt,H,F,C: the font colour palette
    let mut h = harness(1600.0, Language::En);
    sequence(&mut h, &[Key::H, Key::F, Key::C]);
    key(&mut h, Key::ArrowDown);
    key(&mut h, Key::ArrowRight);
    key(&mut h, Key::Enter);
    h.run_steps(3);
    assert_eq!(h.state().grid.last_font_color, "#000000", "HFC must reach the font colour palette");
    // Alt,H,N: the number list opens with the short list of the design
    let mut h = harness(1600.0, Language::En);
    sequence(&mut h, &[Key::H, Key::N]);
    assert!(h.query_all_by_label_contains("Percentage").count() >= 1, "HN must open the number list");
    assert_eq!(h.query_all_by_label_contains("Scientific").count(), 0, "the simple picker lists the short set only");
    assert!(h.state().message.is_none() && h.state().toast.is_none(), "no error box");
}

/// Sweeps the window width and prints where the labels drop and where the "..." menu starts, per language.
/// Measured, not estimated (PLAN WP3): `cargo test -p gridcraft-ui-egui --test office_toolbar widths -- --nocapture`.
#[test]
fn widths_follow_the_text_and_the_toggle_never_overflows() {
    let _g = lock();
    settings_dir("width");
    office_toolbar::set_simple(true);
    let langs: Vec<(&str, Language)> = vec![
        ("en", Language::En),
        ("ru", Language::Ru),
        #[cfg(office_uz)]
        ("uz-Latn", Language::UzLatn),
        #[cfg(office_uz)]
        ("uz-Cyrl", Language::UzCyrl),
    ];
    for (name, lang) in langs {
        let mut labels_from = None;
        let mut all_from = None;
        let mut prev: Option<(bool, usize)> = None;
        for w in (600..=2000).step_by(20) {
            let h = harness(w as f32, lang);
            let plan = office_toolbar::last_plan().expect("a plan");
            assert!(h.query_by_label(lang.tr("Full ribbon")).is_some(), "{name} {w}: the toggle is always there");
            if plan.shown < plan.total {
                assert!(h.query_by_label(lang.tr("More commands")).is_some(), "{name} {w}: items moved to the menu but there is no menu button");
            }
            if let Some((l, s)) = prev {
                assert!(plan.shown >= s && (plan.labels || !l), "{name} {w}: the row must only grow with the window");
            }
            prev = Some((plan.labels, plan.shown));
            if plan.labels && labels_from.is_none() {
                labels_from = Some(w);
            }
            if plan.shown == plan.total && all_from.is_none() {
                all_from = Some(w);
            }
        }
        println!("WIDTHS {name}: labels from {labels_from:?} px, every item in the row from {all_from:?} px");
        assert!(labels_from.is_some() && all_from.is_some(), "{name}: the row fits at 2000 px");
    }
}

#[test]
fn the_toggle_switches_and_is_remembered() {
    let _g = lock();
    let dir = settings_dir("toggle");
    office_toolbar::set_simple(true);
    let mut h = harness(1600.0, Language::En);
    h.get_by_label("Full ribbon").click();
    h.run_steps(4);
    assert!(!office_toolbar::simple());
    let text = std::fs::read_to_string(dir.join("settings.toml")).expect("settings.toml written");
    assert!(text.contains("toolbar = \"full\""), "{text}");
    // full mode shows the pill, and it goes back
    h.get_by_label("Simple ribbon").click();
    h.run_steps(4);
    assert!(office_toolbar::simple());
    assert!(std::fs::read_to_string(dir.join("settings.toml")).expect("settings.toml").contains("toolbar = \"simple\""));
}

/// Evidence for docs/phase4/evidence/toolbar: `OFFICE_SHOT_DIR=<dir> cargo test -p gridcraft-ui-egui --test office_toolbar evidence -- --ignored`.
/// Simple and full mode in every language the build has, a narrow window, and the full ribbon at 1024 px with a contextual tab.
#[test]
#[ignore]
fn evidence_screenshots() {
    let _g = lock();
    settings_dir("shots");
    let dir = std::path::PathBuf::from(std::env::var("OFFICE_SHOT_DIR").expect("OFFICE_SHOT_DIR"));
    std::fs::create_dir_all(&dir).expect("dir");
    let langs: Vec<(&str, Language)> = vec![
        ("en", Language::En),
        ("ru", Language::Ru),
        #[cfg(office_uz)]
        ("uz-Latn", Language::UzLatn),
        #[cfg(office_uz)]
        ("uz-Cyrl", Language::UzCyrl),
    ];
    for (name, lang) in langs {
        for (mode, simple) in [("simple", true), ("full", false)] {
            office_toolbar::set_simple(simple);
            let mut h = harness(1900.0, lang);
            h.run_steps(3);
            h.render().expect("render").save(dir.join(format!("sheets-{mode}-{name}.png"))).expect("save");
        }
    }
    office_toolbar::set_simple(true);
    for w in [1400.0, 1000.0] {
        let mut h = harness(w, Language::En);
        h.run_steps(3);
        h.render().expect("render").save(dir.join(format!("sheets-simple-en-{w}px.png"))).expect("save");
    }
    // the pill against a contextual tab at 1024 px: a table selection adds "Table Design"
    office_toolbar::set_simple(false);
    let mut h = harness(1024.0, Language::En);
    h.state_mut().run("cell.set", serde_json::json!({"cell": "A1", "input": "x"})).expect("cell");
    let _ = h.state_mut().run("home.formatAsTable", serde_json::json!({"range": "A1:B3"}));
    h.run_steps(4);
    h.render().expect("render").save(dir.join("sheets-full-en-1024px-contextual.png")).expect("save");
}
