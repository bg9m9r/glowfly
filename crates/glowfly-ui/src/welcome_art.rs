//! The welcome header's drawing: the app icon's diagonal lattice with a commit vine laid
//! sideways across it, in one style per kind of palette. Dark and Light tint the band toward the
//! accent and let the lattice melt into it at both ends; high contrast keeps the plain panel, a
//! full-strength lattice and a rule under the band. Worked out from palette tokens only.

use super::theme::color::Palette;
use super::theme::contrast;
use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, epaint::QuadraticBezierShape, pos2};

/// The lattice's spacing, and in high contrast (thicker lines need more room).
const SPACING: f32 = 22.0;
const SPACING_HC: f32 = 28.0;
/// The part of the width at each end over which the lattice fades into the band.
const FADE: f32 = 0.30;
/// A faded lattice line is drawn in pieces this long (horizontally), each in its own tone.
const PIECE: f32 = 6.0;
/// A commit node's ring and its centre dot.
const NODE_R: f32 = 6.0;
const DOT_R: f32 = 2.2;

/// Which drawing a palette gets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum ArtKind {
    Dark,
    Light,
    HighContrast,
}

/// The band's and the drawing's colours and line widths.
#[derive(Clone, Copy, Debug)]
pub(super) struct ArtStyle {
    pub kind: ArtKind,
    /// The header band's fill.
    pub band: Color32,
    pub lattice: Color32,
    pub vine: Color32,
    /// Inside a commit node's ring.
    pub node_fill: Color32,
    /// A 1 px line under the band (high contrast).
    pub rule: Option<Color32>,
    pub lattice_w: f32,
    pub vine_w: f32,
    /// The lattice fades into the band at both ends.
    pub fade: bool,
}

impl ArtStyle {
    fn spacing(&self) -> f32 {
        if self.kind == ArtKind::HighContrast { SPACING_HC } else { SPACING }
    }
}

/// Whether `p` is a high-contrast palette: its hairlines already stand out from the panel.
pub(super) fn high_contrast(p: &Palette) -> bool {
    contrast::ratio(p.border, p.panel) >= 3.0
}

/// The drawing's style for `p`.
pub(super) fn art_style(p: &Palette) -> ArtStyle {
    if high_contrast(p) {
        return ArtStyle {
            kind: ArtKind::HighContrast,
            band: p.panel,
            lattice: p.border,
            vine: p.accent,
            node_fill: p.panel,
            rule: Some(p.border),
            lattice_w: 1.0,
            vine_w: 3.0,
            fade: false,
        };
    }
    let (kind, tint, strength) = if p.dark { (ArtKind::Dark, 0.10, 0.28) } else { (ArtKind::Light, 0.08, 0.30) };
    let band = contrast::mix(p.panel, p.accent, tint);
    ArtStyle {
        kind,
        band,
        lattice: contrast::mix(band, p.accent, strength),
        vine: p.accent,
        node_fill: band,
        rule: None,
        lattice_w: 1.0,
        vine_w: 2.5,
        fade: true,
    }
}

/// Paints the drawing into `r` (the painter clips to it).
pub(super) fn paint(p: &Painter, r: Rect, s: &ArtStyle) {
    lattice(p, r, s);
    vine(p, r, s);
}

/// How strong the lattice is at `x`: 0 at either end of `r`, full past the outer `FADE`.
fn fade_at(r: Rect, x: f32) -> f32 {
    let edge = (x - r.left()).min(r.right() - x).max(0.0);
    (edge / (FADE * r.width()).max(1.0)).min(1.0)
}

/// Lines at +45° and -45°, `spacing` apart, from edge to edge of `r`.
fn lattice(p: &Painter, r: Rect, s: &ArtStyle) {
    let step = s.spacing();
    let (l, t, rt, b) = (r.left(), r.top(), r.right(), r.bottom());
    // Lines y = x - k (falling to the right) and y = k - x (rising), on a grid fixed to the
    // rect's corner so the pattern doesn't crawl as the width changes.
    let mut k = l - b;
    while k <= rt - t {
        let (x0, x1) = ((t + k).max(l), (b + k).min(rt));
        if x1 > x0 {
            line(p, r, s, pos2(x0, x0 - k), pos2(x1, x1 - k));
        }
        k += step;
    }
    let mut k = l + t;
    while k <= rt + b {
        let (x0, x1) = ((k - b).max(l), (k - t).min(rt));
        if x1 > x0 {
            line(p, r, s, pos2(x0, k - x0), pos2(x1, k - x1));
        }
        k += step;
    }
}

