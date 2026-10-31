//! Drawing helpers for the commit graph: lanes and the details panel's avatar.

use super::LANE_W;
use super::lane_colours::RowColours;
use crate::ui::avatars::Avatars;
use crate::ui::theme::branch_colours::initials_tones;
use crate::ui::widgets;
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Rect, Stroke, epaint::CubicBezierShape, epaint::RectShape, pos2, vec2};
use glowfly_core::graph::GraphRow;

/// The centre of `lane` in a GRAPH column whose content starts at `graph_x`.
pub fn lane_x(graph_x: f32, lane: usize) -> f32 {
    graph_x + lane as f32 * LANE_W + LANE_W / 2.0
}

fn edge(p: &egui::Painter, a: egui::Pos2, b: egui::Pos2, c: Color32) {
    let stroke = Stroke::new(2.0_f32, c);
    if (a.x - b.x).abs() < 0.5 {
        p.line_segment([a, b], stroke);
    } else {
        let my = (a.y + b.y) / 2.0;
        p.add(CubicBezierShape::from_points_stroke([a, pos2(a.x, my), pos2(b.x, my), b], false, Color32::TRANSPARENT, stroke));
    }
}

/// Draws row `g`'s lane lines in `rect`, each in the colour of the branch owning it there.
pub(super) fn draw_lanes(p: &egui::Painter, graph_x: f32, rect: Rect, g: &GraphRow, colours: &RowColours) {
    let (top, mid, bottom) = (rect.top(), rect.center().y, rect.bottom());
    let x = |i| lane_x(graph_x, i);
    for i in g.through.iter() {
        edge(p, pos2(x(i), top), pos2(x(i), bottom), colours.above(i));
    }
    for &(i, c) in &g.top {
        edge(p, pos2(x(i), top), pos2(x(c), mid), colours.above(i));
    }
    for &(c, j) in &g.bottom {
        edge(p, pos2(x(c), mid), pos2(x(j), bottom), colours.below(j));
    }
}

/// An author avatar inside a `ring` (the branch's colour): the image when available, otherwise
/// initials in the person's colours (`initials_tones`).
pub fn draw_avatar(p: &egui::Painter, c: egui::Pos2, r: f32, name: &str, email: &str, ring: Color32, avatars: Option<&mut Avatars>) {
    p.circle_filled(c, r, ring);
    let inner = r - 2.0;
    if let Some(tex) = avatars.and_then(|a| a.get(email)) {
        let rect = Rect::from_center_size(c, vec2(inner * 2.0, inner * 2.0));
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        p.add(RectShape::filled(rect, CornerRadius::same(inner as u8), Color32::WHITE).with_texture(tex.id(), uv));
    } else {
        let (fill, ink) = initials_tones(email, name);
        p.circle_filled(c, inner, fill);
        p.text(c, Align2::CENTER_CENTER, widgets::initials(name), FontId::proportional(r * 0.85), ink);
    }
}

pub fn paint_pencil(p: &egui::Painter, c: egui::Pos2, col: Color32) {
    let s = Stroke::new(2.0_f32, col);
    p.line_segment([pos2(c.x - 4.0, c.y + 4.0), pos2(c.x + 4.0, c.y - 4.0)], s);
    p.line_segment([pos2(c.x - 5.0, c.y + 5.0), pos2(c.x - 3.0, c.y + 5.0)], Stroke::new(1.5_f32, col));
}
