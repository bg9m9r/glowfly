//! A graph row's node, in its lane's colour: an ordinary commit's author (initials on the
//! person's own muted tone, `theme::initials`, or the avatar) inside a ring, 20 px across in Comfortable rows and 16
//! in Compact ones (`node_r`), a merge commit's small solid dot, this tab's WIP row's lantern
//! (in `lantern`, lit while the tree has changes) and another worktree's WIP row's dashed ring.

use crate::avatars::Avatars;
use crate::theme::color;
use crate::theme::initials::initials_tones;
use crate::widgets;
use crate::widgets::paint_text;
use eframe::egui::{Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, epaint::RectShape, pos2, vec2};
use glowfly_core::settings::Density;

/// The radius of a commit's node and of a WIP row's ring or lantern in Comfortable rows, the larger.
pub(super) const NODE_R: f32 = 10.0;

/// The radius of a commit's node and of a WIP row's ring or lantern at `density`.
pub(super) fn node_r(density: Density) -> f32 {
    match density {
        Density::Comfortable => NODE_R,
        Density::Compact => 8.0,
    }
}
/// The radius of a merge commit's dot.
pub(super) const MERGE_R: f32 = 4.0;

/// Which node a commit row draws.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum NodeKind {
    Commit,
    Merge,
}

/// How far the node reaches from its centre (a commit's is `r`), where the line from the row's
/// labels ends.
pub(super) fn radius(kind: NodeKind, r: f32) -> f32 {
    match kind {
        NodeKind::Commit => r,
        NodeKind::Merge => MERGE_R,
    }
}

/// An ordinary commit's node of radius `r` centred on `c`: the author's avatar inside a 2 px
/// `lane` ring when `avatars` has it, otherwise the initials of `name` in the person's colours
/// (`initials_tones`) inside a thinner `lane` ring.
pub(super) fn commit(p: &Painter, (c, r): (Pos2, f32), name: &str, email: &str, lane: Color32, avatars: Option<&mut Avatars>) {
    if let Some(tex) = avatars.and_then(|a| a.get(email)) {
        p.circle_filled(c, r, lane);
        let inner = r - 2.0;
        let rect = Rect::from_center_size(c, vec2(inner * 2.0, inner * 2.0));
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        p.add(RectShape::filled(rect, CornerRadius::same(inner as u8), Color32::WHITE).with_texture(tex.id(), uv));
        return;
    }
    let (fill, ink) = initials_tones(email, name);
    p.circle_filled(c, r, fill);
    p.circle_stroke(c, r - 0.75, Stroke::new(1.5_f32, lane));
    paint_text(p, c, Align2::CENTER_CENTER, widgets::initials(name), FontId::proportional(r * 0.8), ink);
}

/// A merge commit's node: a small solid dot.
pub(super) fn merge_dot(p: &Painter, c: Pos2, lane: Color32) {
    p.circle_filled(c, MERGE_R, lane);
}

/// Another worktree's WIP row's node: a dashed ring of radius `r`.
pub(super) fn wip_ring(p: &Painter, c: Pos2, r: f32, lane: Color32) {
    widgets::dashed_circle(p, c, r, Stroke::new(1.6_f32, lane));
}

/// The sizes of the WIP lantern, by density (geometry of the mark, not the corner scale): the
/// cap's size and corner radius and the gap under it, the body's
/// size and corner radius (its top `body_top` above the row's middle), and the flame's size.
struct LanternSize {
    cap: (f32, f32),
    cap_r: f32,
    gap: f32,
    body: (f32, f32),
    body_r: f32,
    body_top: f32,
    flame: (f32, f32),
}

const LANTERN_COMFORTABLE: LanternSize =
    LanternSize { cap: (6.0, 2.0), cap_r: 1.0, gap: 2.0, body: (10.0, 12.0), body_r: 3.0, body_top: 4.0, flame: (4.0, 6.0) };
const LANTERN_COMPACT: LanternSize =
    LanternSize { cap: (5.0, 2.0), cap_r: 1.0, gap: 1.0, body: (8.0, 10.0), body_r: 2.5, body_top: 3.5, flame: (2.0, 4.0) };

/// The rects of the WIP lantern centred on `c` for a node of radius `r`: cap, body and flame.
fn lantern_rects(c: Pos2, r: f32) -> (Rect, Rect, Rect, &'static LanternSize) {
    let s = if r >= NODE_R { &LANTERN_COMFORTABLE } else { &LANTERN_COMPACT };
    let body = Rect::from_min_size(pos2(c.x - s.body.0 / 2.0, c.y - s.body_top), vec2(s.body.0, s.body.1));
    let cap = Rect::from_min_size(pos2(c.x - s.cap.0 / 2.0, body.top() - s.gap - s.cap.1), vec2(s.cap.0, s.cap.1));
    let flame = Rect::from_center_size(body.center(), vec2(s.flame.0, s.flame.1));
    (cap, body, flame, s)
}

/// This tab's WIP row's node: a small lantern centred on `c` in `lantern`, its body filled with
/// the row's top fill `fill` (hiding the lane line, which leaves from its bottom), with a flame
/// while the tree has changes (`lit`).
pub(super) fn wip_lantern(p: &Painter, c: Pos2, r: f32, fill: Color32, lit: bool) {
    let ink = color::current().lantern;
    let (cap, body, flame, s) = lantern_rects(c, r);
    p.rect_filled(cap, CornerRadius::same(s.cap_r as u8), ink);
    p.rect_filled(body, CornerRadius::same(s.body_r as u8), fill);
    p.rect_stroke(body, CornerRadius::same(s.body_r as u8), Stroke::new(1.5_f32, ink), StrokeKind::Inside);
    if lit {
        p.rect_filled(flame, CornerRadius::same((s.flame.0 / 2.0) as u8), ink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lantern_is_sixteen_px_tall_comfortable_and_thirteen_compact() {
        let c = pos2(0.0, 0.0);
        let (cap, body, flame, _) = lantern_rects(c, NODE_R);
        assert_eq!((cap.top(), cap.bottom(), body.top(), body.bottom()), (-8.0, -6.0, -4.0, 8.0));
        assert_eq!((body.width(), cap.width(), flame.center().y), (10.0, 6.0, 2.0));
        let (cap, body, flame, _) = lantern_rects(c, node_r(Density::Compact));
        assert_eq!((cap.top(), body.top(), body.bottom()), (-6.5, -3.5, 6.5));
        assert_eq!((body.width(), flame.size(), flame.center().y), (8.0, vec2(2.0, 4.0), 1.5));
    }
}
