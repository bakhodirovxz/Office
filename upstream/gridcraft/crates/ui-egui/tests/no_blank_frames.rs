//! office overlay (patches/gridcraft/0156): while new fonts activate, `SheetApp::ui` draws nothing and asks egui to redo
//! the pass, so no frame reaches the screen empty. An empty frame shows eframe's near-black clear colour: the window went
//! dark at start and for a frame after formatting that changed the workbook's fonts (owner, Phase 4 MSI test).

use gridcraft_engine::Session;
use gridcraft_ui_egui::SheetApp;
use serde_json::json;

/// An empty frame has at most the test harness's own background shape; the interface is hundreds.
const EMPTY: usize = 10;

fn new_app() -> SheetApp {
    // The font catalogue is ready, so `logic` binds the workbook's fonts (the rebind path) from the first frame on.
    gridcraft_ui_egui::fonts::catalog();
    let mut session = Session::new();
    session.new_workbook();
    SheetApp::new(session, Default::default())
}

#[test]
fn the_first_frames_paint_the_interface() {
    // eframe's frames, from the very first one (a kittest harness runs frames before a test sees them).
    let ctx = egui::Context::default();
    let mut app = new_app();
    for frame in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0))),
            time: Some(f64::from(frame) / 60.0),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            app.logic(ui.ctx());
            app.ui(ui);
        });
        out.textures_delta.clear(); // no renderer here
        assert!(out.shapes.len() > EMPTY, "frame {frame}: {} shapes, the window would show the clear colour", out.shapes.len());
    }
}

#[test]
fn no_frame_is_presented_empty_while_fonts_change() {
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1200.0, 800.0)).build_ui_state(
        |ui, app: &mut SheetApp| {
            app.logic(ui.ctx());
            app.ui(ui);
        },
        new_app(),
    );
    let painted = |h: &egui_kittest::Harness<'static, SheetApp>| h.output().shapes.len();
    // Each new style adds a font name to the workbook's list, which rebinds the font set.
    for name in ["Georgia", "Courier New", "Probe Font", "Georgia"] {
        h.state_mut().run("home.fontName", json!({ "name": name })).unwrap();
        for step in 0..3 {
            h.step();
            assert!(painted(&h) > EMPTY, "after font {name}, frame {step}: {} shapes", painted(&h));
        }
    }
}