/// One lattice line, in pieces that fade toward the ends of `r` when the style fades.
fn line(p: &Painter, r: Rect, s: &ArtStyle, a: Pos2, b: Pos2) {
    if !s.fade {
        p.line_segment([a, b], Stroke::new(s.lattice_w, s.lattice));
        return;
    }
    let pieces = ((b.x - a.x) / PIECE).ceil().max(1.0) as usize;
    for i in 0..pieces {
        let (f0, f1) = (i as f32 / pieces as f32, (i + 1) as f32 / pieces as f32);
        let (p0, p1) = (a.lerp(b, f0), a.lerp(b, f1));
        let tone = contrast::mix(s.band, s.lattice, fade_at(r, (p0.x + p1.x) / 2.0));
        p.line_segment([p0, p1], Stroke::new(s.lattice_w, tone));
    }
}

/// The trunk with three commits, and a branch that leaves it in a curve and carries two more.
fn vine(p: &Painter, r: Rect, s: &ArtStyle) {
    let at = |fx: f32, fy: f32| pos2(r.left() + fx * r.width(), r.top() + fy * r.height());
    let (trunk_y, branch_y) = (0.62, 0.32);
    let stroke = Stroke::new(s.vine_w, s.vine);
    p.line_segment([at(0.08, trunk_y), at(0.96, trunk_y)], stroke);
    let curve = QuadraticBezierShape::from_points_stroke(
        [at(0.30, trunk_y), at(0.44, trunk_y), at(0.46, branch_y)],
        false,
        Color32::TRANSPARENT,
        stroke,
    );
    p.add(Shape::QuadraticBezier(curve));
    p.line_segment([at(0.46, branch_y), at(0.90, branch_y)], stroke);
    let nodes = [(0.14, trunk_y), (0.50, trunk_y), (0.86, trunk_y), (0.62, branch_y), (0.80, branch_y)];
    for (fx, fy) in nodes {
        let c = at(fx, fy);
        p.circle(c, NODE_R, s.node_fill, Stroke::new(2.0_f32, s.vine));
        p.circle_filled(c, DOT_R, s.vine);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::{self, DARK, HIGH_CONTRAST_DARK, HIGH_CONTRAST_LIGHT, LIGHT, PORCELAIN};
    use contrast::ratio;

    const ALL: [(&str, Palette); 5] = [
        ("Dark", DARK),
        ("Light", LIGHT),
        ("Porcelain", PORCELAIN),
        ("High contrast dark", HIGH_CONTRAST_DARK),
        ("High contrast light", HIGH_CONTRAST_LIGHT),
    ];

    #[test]
    fn only_the_high_contrast_palettes_get_the_high_contrast_drawing() {
        for (name, p) in ALL {
            let hc = name.starts_with("High contrast");
            assert_eq!(art_style(&p).kind == ArtKind::HighContrast, hc, "{name}");
        }
        assert_eq!(art_style(&theme::DARK).kind, ArtKind::Dark);
        assert_eq!(art_style(&theme::LIGHT).kind, ArtKind::Light);
    }

    #[test]
    fn the_band_keeps_text_readable_and_the_vine_visible() {
        for (name, p) in ALL {
            let s = art_style(&p);
            let text_min = if s.kind == ArtKind::HighContrast { 7.0 } else { 4.5 };
            assert!(ratio(p.text, s.band) >= text_min, "{name}: title on the band");
            assert!(ratio(p.weak, s.band) >= 4.5, "{name}: subtitle on the band");
            assert!(ratio(s.vine, s.band) >= 3.0, "{name}: vine on the band");
            if s.kind == ArtKind::HighContrast {
                assert!(ratio(s.lattice, s.band) >= 3.0, "{name}: lattice on the band");
                assert!(s.rule.is_some(), "{name}: a rule under the band");
            } else {
                assert!(ratio(s.lattice, s.band) < ratio(s.vine, s.band), "{name}: the lattice is quieter than the vine");
                assert!(s.rule.is_none(), "{name}: no rule");
            }
        }
    }

    #[test]
    fn the_lattice_fades_out_at_both_ends() {
        let r = Rect::from_min_max(pos2(0.0, 0.0), pos2(300.0, 100.0));
        assert_eq!(fade_at(r, 0.0), 0.0);
        assert_eq!(fade_at(r, 300.0), 0.0);
        assert_eq!(fade_at(r, 150.0), 1.0);
        assert!((fade_at(r, 45.0) - 0.5).abs() < 1e-4);
    }
}
