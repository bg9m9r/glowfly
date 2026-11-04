//! The HEAD lantern: the one glow on screen, round the checked-out commit's node, and the
//! `lantern` rim on the node's edge (docs/COLOUR.md, "lantern"). HEAD's node is drawn from a
//! centre snapped to the pixel grid, and the glow starts a whole pixel past the rim's
//! antialiased edge, so the rim sits on the plain row fill and keeps its 3:1 (checked on the
//! rendered pixels in `lantern_pixel_tests.rs`). It fades to the row's edge (a
//! smoothstep) as opaque rings, each the row's top fill tinted toward `lantern` through
//! `contrast::tint` (a plain warm wash on a dark field reads brown). On a hovered row it stops
//! short of the hover's shadow. None in the high-contrast themes, nor on a row a search dims; it
//! is static, so reduce motion keeps it.

use super::nodes::{NodeKind, radius};
use super::row_look::{RowLook, top_fill};
use crate::ui::theme::color::{self, Palette};
use crate::ui::theme::contrast::tint;
use crate::ui::widgets::row_fill::HOVER_SHADOW_H;
use eframe::egui::{Color32, Mesh, Painter, Pos2, Rect, Stroke, emath::GuiRounding, vec2};
use git2::Oid;
use glowfly_core::repo::CommitInfo;

/// How far past the node's edge the glow reaches, unless the row's edge comes first.
const REACH: f32 = 10.0;
/// The rings the glow is drawn in (`RINGS + 1` radii) and the segments round each.
const RINGS: usize = 8;
const SEGMENTS: usize = 48;
/// About how far outside the node's edge the rim's centre line runs (`rim_radius` puts it on
/// a pixel centre), and its width before `rim_width` adds the pixel egui's feathering takes:
/// a stroke paints a solid core only where it is wider than its 1 px feather, so the rim is
/// 1 pt + 1 px wide and paints one pixel of solid `lantern` across.
const RIM_GAP: f32 = 1.5;
const RIM_W: f32 = 1.0;
/// How far egui's antialiasing carries a stroke past its edge, in physical px.
const FEATHER: f32 = 0.5;
/// How far outside the node's edge the glow starts: a whole pixel past the rim's antialiased
/// edge, so a pixel of plain fill shows between the rim and the glow.
const GLOW_GAP: f32 = 4.0;
/// Whether, round a node whose edge is `edge` at `ppp` pixels per point, a whole pixel of plain
/// fill lies between the rim's antialiased outer edge and the glow (in points; `rim_radius`
/// worked out as a const).
const fn gap_holds(edge: f32, ppp: f32) -> bool {
    let px = 1.0 / ppp;
    let rim_r = ((((edge + RIM_GAP) * ppp - 0.5) + 0.5) as i32 as f32 + 0.5) / ppp;
    edge + GLOW_GAP >= rim_r + (RIM_W + px) / 2.0 + FEATHER * px + px
}
// A Comfortable and a Compact commit's node (10 and 8) and a merge dot (4), at 1 and 1.5.
const _: () = assert!(gap_holds(10.0, 1.0) && gap_holds(8.0, 1.0) && gap_holds(4.0, 1.0));
const _: () = assert!(gap_holds(10.0, 1.5) && gap_holds(8.0, 1.5) && gap_holds(4.0, 1.5));

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
/// rim goes: HEAD's node centre snapped to the pixel grid, and the node's edge.
pub(super) fn glow(
    p: &Painter,
    c: Pos2,
    (commit, head): (Option<&CommitInfo>, Option<Oid>),
    sizes: (f32, f32),
    (look, shown): (RowLook, bool),
) -> Option<(Pos2, f32)> {
    let commit = commit.filter(|c| glows(head == Some(c.oid), color::current().high_contrast, shown))?;
    let kind = if commit.parents.len() > 1 { NodeKind::Merge } else { NodeKind::Commit };
    head_glow(p, c, kind, sizes, look)
}

