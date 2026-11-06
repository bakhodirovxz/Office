//! office overlay (patches/wordcraft/0140): the frame the window opens with paints the interface. While the first fonts
//! activate, `WordApp::ui` draws nothing and asks egui to redo the pass; it used to present two empty frames, which show
//! eframe's near-black clear colour until the first page was laid out (owner, Phase 4 MSI test: the window goes dark).

use wordcraft_engine::Session;
use wordcraft_ui_egui::{Services, WordApp};

/// An empty frame has no shapes; the interface is hundreds.
const EMPTY: usize = 10;

#[test]
fn the_first_frames_paint_the_interface() {
    // eframe's frames, from the very first one (a kittest harness runs frames before a test sees them).
    let ctx = egui::Context::default();
    let mut app = WordApp::new(Session::new(wordcraft_doc::Document::from_text("one\ntwo")), Services::default());
    for frame in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1440.0, 900.0))),
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
