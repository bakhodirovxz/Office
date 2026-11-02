use std::collections::HashSet;

use egui::{Color32, Pos2, Rect, vec2};
use serde_json::json;
use wordcraft_engine::Session;

use super::*;

const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

fn session() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    s.run("text.insert", &json!({"text": "Salom dunyo"})).unwrap();
    s
}

#[test]
fn the_mode_switch_needs_a_narrow_window_and_a_finger() {
    assert!(decide(None, 360.0, true));
    assert!(decide(None, MAX_WIDTH, true));
    assert!(!decide(None, MAX_WIDTH + 1.0, true), "a wide window keeps the desktop layout");
    assert!(!decide(None, 360.0, false), "a narrow desktop window with a mouse keeps it too");
    assert!(decide(Some(true), 1280.0, false), "forced on");
    assert!(!decide(Some(false), 360.0, true), "forced off");
}

#[test]
fn the_contextual_set_follows_the_selection() {
    let mut s = session();
    assert_eq!(set_of(&s), Set::Caret);
    s.run("select.all", &json!({})).unwrap();
    assert_eq!(set_of(&s), Set::Selection);
    s.run("select.collapse", &json!({"end": true})).unwrap();
    s.run("insert.table", &json!({"rows": 2, "cols": 2})).unwrap();
    assert_eq!(set_of(&s), Set::Table, "the caret is in a cell");
    s.run("table.deleteTable", &json!({})).unwrap();
    s.run("insert.picture", &json!({"data": PNG, "width": 50})).unwrap();
    assert_eq!(set_of(&s), Set::Picture);
    s.run("select.collapse", &json!({"end": true})).unwrap();
    s.run("text.insert", &json!({"text": "x"})).unwrap();
    assert_eq!(set_of(&s), Set::Caret, "typing left the picture");
}

#[test]
fn the_toolbar_has_at_most_five_slots_of_48_dp_at_320() {
    for set in [Set::Caret, Set::Selection, Set::Table, Set::Picture] {
        let n = slots(set).len();
        assert!((3..=5).contains(&n), "{set:?}: {n} slots");
        assert!(320.0 / n as f32 >= ROW, "{set:?}: {n} slots are narrower than {ROW} at 320 dp");
    }
    let names = |set| slots(set).iter().map(|s| s.label).collect::<Vec<_>>();
    assert_eq!(names(Set::Selection)[..4], ["Bold", "Italic", "Underline", "Highlight"]);
    assert_eq!(names(Set::Table)[..2], ["Row", "Column"]);
    assert_eq!(names(Set::Picture)[..2], ["Wrap Text", "Size"]);
}

const SHEETS: [Sheet; 8] = [Sheet::Menu, Sheet::Format, Sheet::List, Sheet::Insert, Sheet::Table, Sheet::Delete, Sheet::Wrap, Sheet::Picture];

fn every_control(s: &Session) -> Vec<(String, String, String)> {
    let mut v: Vec<(String, String, String)> = Vec::new();
    for set in [Set::Caret, Set::Selection, Set::Table, Set::Picture] {
        for sl in slots(set) {
            v.push((sl.label.into(), match sl.act { Act::Cmd(c, _) => c.into(), _ => String::new() }, match sl.act { Act::Cmd(_, p) => p.into(), _ => "{}".into() }));
        }
    }
    for sh in SHEETS {
        for it in rows(sh, s).into_iter().flatten() {
            v.push((it.label.into(), it.cmd.into(), it.params.into()));
        }
    }
    v
}

#[test]
fn every_control_runs_a_registered_command_with_valid_parameters() {
    let s = session();
    for (label, cmd, params) in every_control(&s) {
        assert!(serde_json::from_str::<serde_json::Value>(&params).is_ok(), "{label}: bad parameters {params}");
        if cmd.is_empty() || cmd == DESKTOP || cmd.starts_with("ui.") {
            continue;
        }
        assert!(s.registry.get(&cmd).is_some(), "{label}: `{cmd}` is not registered");
    }
}

#[test]
fn formatting_from_the_sheet_changes_the_selection() {
    let mut s = session();
    s.run("select.all", &json!({})).unwrap();
    for (cmd, params, key) in [("format.bold", "{}", "bold"), ("format.italic", "{}", "italic")] {
        s.run(cmd, &serde_json::from_str(params).unwrap()).unwrap();
        assert_eq!(state(&mut s)[key], json!(true), "{cmd}");
    }
    assert!(rows(Sheet::Format, &s).iter().flatten().any(|i| i.flag == "bold" && i.keep), "the sheet shows bold pressed and stays open");
}

