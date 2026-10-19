//! A graph row's node, in its lane's colour: an ordinary commit's author (initials on a muted
//! fill, or the avatar) inside a ring, a merge commit's small solid dot, and a WIP row's dashed
//! ring.

use crate::ui::avatars::Avatars;
use crate::ui::theme::color;
use crate::ui::theme::contrast::{mix, readable_on};
use crate::ui::widgets;
use eframe::egui::{Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, epaint::RectShape, pos2, vec2};

/// The radius of a commit's node and of a WIP row's ring.
pub(super) const NODE_R: f32 = 11.0;
/// The radius of a merge commit's dot.
pub(super) const MERGE_R: f32 = 4.0;
/// How far the muted fill behind initials moves from the background toward the lane colour.
const FILL_T: f32 = 0.30;

/// Which node a commit row draws.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum NodeKind {
    Commit,
    Merge,
}

/// How far the node reaches from its centre, where the line from the row's labels ends.
pub(super) fn radius(kind: NodeKind) -> f32 {
    match kind {
        NodeKind::Commit => NODE_R,
        NodeKind::Merge => MERGE_R,
    }
}

/// An ordinary commit's node centred on `c`: the author's avatar inside a 2 px `lane` ring when
/// `avatars` has it, otherwise the initials of `name` on a muted fill inside a thinner ring.
pub(super) fn commit(p: &Painter, c: Pos2, name: &str, email: &str, lane: Color32, avatars: Option<&mut Avatars>) {
    if let Some(tex) = avatars.and_then(|a| a.get(email)) {
        p.circle_filled(c, NODE_R, lane);
        let inner = NODE_R - 2.0;
        let rect = Rect::from_center_size(c, vec2(inner * 2.0, inner * 2.0));
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        p.add(RectShape::filled(rect, CornerRadius::same(inner as u8), Color32::WHITE).with_texture(tex.id(), uv));
        return;
    }
    let pal = color::current();
    let fill = mix(pal.bg, lane, FILL_T);
    p.circle_filled(c, NODE_R, fill);
    p.circle_stroke(c, NODE_R - 0.75, Stroke::new(1.5_f32, lane));
    let ink = readable_on(lane, fill, pal.dark, 4.5);
    p.text(c, Align2::CENTER_CENTER, widgets::initials(name), FontId::proportional(NODE_R * 0.8), ink);
}

/// A merge commit's node: a small solid dot.
pub(super) fn merge_dot(p: &Painter, c: Pos2, lane: Color32) {
    p.circle_filled(c, MERGE_R, lane);
}

/// A WIP row's node: a dashed ring.
pub(super) fn wip_ring(p: &Painter, c: Pos2, lane: Color32) {
    widgets::dashed_circle(p, c, NODE_R, Stroke::new(1.6_f32, lane));
}