/// The glow round HEAD's `kind` node (radius `node_r`, in a `row_h` row) on a row that looks
/// `look`, from `c` snapped to the pixel grid; None in the high-contrast themes. On a lifted
/// row (hovered, not selected) it stops above the hover's shadow. Returns the snapped centre
/// and the node's edge.
pub(super) fn head_glow(p: &Painter, c: Pos2, kind: NodeKind, (node_r, row_h): (f32, f32), look: RowLook) -> Option<(Pos2, f32)> {
    let pal = color::current();
    if pal.high_contrast {
        return None;
    }
    let c = c.round_to_pixels(p.pixels_per_point());
    let (edge, inner, outer) = extent(kind, node_r, row_h);
    let clip = clip_for(p.clip_rect(), look.hovered && !look.selected);
    p.with_clip_rect(clip).add(mesh(&pal, c, (inner, outer), top_fill(look)));
    Some((c, edge))
}

/// The glow's clip within the row's `clip`: on a `lifted` row (hovered, not selected) it stops
/// above the hover's shadow, which it would otherwise paint over.
pub(super) fn clip_for(mut clip: Rect, lifted: bool) -> Rect {
    if lifted {
        clip.max.y -= HOVER_SHADOW_H;
    }
    clip
}

/// The glow's rings centred on `c` from `inner` to `outer`, tinted on `fill`.
fn mesh(pal: &Palette, c: Pos2, (inner, outer): (f32, f32), fill: Color32) -> Mesh {
    let mut mesh = Mesh::default();
    for i in 0..=RINGS {
        let t = i as f32 / RINGS as f32;
        let (r, col) = (inner + (outer - inner) * t, shade(pal, fill, t));
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
    mesh
}

/// The rim's radius round a node whose edge is `edge` from a centre on the pixel grid, at
/// `ppp` pixels per point: about `RIM_GAP` out, on a pixel centre.
pub(super) fn rim_radius(edge: f32, ppp: f32) -> f32 {
    (((edge + RIM_GAP) * ppp - 0.5).round() + 0.5) / ppp
}

/// The rim's width at `ppp`: 1 pt and the pixel egui's feathering takes.
pub(super) fn rim_width(ppp: f32) -> f32 {
    RIM_W + 1.0 / ppp
}

/// The opaque `lantern` rim just outside a glowing node whose edge is `edge` from `c`.
pub(super) fn rim(p: &Painter, c: Pos2, edge: f32) {
    let (ppp, lantern) = (p.pixels_per_point(), color::current().lantern);
    // egui strokes a circle outside its radius: start half the width in so the rim centres on
    // `rim_radius`.
    let w = rim_width(ppp);
    p.circle_stroke(c, rim_radius(edge, ppp) - w / 2.0, Stroke::new(w, lantern));
}

#[cfg(test)]
#[path = "lantern_pixel_tests.rs"]
mod pixel_tests;

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
        assert_eq!(extent(NodeKind::Commit, 10.0, 36.0), (10.0, 14.0, 18.0));
        assert_eq!(extent(NodeKind::Commit, 8.0, 28.0), (8.0, 12.0, 14.0));
        assert_eq!(extent(NodeKind::Merge, 10.0, 36.0), (4.0, 8.0, 14.0));
        assert_eq!(extent(NodeKind::Merge, 8.0, 28.0), (4.0, 8.0, 14.0));
    }

    /// How far `rim_radius` may move the rim to put it on a pixel centre, in physical px.
    const RIM_SNAP: f32 = 0.5;

    #[test]
    fn the_rim_sits_on_a_pixel_centre() {
        for edge in [4.0, 8.0, 10.0] {
            assert_eq!(rim_radius(edge, 1.0), edge + 1.5);
            for ppp in [1.0, 1.25, 1.5, 2.0] {
                let r = rim_radius(edge, ppp);
                assert_eq!((r * ppp).fract(), 0.5, "{edge} at {ppp}");
                assert!((r - edge - RIM_GAP).abs() <= RIM_SNAP / ppp + 1e-4, "{edge} at {ppp}: {r}");
            }
        }
    }

    #[test]
    fn a_lifted_row_clips_the_glow_above_the_hover_shadow() {
        let row = Rect::from_min_max(eframe::egui::pos2(0.0, 0.0), eframe::egui::pos2(100.0, 36.0));
        assert_eq!(clip_for(row, false), row);
        assert_eq!(clip_for(row, true), Rect::from_min_max(row.min, eframe::egui::pos2(100.0, 36.0 - HOVER_SHADOW_H)));
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