/// Every label has a row in the catalog with Uzbek (Latin, Cyrillic) and Russian, so the phone UI is never English-only.
#[test]
fn every_label_is_in_the_catalog_in_all_three_languages() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../i18n");
    let mut have: HashSet<String> = HashSet::new();
    for f in ["wordcraft.tsv", "shared.tsv"] {
        let text = std::fs::read_to_string(dir.join(f)).unwrap();
        for line in text.lines().skip(1) {
            let c: Vec<&str> = line.split('\t').collect();
            if c.len() > 11 && !c[2].is_empty() && !c[3].is_empty() && !c[5].is_empty() && !c[7].is_empty() && c[11].contains("wordcraft") {
                have.insert(c[2].to_string());
            }
        }
    }
    let s = session();
    let mut missing: Vec<String> = every_control(&s).into_iter().map(|c| c.0).filter(|l| l != SIZE_LABEL && l.chars().any(char::is_alphabetic) && !have.contains(l)).collect();
    missing.sort();
    missing.dedup();
    assert!(missing.is_empty(), "no catalog row (en, uz-Latn, uz-Cyrl, ru, apps wordcraft): {missing:?}");
}

fn colors() -> [Color32; color::COUNT] {
    [Color32::WHITE, Color32::GRAY, Color32::BLACK, Color32::DARK_GRAY, Color32::BLUE, Color32::WHITE, Color32::LIGHT_GRAY, Color32::WHITE]
}

/// Draw one frame at `w` x 640 and return the commands the chrome asked for.
fn frame(s: &mut Session, ctx: &egui::Context, w: f32, sheet: Option<Sheet>, title: &str) -> Vec<(String, serde_json::Value)> {
    set_sheet(ctx, sheet);
    let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(w, 640.0))), ..Default::default() };
    let mut cmds = Vec::new();
    let icon = |_: &Painter, _: Rect, _: &str, _: Color32| {};
    let t = |k: &str| k.to_string();
    let mut out = ctx.run_ui(input, |ui| {
        cmds = chrome(s, ui, title, &colors(), &t, &icon);
        egui::CentralPanel::default().show(ui, |_| {});
    });
    out.textures_delta.clear();
    cmds
}

/// 360 and 412 dp: nothing is wider than the window, the top row and the bottom toolbar are 48 dp, and a very long file name is cut, not overlapped.
#[test]
fn the_chrome_draws_at_360_and_412_without_overflow() {
    let ctx = egui::Context::default();
    let mut s = session();
    for w in [360.0, 412.0] {
        for sheet in [None, Some(Sheet::Menu), Some(Sheet::Format), Some(Sheet::Wrap)] {
            let cmds = frame(&mut s, &ctx, w, sheet, &"A very long file name that does not fit in a phone's top row at all.docx".repeat(3));
            assert!(cmds.is_empty());
            let used = ctx.globally_used_rect();
            assert!(used.max.x <= w + 0.5, "{w} dp: content reaches {}", used.max.x);
        }
    }
}

fn run_frame(s: &mut Session, ctx: &egui::Context, events: Vec<egui::Event>, time: f64) -> Vec<(String, serde_json::Value)> {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(360.0, 640.0))),
        events,
        time: Some(time),
        ..Default::default()
    };
    let mut cmds = Vec::new();
    let icon = |_: &Painter, _: Rect, _: &str, _: Color32| {};
    let t = |k: &str| k.to_string();
    let mut out = ctx.run_ui(input, |ui| {
        let _ = active(ui.ctx());
        cmds = chrome(s, ui, "x.docx", &colors(), &t, &icon);
        egui::CentralPanel::default().show(ui, |_| {});
    });
    out.textures_delta.clear();
    cmds
}

/// A tap (touch start, press, release, touch end, in the frames a phone sends them) on a slot opens its sheet; a second tap closes it.
#[test]
fn a_tap_on_a_slot_opens_and_closes_its_sheet() {
    use egui::{Event, PointerButton, TouchDeviceId, TouchPhase};
    let ctx = egui::Context::default();
    let mut s = session();
    let at = pos2(36.0, 640.0 - 20.0);
    let touch = |phase| Event::Touch { device_id: TouchDeviceId(0), id: egui::TouchId(0), phase, pos: at, force: None };
    let button = |pressed| Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Default::default() };
    let mut time = 1.0;
    for tap in 0..2 {
        for frame in [vec![Event::PointerMoved(at)], vec![touch(TouchPhase::Start), button(true)], vec![button(false)], vec![touch(TouchPhase::End)], vec![]] {
            time += 0.05;
            let _ = run_frame(&mut s, &ctx, frame, time);
        }
        let open = open_sheet(&ctx);
        assert_eq!(open, if tap == 0 { Some(Sheet::Format) } else { None }, "after tap {tap}");
    }
}

