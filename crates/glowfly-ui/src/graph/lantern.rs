//! The HEAD lantern: the one glow on screen, round the checked-out commit's node, and the 1 px
//! `lantern` rim on the node's edge (docs/COLOUR.md, "lantern"). The glow starts just past the
//! rim, so the rim sits on the plain row fill and keeps its 3:1, and fades to the row's edge (a
//! smoothstep) as opaque rings, each the row's top fill tinted toward `lantern` through
//! `contrast::tint` (a plain warm wash on a dark field reads brown). On a hovered row it stops
//! short of the hover's shadow. None in the high-contrast themes, nor on a row a search dims; it
//! is static, so reduce motion keeps it.

use super::nodes::{NodeKind, radius};
use super::row_look::{RowLook, top_fill};
use crate::ui::theme::color::{self, Palette};
use crate::ui::theme::contrast::tint;
use crate::ui::widgets::row_fill::HOVER_SHADOW_H;
use eframe::egui::{Color32, Mesh, Painter, Pos2, Stroke, vec2};
use git2::Oid;
use glowfly_core::repo::CommitInfo;

/// How far past the node's edge the glow reaches, unless the row's edge comes first.
const REACH: f32 = 10.0;
/// The rings the glow is drawn in (`RINGS + 1` radii) and the segments round each.
const RINGS: usize = 8;
const SEGMENTS: usize = 48;
/// How far outside the node's edge the rim's centre line runs, and its width.
const RIM_GAP: f32 = 1.75;
const RIM_W: f32 = 1.0;
/// How far outside the node's edge the glow starts: past the rim's outer edge and its
/// antialiasing, so the rim reads on the plain fill.
const GLOW_GAP: f32 = 2.5;

/// How strongly the glow tints its row at the node's edge: lighter on paper (a stronger warm
/// wash there reads tan) and lower again with colour-blind safe colours.
pub(crate) fn peak(dark: bool, colour_safe: bool) -> f32 {
    match (dark, colour_safe) {
        (true, false) => 0.25,
        (true, true) => 0.18,
        (false, false) => 0.16,
        (false, true) => 0.12,
    }
}

/// Whether a row glows: it is the checked-out commit's (`head`), the theme isn't high
/// contrast and the node is `shown` (the tree column is on and no search dims the row). Only a
/// commit row can be HEAD's, so a frame has at most one glow.
pub(super) fn glows(head: bool, high_contrast: bool, shown: bool) -> bool {
    head && shown && !high_contrast
}

/// Round a `kind` node of radius `node_r` in a `row_h` row: the node's edge (where the rim
/// goes), and the glow's inner and outer radius.
pub(super) fn extent(kind: NodeKind, node_r: f32, row_h: f32) -> (f32, f32, f32) {
    let edge = radius(kind, node_r);
    (edge, edge + GLOW_GAP, (edge + REACH).min(row_h / 2.0))
}

/// The glow's fall-off: slow by the rim and slow again at the row's edge.
fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// The glow's colour `t` of the way (0 to 1) from its inner edge to its outer edge, on `fill`.
pub(super) fn shade(pal: &Palette, fill: Color32, t: f32) -> Color32 {
    let a = peak(pal.dark, color::colour_safe()) * (1.0 - smoothstep(t));
    tint(fill, pal.lantern, a, pal.dark)
}

