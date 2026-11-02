//! The few icons of the phone layout that upstream's set has none of (a back arrow, a keyboard, a pencil, a check mark): strokes on a 22 x 22
//! grid in the colour the caller passes, so they follow the theme like every other icon. `paint` is false for any other name.

use egui::{Color32, Painter, Rect, Shape, Stroke, pos2};

pub fn paint(p: &Painter, r: Rect, name: &str, c: Color32) -> bool {
    let s = r.width().min(r.height()) / 22.0;
    let at = |x: f32, y: f32| pos2(r.center().x + (x - 11.0) * s, r.center().y + (y - 11.0) * s);
    let stroke = Stroke::new(1.8 * s, c);
    let line = |pts: &[(f32, f32)]| p.add(Shape::line(pts.iter().map(|&(x, y)| at(x, y)).collect(), stroke));
    match name {
        "phoneBack" => {
            line(&[(18.0, 11.0), (4.0, 11.0)]);
            line(&[(10.0, 5.0), (4.0, 11.0), (10.0, 17.0)]);
        }
        "keyboard" => {
            p.rect_stroke(Rect::from_min_max(at(1.5, 5.0), at(20.5, 17.0)), 2.0 * s, stroke, egui::StrokeKind::Middle);
            for (y, xs) in [(8.5, &[5.0, 8.5, 12.0, 15.5][..]), (11.5, &[6.5, 10.0, 13.5, 17.0][..])] {
                for x in xs {
                    p.circle_filled(at(*x, y), 0.95 * s, c);
                }
            }
            line(&[(6.0, 14.5), (16.0, 14.5)]);
        }
        "phonePencil" => {
            p.add(Shape::closed_line([(4.0, 18.0), (5.0, 13.5), (14.5, 4.0), (18.0, 7.5), (8.5, 17.0)].iter().map(|&(x, y)| at(x, y)).collect(), stroke));
            line(&[(12.5, 6.0), (16.0, 9.5)]);
        }
        "done" => {
            line(&[(4.0, 11.5), (9.0, 16.5), (18.5, 6.0)]);
        }
        _ => return false,
    }
    true
}
