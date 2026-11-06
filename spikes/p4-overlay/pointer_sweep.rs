//! measurement (p4-overlay): the cursor icon WordApp asks for, down a vertical line through the window.
use wordcraft_engine::Session;
use wordcraft_ui_egui::{Services, WordApp};

#[test]
fn pointer_sweep() {
    let app = WordApp::new(Session::new(wordcraft_doc::Document::from_text("one two three\nfour five six")), Services::default());
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
        |ui, app: &mut WordApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    for x in [60.0, 400.0, 720.0] {
        let mut last = None;
        for y in (0..900).step_by(6) {
            h.event(egui::Event::PointerMoved(egui::pos2(x, y as f32)));
            h.step();
            h.step();
            let icon = h.output().platform_output.cursor_icon;
            if last != Some(icon) {
                println!("x={x} y={y}: {icon:?}");
                last = Some(icon);
            }
        }
    }
}
