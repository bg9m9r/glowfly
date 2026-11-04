//! A graph row's node, in its lane's colour: an ordinary commit's author (initials on the
//! person's own muted tone, `theme::initials`, or the avatar) inside a ring, 20 px across in Comfortable rows and 16
//! in Compact ones (`node_r`), a merge commit's small solid dot, and a WIP row's dashed ring.

use crate::ui::avatars::Avatars;
use crate::ui::theme::initials::initials_tones;
use crate::ui::widgets;
use eframe::egui::{Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, epaint::RectShape, pos2, vec2};
use glowfly_core::settings::Density;

/// The radius of a commit's node and of a WIP row's ring in Comfortable rows, the larger.
pub(super) const NODE_R: f32 = 10.0;

/// The radius of a commit's node and of a WIP row's ring at `density`.
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
    p.text(c, Align2::CENTER_CENTER, widgets::initials(name), FontId::proportional(r * 0.8), ink);
}

/// A merge commit's node: a small solid dot.
pub(super) fn merge_dot(p: &Painter, c: Pos2, lane: Color32) {
    p.circle_filled(c, MERGE_R, lane);
}

/// A WIP row's node: a dashed ring of radius `r`.
pub(super) fn wip_ring(p: &Painter, c: Pos2, r: f32, lane: Color32) {
    widgets::dashed_circle(p, c, r, Stroke::new(1.6_f32, lane));
}
