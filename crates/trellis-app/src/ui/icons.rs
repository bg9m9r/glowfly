//! Painting of the newer icons (code, eye, storage, flow, palette, shield, sparse, star, close,
//! lock, wrench, warning), called from `widgets::paint_icon`.

use super::widgets::Icon;
use eframe::egui::{self, Color32, Painter, Pos2, Rect, Stroke, pos2, vec2};

/// Paints `icon` centred on `c`; returns false for icons painted by `paint_icon` itself.
pub(super) fn paint(p: &Painter, c: Pos2, icon: Icon, s: Stroke) -> bool {
    let line = |a: Pos2, b: Pos2| p.line_segment([a, b], s);
    match icon {
        Icon::Code => {
            line(pos2(c.x - 5.0, c.y - 5.0), pos2(c.x - 9.0, c.y));
            line(pos2(c.x - 9.0, c.y), pos2(c.x - 5.0, c.y + 5.0));
            line(pos2(c.x + 2.0, c.y - 7.0), pos2(c.x - 2.0, c.y + 7.0));
            line(pos2(c.x + 5.0, c.y - 5.0), pos2(c.x + 9.0, c.y));
            line(pos2(c.x + 9.0, c.y), pos2(c.x + 5.0, c.y + 5.0));
        }
        Icon::Eye | Icon::EyeOff => {
            let (l, r) = (pos2(c.x - 8.0, c.y), pos2(c.x + 8.0, c.y));
            for dy in [-7.0, 7.0] {
                let lid = [l, pos2(c.x, c.y + dy), r];
                p.add(egui::epaint::QuadraticBezierShape::from_points_stroke(lid, false, Color32::TRANSPARENT, s));
            }
            p.circle_stroke(c, 2.2, s);
            if icon == Icon::EyeOff {
                line(pos2(c.x - 7.0, c.y + 6.0), pos2(c.x + 7.0, c.y - 6.0));
            }
        }
        Icon::Storage => {
            // A cylinder: the top ellipse, two sides, a middle band and the bottom arc.
            let (rx, ry, top, bottom) = (7.0, 2.6, c.y - 5.5, c.y + 5.5);
            p.add(egui::Shape::ellipse_stroke(pos2(c.x, top), vec2(rx, ry), s));
            line(pos2(c.x - rx, top), pos2(c.x - rx, bottom));
            line(pos2(c.x + rx, top), pos2(c.x + rx, bottom));
            let lower_arc = |y: f32| {
                let pts = (0..=16).map(|i| {
                    let a = std::f32::consts::PI * i as f32 / 16.0;
                    pos2(c.x + rx * a.cos(), y + ry * a.sin())
                });
                p.add(egui::Shape::line(pts.collect(), s));
            };
            lower_arc((top + bottom) / 2.0);
            lower_arc(bottom);
        }
        Icon::Flow => {
            // A trunk with a side branch that leaves it and joins it again lower down.
            let (x, r) = (c.x - 4.0, 1.8);
            p.circle_stroke(pos2(x, c.y - 8.0), r, s);
            p.circle_stroke(pos2(x, c.y + 8.0), r, s);
            line(pos2(x, c.y - 8.0 + r), pos2(x, c.y + 8.0 - r));
            let commit = pos2(c.x + 4.0, c.y + 1.0);
            p.circle_stroke(commit, r, s);
            line(pos2(x, c.y - 4.0), pos2(commit.x, commit.y - r));
            line(pos2(commit.x, commit.y + r), pos2(x, c.y + 6.0));
        }
        Icon::Palette => {
            // A rounded search field above two list rows, the second shorter.
            let field = Rect::from_min_max(pos2(c.x - 8.5, c.y - 8.0), pos2(c.x + 8.5, c.y - 2.0));
            p.rect_stroke(field, 2.0, s, egui::StrokeKind::Middle);
            line(pos2(c.x - 8.5, c.y + 2.5), pos2(c.x + 8.5, c.y + 2.5));
            line(pos2(c.x - 8.5, c.y + 7.0), pos2(c.x + 3.5, c.y + 7.0));
        }
        Icon::Shield => {
            // A shield outline: flat shoulders, straight sides, tapering to a point below.
            let (x, y) = (c.x, c.y);
            let pts = vec![
                pos2(x - 6.5, y - 6.0),
                pos2(x, y - 8.5),
                pos2(x + 6.5, y - 6.0),
                pos2(x + 6.5, y - 0.5),
                pos2(x + 4.0, y + 4.5),
                pos2(x, y + 8.5),
                pos2(x - 4.0, y + 4.5),
                pos2(x - 6.5, y - 0.5),
            ];
            p.add(egui::Shape::closed_line(pts, s));
            // The exclamation mark inside.
            line(pos2(x, y - 4.5), pos2(x, y + 1.0));
            p.circle_filled(pos2(x, y + 4.0), 1.1, s.color);
        }
        Icon::Sparse => {
            // A folder (tab on the top left) holding a solid row and a row of three dots.
            let (l, r, t, b) = (c.x - 8.0, c.x + 8.0, c.y - 4.5, c.y + 6.5);
            let pts = vec![pos2(l, t - 2.5), pos2(c.x - 3.0, t - 2.5), pos2(c.x - 1.0, t), pos2(r, t), pos2(r, b), pos2(l, b)];
            p.add(egui::Shape::closed_line(pts, s));
            line(pos2(c.x - 4.5, c.y - 0.5), pos2(c.x + 4.5, c.y - 0.5));
            for dx in [-4.0, 0.0, 4.0] {
                p.circle_filled(pos2(c.x + dx, c.y + 3.5), 1.1, s.color);
            }
        }
        Icon::Star | Icon::StarFilled => {
            // Five outer points (radius 7) alternating with five inner ones (radius 3).
            let at = |i: usize, r: f32| {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                pos2(c.x + r * a.cos(), c.y + r * a.sin())
            };
            let pts: Vec<Pos2> = (0..10).map(|i| at(i, if i % 2 == 0 { 7.0 } else { 3.0 })).collect();
            if icon == Icon::StarFilled {
                // A star isn't convex: fill the inner pentagon and each point's triangle.
                let inner: Vec<Pos2> = pts.iter().skip(1).step_by(2).copied().collect();
                p.add(egui::Shape::convex_polygon(inner, s.color, Stroke::NONE));
                for i in (0..10).step_by(2) {
                    let tri = vec![pts[(i + 9) % 10], pts[i], pts[i + 1]];
                    p.add(egui::Shape::convex_polygon(tri, s.color, Stroke::NONE));
                }
            }
            p.add(egui::Shape::closed_line(pts, s));
        }
        Icon::Lock => {
            // A rounded body, slightly low, under a half-circle shackle, with a keyhole dot.
            let body = Rect::from_center_size(pos2(c.x, c.y + 3.0), vec2(12.0, 9.0));
            p.rect_stroke(body, 2.0, s, egui::StrokeKind::Middle);
            let (r, base) = (4.0, body.top());
            let mut pts: Vec<Pos2> = vec![pos2(c.x - r, base)];
            pts.extend((0..=16).map(|i| {
                let a = std::f32::consts::PI * i as f32 / 16.0;
                pos2(c.x - r * a.cos(), base - 2.0 - r * a.sin())
            }));
            pts.push(pos2(c.x + r, base));
            p.add(egui::Shape::line(pts, s));
            p.circle_filled(pos2(c.x, body.center().y), 1.3, s.color);
        }
        Icon::Tools => {
            // A handle from the bottom left to an open jaw at the top right, its gap facing
            // up and to the right.
            use std::f32::consts::PI;
            let (jaw, r) = (pos2(c.x + 3.0, c.y - 3.0), 4.5);
            line(pos2(c.x - 7.5, c.y + 7.5), pos2(jaw.x - r * 0.7, jaw.y + r * 0.7));
            // 270° clockwise from pointing right round to pointing up, leaving the up-right
            // quarter open.
            let pts: Vec<Pos2> = (0..=24)
                .map(|i| {
                    let a = 1.5 * PI * i as f32 / 24.0;
                    pos2(jaw.x + r * a.cos(), jaw.y + r * a.sin())
                })
                .collect();
            p.add(egui::Shape::line(pts, s));
        }
        Icon::Warning => {
            // A triangle about 16px wide around an exclamation mark.
            let (top, left, right) = (pos2(c.x, c.y - 7.5), pos2(c.x - 8.0, c.y + 7.0), pos2(c.x + 8.0, c.y + 7.0));
            line(top, left);
            line(left, right);
            line(right, top);
            line(pos2(c.x, c.y - 3.0), pos2(c.x, c.y + 2.0));
            p.circle_filled(pos2(c.x, c.y + 4.6), 1.1, s.color);
        }
        Icon::Close => {
            line(pos2(c.x - 4.5, c.y - 4.5), pos2(c.x + 4.5, c.y + 4.5));
            line(pos2(c.x - 4.5, c.y + 4.5), pos2(c.x + 4.5, c.y - 4.5));
        }
        _ => return false,
    }
    true
}
