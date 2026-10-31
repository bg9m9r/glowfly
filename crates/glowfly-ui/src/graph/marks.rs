//! The small marks painted in the graph's ref chips and their tooltips: a monitor for a local
//! branch, a cloud for a remote one (or the upstream folded into a local branch's chip) and a tag
//! for a tag. They are painted because the bundled fonts lack suitable glyphs.

use eframe::egui::{self, Color32, CornerRadius, Painter, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};

/// The room each mark takes in a chip.
pub(super) const MARK_W: f32 = 14.0;
/// The line width of the local and tag marks.
const LINE_W: f32 = 1.3;

/// A small monitor centred on `c`: a 9×6 screen outline over a short base line, the mark of a
/// branch in this repository.
pub(super) fn paint_local(p: &Painter, c: Pos2, col: Color32) {
    let screen = Rect::from_center_size(pos2(c.x, c.y - 1.5), vec2(9.0, 6.0));
    p.rect_stroke(screen, CornerRadius::same(1), Stroke::new(LINE_W, col), StrokeKind::Middle);
    let base_y = screen.bottom() + 2.0;
    p.line_segment([pos2(c.x - 2.0, base_y), pos2(c.x + 2.0, base_y)], Stroke::new(LINE_W, col));
}

/// A tag about 10×8 centred on `c`, pointing left, with a hole near its point.
pub(super) fn paint_tag(p: &Painter, c: Pos2, col: Color32) {
    let at = |x: f32, y: f32| pos2(c.x + x, c.y + y);
    let outline = vec![at(-5.0, 0.0), at(-1.5, -4.0), at(5.0, -4.0), at(5.0, 4.0), at(-1.5, 4.0)];
    p.add(egui::epaint::PathShape::closed_line(outline, Stroke::new(LINE_W, col)));
    p.circle_filled(at(-0.8, 0.0), 1.2, col);
}

/// A cloud outline about 12 px wide centred on `c`: the remote mark. Three bumps over a flat
/// base, traced as the outline seen from a point inside.
pub(super) fn paint_cloud(p: &Painter, c: Pos2, col: Color32) {
    // (centre offset, radius) of each bump, and the base box between the outer bumps' centres.
    const BUMPS: [((f32, f32), f32); 3] = [((-4.0, 1.5), 2.0), ((0.0, -0.8), 3.3), ((3.4, 1.0), 2.5)];
    const BASE: (f32, f32, f32, f32) = (-4.0, 1.0, 3.4, 3.5);
    let o = vec2(0.0, 2.0);
    let far = |d: egui::Vec2| {
        let circles = BUMPS.iter().filter_map(|&((cx, cy), r)| {
            let m = o - vec2(cx, cy);
            let b = d.dot(m);
            let disc = b * b - (m.length_sq() - r * r);
            (disc >= 0.0).then(|| -b + disc.sqrt())
        });
        let slab = |o: f32, d: f32, lo: f32, hi: f32| {
            if d > 1e-6 {
                (hi - o) / d
            } else if d < -1e-6 {
                (lo - o) / d
            } else {
                f32::INFINITY
            }
        };
        let base = slab(o.x, d.x, BASE.0, BASE.2).min(slab(o.y, d.y, BASE.1, BASE.3));
        circles.fold(base, f32::max)
    };
    let points = (0..72)
        .map(|i| {
            let a = i as f32 / 72.0 * std::f32::consts::TAU;
            let d = vec2(a.cos(), a.sin());
            c + o + d * far(d)
        })
        .collect();
    p.add(egui::epaint::PathShape::closed_line(points, Stroke::new(1.4_f32, col)));
}