/// Paints the glow centred on `c` behind the node of radius `node_r` when the row is `commit`'s
/// and it glows (`glows`: it is `head`'s, …), on the row's top fill (`row_look::top_fill(look)`), and returns where the
/// rim goes. On a lifted row (`look` hovered, not selected) it stops above the hover's shadow.
pub(super) fn glow(
    p: &Painter,
    c: Pos2,
    (commit, head): (Option<&CommitInfo>, Option<Oid>),
    (node_r, row_h): (f32, f32),
    (look, shown): (RowLook, bool),
) -> Option<f32> {
    let (fill, lifted) = (top_fill(look), look.hovered && !look.selected);
    let pal = color::current();
    let commit = commit.filter(|c| glows(head == Some(c.oid), pal.high_contrast, shown))?;
    let kind = if commit.parents.len() > 1 { NodeKind::Merge } else { NodeKind::Commit };
    let (edge, inner, outer) = extent(kind, node_r, row_h);
    let mut mesh = Mesh::default();
    for i in 0..=RINGS {
        let t = i as f32 / RINGS as f32;
        let (r, col) = (inner + (outer - inner) * t, shade(&pal, fill, t));
        for s in 0..SEGMENTS {
            let a = s as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            mesh.colored_vertex(c + vec2(a.cos(), a.sin()) * r, col);
        }
    }
    for i in 0..RINGS as u32 {
        let (seg, ring) = (SEGMENTS as u32, i * SEGMENTS as u32);
        for s in 0..seg {
            let (a, b) = (ring + s, ring + (s + 1) % seg);
            mesh.add_triangle(a, b, a + seg);
            mesh.add_triangle(b, b + seg, a + seg);
        }
    }
    let mut clip = p.clip_rect();
    if lifted {
        clip.max.y -= HOVER_SHADOW_H;
    }
    p.with_clip_rect(clip).add(mesh);
    Some(edge)
}

/// The 1 px opaque `lantern` rim just outside a glowing node whose edge is `edge` from `c`.
pub(super) fn rim(p: &Painter, c: Pos2, edge: f32) {
    p.circle_stroke(c, edge + RIM_GAP, Stroke::new(RIM_W, color::current().lantern));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::builtin;
    use glowfly_core::settings::BuiltinTheme;

    #[test]
    fn only_the_head_row_glows_and_never_in_high_contrast_or_dimmed() {
        assert!(glows(true, false, true));
        assert!(!glows(false, false, true), "a row that isn't HEAD's");
        assert!(!glows(true, true, true), "high contrast");
        assert!(!glows(true, false, false), "dimmed by a search, or the tree column hidden");
    }

    #[test]
    fn the_glow_ends_at_the_row_edge() {
        assert_eq!(extent(NodeKind::Commit, 10.0, 36.0), (10.0, 12.5, 18.0));
        assert_eq!(extent(NodeKind::Commit, 8.0, 28.0), (8.0, 10.5, 14.0));
        assert_eq!(extent(NodeKind::Merge, 10.0, 36.0), (4.0, 6.5, 14.0));
        assert_eq!(extent(NodeKind::Merge, 8.0, 28.0), (4.0, 6.5, 14.0));
    }

    /// The rim sits on the plain fill: the glow starts past its outer edge and antialiasing.
    #[test]
    fn the_glow_leaves_the_rim_on_the_plain_fill() {
        for (node_r, row_h) in [(10.0, 36.0), (8.0, 28.0)] {
            for kind in [NodeKind::Commit, NodeKind::Merge] {
                let (edge, inner, outer) = extent(kind, node_r, row_h);
                assert!(inner >= edge + RIM_GAP + RIM_W / 2.0 + 0.25, "{kind:?} at {node_r}");
                assert!(inner < outer, "{kind:?} at {node_r}: the glow has room");
            }
        }
    }

    #[test]
    fn the_glow_fades_to_the_surface() {
        for t in BuiltinTheme::ALL {
            let p = builtin(t);
            assert_eq!(shade(p, p.bg, 1.0), p.bg, "{t:?}: no seam at the outer edge");
        }
    }

    /// Halfway out the glow still lifts the row (docs/COLOUR.md, "lantern").
    #[test]
    fn the_glow_still_shows_halfway_out() {
        use crate::ui::theme::contrast::ratio;
        for t in BuiltinTheme::ALL {
            let p = builtin(t);
            if p.high_contrast {
                continue;
            }
            let half = tint(p.bg, p.lantern, peak(p.dark, false) * (1.0 - smoothstep(0.5)), p.dark);
            assert!(ratio(half, p.bg) >= 1.08, "{t:?}: {:.3}:1 halfway out", ratio(half, p.bg));
        }
    }

    #[test]
    fn colour_blind_safe_colours_lower_the_peak() {
        for dark in [true, false] {
            assert!(peak(dark, true) < peak(dark, false));
        }
        assert_eq!(peak(true, false), 0.25);
        assert_eq!(peak(true, true), 0.18);
    }
}