/// A slow frame batches the whole tap (move, touch start, press, release, touch end) into one frame.
#[test]
fn a_tap_batched_into_one_frame_still_opens_the_sheet() {
    use egui::{Event, PointerButton, TouchDeviceId, TouchPhase};
    let ctx = egui::Context::default();
    let mut s = session();
    let at = pos2(36.0, 640.0 - 20.0);
    let touch = |phase| Event::Touch { device_id: TouchDeviceId(0), id: egui::TouchId(0), phase, pos: at, force: None };
    let button = |pressed| Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Default::default() };
    let _ = run_frame(&mut s, &ctx, vec![], 1.0);
    let _ = run_frame(&mut s, &ctx, vec![touch(TouchPhase::Start), button(true), button(false), touch(TouchPhase::End), Event::PointerGone], 1.1);
    let _ = run_frame(&mut s, &ctx, vec![], 1.2);
    assert_eq!(open_sheet(&ctx), Some(Sheet::Format));
}

/// eframe sends both the pointer events and the touch events of one tap: every button event twice.
#[test]
fn a_tap_with_doubled_events_still_opens_the_sheet() {
    use egui::{Event, PointerButton, TouchDeviceId, TouchPhase};
    let at = pos2(36.0, 640.0 - 20.0);
    let touch = |phase| Event::Touch { device_id: TouchDeviceId(0), id: egui::TouchId(0), phase, pos: at, force: None };
    let button = |pressed| Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Default::default() };
    for split in [false, true] {
        let ctx = egui::Context::default();
        let mut s = session();
        let _ = run_frame(&mut s, &ctx, vec![], 1.0);
        let down = vec![button(true), touch(TouchPhase::Start), button(true)];
        let up = vec![button(false), touch(TouchPhase::End), button(false), Event::PointerGone];
        if split {
            let _ = run_frame(&mut s, &ctx, down, 1.1);
            let _ = run_frame(&mut s, &ctx, up, 1.2);
        } else {
            let _ = run_frame(&mut s, &ctx, [down, up].concat(), 1.1);
        }
        let _ = run_frame(&mut s, &ctx, vec![], 1.3);
        assert_eq!(open_sheet(&ctx), Some(Sheet::Format), "split {split}");
    }
}

fn tap_at(s: &mut Session, ctx: &egui::Context, at: Pos2, time: &mut f64) -> Vec<(String, serde_json::Value)> {
    use egui::{Event, PointerButton, TouchDeviceId, TouchPhase};
    let touch = |phase| Event::Touch { device_id: TouchDeviceId(0), id: egui::TouchId(0), phase, pos: at, force: None };
    let button = |pressed| Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Default::default() };
    let mut cmds = Vec::new();
    for frame in [vec![touch(TouchPhase::Start), button(true)], vec![], vec![button(false), touch(TouchPhase::End)], vec![]] {
        *time += 0.05;
        cmds.extend(run_frame(s, ctx, frame, *time));
    }
    cmds
}

/// A tap on a row of an open sheet runs its command (and closes the sheet unless the row repeats); the menu's Save As is the second row.
#[test]
fn a_tap_on_a_sheet_row_runs_its_command() {
    let ctx = egui::Context::default();
    let mut s = session();
    let mut time = 1.0;
    let _ = run_frame(&mut s, &ctx, vec![], time);
    set_sheet(&ctx, Some(Sheet::Menu));
    let _ = run_frame(&mut s, &ctx, vec![], 1.01);
    // 640 high: the sheet (5 rows, 248) sits above the 48 bar, its first row at 346
    let at = pos2(180.0, 346.0 + 48.0 + 24.0);
    let cmds = tap_at(&mut s, &ctx, at, &mut time);
    assert_eq!(cmds.iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), ["file.saveAs"], "{cmds:?}");
    assert_eq!(open_sheet(&ctx), None);
}

/// A finger is also reported as pointer events in frames of their own: one right after a touch must not turn the phone layout off; a mouse later does.
#[test]
fn a_pointer_event_after_a_touch_keeps_the_phone_layout_until_the_grace_is_over() {
    use egui::{Event, PointerButton, TouchDeviceId, TouchPhase};
    set_force(None);
    let ctx = egui::Context::default();
    let at = pos2(10.0, 10.0);
    let touch = Event::Touch { device_id: TouchDeviceId(0), id: egui::TouchId(0), phase: TouchPhase::Start, pos: at, force: None };
    let button = Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Default::default() };
    let on = |events: Vec<Event>, time: f64| {
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(360.0, 640.0))), events, time: Some(time), ..Default::default() };
        let mut r = false;
        let mut out = ctx.run_ui(input, |ui| r = active(ui.ctx()));
        out.textures_delta.clear();
        r
    };
    assert!(on(vec![touch], 10.0), "a touch in a narrow window");
    assert!(on(vec![button.clone()], 10.2), "its pointer event, a frame later");
    assert!(!on(vec![button], 12.0), "a mouse two seconds later");
}
