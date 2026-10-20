//! Painting of the terminal panel's header icons (eraser, stop, maximise and restore, and the
//! hide chevron), called from `widgets::paint_icon`. Same 20 px grid and stroke as the others.

use super::widgets::Icon;
use eframe::egui::{CornerRadius, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2, pos2, vec2};

/// Paints `icon` centred on `c`; returns false for icons painted elsewhere.
pub(super) fn paint(p: &Painter, c: Pos2, icon: Icon, s: Stroke) -> bool {
    let line = |a: Pos2, b: Pos2| p.line_segment([a, b], s);
    // An arrow from `from` to `to` with a two-stroke head along the axes, pointing away from
    // `from` (the head's arms run back toward it).
    let arrow = |from: Pos2, to: Pos2| {
        line(from, to);
        let back = vec2((from.x - to.x).signum(), (from.y - to.y).signum()) * 4.5;
        line(to, to + vec2(back.x, 0.0));
        line(to, to + vec2(0.0, back.y));
    };
    match icon {
        Icon::Eraser => {
            // A block tilted 45 degrees, a line across its rubbing end, and the surface under it.
            let m = c + vec2(0.5, -1.5);
            let (u, v) = (vec2(1.0, -1.0) / 2f32.sqrt(), vec2(1.0, 1.0) / 2f32.sqrt());
            let corner = |a: f32, b: f32| m + u * a + v * b;
            let body = vec![corner(-7.0, -3.5), corner(7.0, -3.5), corner(7.0, 3.5), corner(-7.0, 3.5)];
            p.add(eframe::egui::Shape::closed_line(body, s));
            line(corner(-2.5, -3.5), corner(-2.5, 3.5));
            line(pos2(c.x - 1.0, c.y + 8.0), pos2(c.x + 8.0, c.y + 8.0));
        }
        Icon::Stop => {
            p.rect_stroke(Rect::from_center_size(c, Vec2::splat(11.0)), CornerRadius::same(2), s, StrokeKind::Middle);
        }
        Icon::PanelMaximize => {
            arrow(c + vec2(1.5, -1.5), c + vec2(7.0, -7.0));
            arrow(c + vec2(-1.5, 1.5), c + vec2(-7.0, 7.0));
        }
        Icon::PanelRestore => {
            arrow(c + vec2(7.0, -7.0), c + vec2(1.5, -1.5));
            arrow(c + vec2(-7.0, 7.0), c + vec2(-1.5, 1.5));
        }
        Icon::ChevronDown => {
            line(pos2(c.x - 5.0, c.y - 2.5), pos2(c.x, c.y + 2.5));
            line(pos2(c.x, c.y + 2.5), pos2(c.x + 5.0, c.y - 2.5));
        }
        _ => return false,
    }
    true
}
