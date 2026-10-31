//! Painting of the changed-file lists' List / Tree toggle icons, called from
//! `widgets::paint_icon`. Same 20 px grid and stroke as the others.

use crate::ui::widgets::Icon;
use eframe::egui::{Painter, Pos2, Stroke, pos2};

/// Paints `icon` centred on `c`; returns false for icons painted elsewhere.
pub(crate) fn paint(p: &Painter, c: Pos2, icon: Icon, s: Stroke) -> bool {
    let line = |x0: f32, y0: f32, x1: f32, y1: f32| p.line_segment([pos2(c.x + x0, c.y + y0), pos2(c.x + x1, c.y + y1)], s);
    let dot = |x: f32, y: f32| p.circle_filled(pos2(c.x + x, c.y + y), s.width * 0.8, s.color);
    match icon {
        Icon::ListView => {
            // Three rows, a dot left of each.
            for y in [-5.0, 0.0, 5.0] {
                dot(-6.5, y);
                line(-3.0, y, 7.5, y);
            }
        }
        Icon::TreeView => {
            // A top row; from its left end an elbow down to two indented rows.
            line(-7.5, -6.0, 7.5, -6.0);
            line(-5.5, -6.0, -5.5, 5.0);
            line(-5.5, 0.0, -2.0, 0.0);
            line(-5.5, 5.0, -2.0, 5.0);
            line(0.5, 0.0, 7.5, 0.0);
            line(0.5, 5.0, 7.5, 5.0);
        }
        _ => return false,
    }
    true
}
