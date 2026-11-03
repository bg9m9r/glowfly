//! Painted glyphs on the pixel grid: an icon's centre moved so its strokes cover whole pixels
//! evenly on both sides (`snap_centre`), a straight arm's length kept to whole pixels from that
//! centre (`snap_arm`), and the plus (and minus) painted that way (`paint_plus`), at any pixel ratio
//! (100 %, 125 %, 150 %, 200 %). `widgets::paint_icon` snaps every icon's centre here; the
//! tab bars' glyphs (`tab_glyph.rs`) too.

use eframe::egui::{Painter, Pos2, Stroke, pos2};

/// Physical pixels a line `width` points wide covers at `ppp`: at least one.
fn px_width(width: f32, ppp: f32) -> f32 {
    (width * ppp).round().max(1.0)
}

/// Where a line `width` points wide is centred on the physical grid: on a pixel's middle (0.5)
/// when it covers an odd number of pixels, on a pixel edge (0.0) when it covers an even number.
fn grid_offset(width: f32, ppp: f32) -> f32 {
    if px_width(width, ppp) as i32 % 2 == 1 { 0.5 } else { 0.0 }
}

/// `c` moved to the nearest point where lines `width` points wide through it fall evenly on
/// the physical pixels at `ppp` (so a glyph never looks heavier on one side).
pub(super) fn snap_centre(c: Pos2, width: f32, ppp: f32) -> Pos2 {
    let off = grid_offset(width, ppp);
    let snap = |v: f32| ((v * ppp - off).round() + off) / ppp;
    pos2(snap(c.x), snap(c.y))
}

/// `arm` (from a centre snapped for lines `width` wide to a straight line's end) rounded so the
/// end lands on a pixel edge: whole pixels for an even line, a half pixel more for an odd one.
fn snap_arm(arm: f32, width: f32, ppp: f32) -> f32 {
    let off = grid_offset(width, ppp);
    (((arm * ppp - off).round() + off) / ppp).max(off / ppp)
}

/// The two strokes of a plus with arms `arm` long either side of `c`, centre and ends on the
/// pixel grid for lines `width` wide at `ppp`.
pub(super) fn plus_segments(c: Pos2, arm: f32, width: f32, ppp: f32) -> [[Pos2; 2]; 2] {
    let c = snap_centre(c, width, ppp);
    let a = snap_arm(arm, width, ppp);
    [[pos2(c.x - a, c.y), pos2(c.x + a, c.y)], [pos2(c.x, c.y - a), pos2(c.x, c.y + a)]]
}

/// Paints a plus with arms `arm` long either side of `c` in `s`, snapped (`plus_segments`).
pub(super) fn paint_plus(p: &Painter, c: Pos2, arm: f32, s: Stroke) {
    for seg in plus_segments(c, arm, s.width, p.ctx().pixels_per_point()) {
        p.line_segment(seg, s);
    }
}

/// Paints the plus's level stroke alone (a minus), snapped as the plus is, so the two match.
pub(super) fn paint_minus(p: &Painter, c: Pos2, arm: f32, s: Stroke) {
    let [level, _] = plus_segments(c, arm, s.width, p.ctx().pixels_per_point());
    p.line_segment(level, s);
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATIOS: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

    /// Whether `v` points is on a physical pixel edge (`off` 0) or middle (`off` 0.5) at `ppp`.
    fn on_grid(v: f32, ppp: f32, off: f32) -> bool {
        let phys = v * ppp - off;
        (phys - phys.round()).abs() < 1e-3
    }

    #[test]
    fn a_plus_sits_on_the_grid_at_every_scale() {
        for width in [1.4, 1.6] {
            for ppp in RATIOS {
                for c in [pos2(100.0, 50.0), pos2(100.5, 50.5), pos2(37.3, 12.8)] {
                    let [h, v] = plus_segments(c, 5.0, width, ppp);
                    let off = grid_offset(width, ppp);
                    let centre = pos2(v[0].x, h[0].y);
                    assert!(on_grid(centre.x, ppp, off) && on_grid(centre.y, ppp, off), "{width} at {ppp}: {centre:?}");
                    for end in [h[0].x, h[1].x, v[0].y, v[1].y] {
                        assert!(on_grid(end, ppp, 0.0), "{width} at {ppp}: an end on a pixel edge, {end}");
                    }
                    assert!((centre.x - h[0].x - (h[1].x - centre.x)).abs() < 1e-4, "arms alike");
                    assert!((centre.y - v[0].y - (v[1].y - centre.y)).abs() < 1e-4, "arms alike");
                    assert!((centre - c).length() <= 0.75 / ppp * std::f32::consts::SQRT_2, "moved under a pixel");
                }
            }
        }
    }

    #[test]
    fn the_plus_is_8_px_at_14_and_16_and_10_at_20() {
        // `paint_icon_sized`'s arms (5 px at 20, scaled), on 1.6 px lines at 100 %.
        for (size, across) in [(14.0, 8.0), (16.0, 8.0), (20.0, 10.0)] {
            let [h, _] = plus_segments(pos2(50.0, 50.0), 5.0 * size / 20.0, 1.6, 1.0);
            assert_eq!(h[1].x - h[0].x, across, "at {size}");
        }
    }

    #[test]
    fn odd_lines_centre_on_a_pixel_and_even_ones_between() {
        // 1.6 px at 100 % covers 2 pixels: the centre on an edge; 3.2 at 200 % covers 3.
        assert_eq!(snap_centre(pos2(10.3, 10.6), 1.6, 1.0), pos2(10.0, 11.0));
        assert_eq!(snap_centre(pos2(10.3, 10.6), 1.6, 2.0), pos2(10.25, 10.75));
        // 1.4 at 100 % covers 1 pixel: its middle.
        assert_eq!(snap_centre(pos2(10.0, 10.0), 1.4, 1.0), pos2(10.5, 10.5));
    }
}
