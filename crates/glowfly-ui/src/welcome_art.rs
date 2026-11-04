//! The welcome header's drawing (design pass D20): the app icon's own firefly turned on its
//! side across the band, drawn from the icon's geometry (`tests/packaging/app_icon/draw.rs`, in
//! its 1024-unit space), not from an image. The gold lane runs in from the left into the
//! lantern with its glass wings; a rose lane branches off a gold commit and an ice-blue lane off
//! a rose commit, climbing to the right, each ending in a glowing head. Every colour is opaque
//! (a `mix` or `tint` of the band), so nothing stacks: on the dark themes the trail sits 70% of
//! the way from the band to its hue under a full-strength gold, with ringed halos; the light
//! themes draw it opaque in darker hues without halos; high contrast draws everything at full
//! strength on the plain panel, with a rule under the band.

use super::theme::color::Palette;
use super::theme::contrast;
use eframe::egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};

/// The icon's hues (`draw.rs`), and the light and high-contrast light trail's own.
const GOLD: Color32 = Color32::from_rgb(0xdf, 0xba, 0x3b);
const ROSE: Color32 = Color32::from_rgb(0xfb, 0x97, 0xdb);
const ICE: Color32 = Color32::from_rgb(0x93, 0xc1, 0xfe);
const ROSE_LIGHT: Color32 = Color32::from_rgb(0xb4, 0x50, 0x8f);
const ICE_LIGHT: Color32 = Color32::from_rgb(0x3a, 0x7c, 0xd0);
/// High contrast light's firefly: the light themes' `lantern` orange (its own `lantern` is text).
const ORANGE_HC_LIGHT: Color32 = Color32::from_rgb(0xcc, 0x59, 0x0d);
const ROSE_HC_LIGHT: Color32 = Color32::from_rgb(0x95, 0x3d, 0x7c);
const ICE_HC_LIGHT: Color32 = Color32::from_rgb(0x24, 0x62, 0xb1);
/// How far the dark themes' trail goes from the band to its hue.
const TRAIL: f32 = 0.70;

/// The map from the icon's units: one icon unit across the band's height, and the top inset of
/// the ice lane's row (rows: ice 24, rose 44.7, gold 76 px down a 104 px band).
pub(super) const SV: f32 = 0.115;
const TOP: f32 = 24.0;
/// The ice head ends this far from the art's right edge; the icon's units the art spans across.
const RIGHT: f32 = 24.0;
const SPAN: f32 = 750.0;
/// The gold lane fades in over this much of the art's left edge, in pieces this long.
const FADE_IN: f32 = 32.0;
const PIECE: f32 = 4.0;

/// The lanes, in icon units: the gold trunk, then each branch's elbow (from, the elbow's control
/// height, the turn) and where it ends.
const GOLD_LANE: [(f32, f32); 2] = [(340.0, 1000.0), (340.0, 664.0)];
const ROSE_LANE: ((f32, f32), f32, (f32, f32), (f32, f32)) = ((340.0, 858.0), 774.0, (612.0, 690.0), (612.0, 452.0));
const ICE_LANE: ((f32, f32), f32, (f32, f32), (f32, f32)) = ((612.0, 616.0), 537.0, (792.0, 458.0), (792.0, 250.0));
/// The commits: where, which lane (0 gold, 1 rose, 2 ice) and the dot's radius in px.
const COMMITS: [((f32, f32), usize, f32); 5] =
    [((340.0, 858.0), 0, 4.0), ((612.0, 616.0), 1, 4.0), ((612.0, 534.0), 1, 3.3), ((792.0, 458.0), 2, 4.0), ((792.0, 356.0), 2, 3.3)];
/// The band-coloured ring that sets a commit dot off from its lane.
const KNOCK_OUT: f32 = 1.5;
/// The dark themes' pale commit centres (the icon's tints), as a part of the dot.
const PALE: [Color32; 3] = [Color32::from_rgb(0xff, 0xf1, 0xc4), Color32::from_rgb(0xff, 0xf0, 0xfa), Color32::from_rgb(0xee, 0xf5, 0xff)];
const PALE_R: f32 = 0.46;
/// The heads (rose, ice): where, and the icon's core radius; drawn 1.2 times as large.
pub(super) const HEADS: [((f32, f32), f32); 2] = [((612.0, 452.0), 40.0), ((792.0, 250.0), 44.0)];
const HEAD_SCALE: f32 = 1.2;
/// A head's mid tint and centre on the dark themes (rose, ice).
const HEAD_MID: [Color32; 2] = [Color32::from_rgb(0xfd, 0xc4, 0xea), Color32::from_rgb(0xc3, 0xdc, 0xff)];
const HEAD_CENTRE: [Color32; 2] = [Color32::from_rgb(0xff, 0xf3, 0xfb), Color32::from_rgb(0xf4, 0xf9, 0xff)];
/// A head's halo rings on the dark themes: (radius × core, tint toward the hue), outermost first.
pub(super) const HEAD_HALO: [(f32, f32); 2] = [(2.2, 0.06), (1.6, 0.12)];
/// The lantern's frame (`draw.rs` `LIGHT`: translate(340 664) rotate(28) scale(1.1)) and its
/// ellipse's radii in the frame's units.
const LANTERN_AT: (f32, f32) = (340.0, 664.0);
const LANTERN_TURN: f32 = 28.0;
const LANTERN_SCALE: f32 = 1.1;
const LANTERN_R: (f32, f32) = (100.0, 132.0);
/// The dark themes' lantern: concentric ellipses (scale, colour), and its halo rings (scale,
/// tint toward gold), outermost first.
const LANTERN_TONES: [(f32, Color32); 3] =
    [(1.0, Color32::from_rgb(0xf4, 0xb1, 0x4d)), (0.72, Color32::from_rgb(0xff, 0xe6, 0xb0)), (0.40, Color32::from_rgb(0xff, 0xfb, 0xf1))];
pub(super) const LANTERN_HALO: [(f32, f32); 3] = [(1.7, 0.08), (1.45, 0.14), (1.2, 0.22)];
/// Segments per ellipse and per wing curve.
const ELLIPSE_STEPS: usize = 32;
const CURVE_STEPS: usize = 24;

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
    /// A 1 px line under the band (high contrast).
    pub rule: Option<Color32>,
    /// The firefly (its lane, commit and lantern) and the two trailing lanes.
    pub gold: Color32,
    pub rose: Color32,
    pub ice: Color32,
    pub lane_w: f32,
    /// The wings' fill (none in high contrast) and their 1 px edge.
    pub wing_fill: Option<Color32>,
    pub wing_edge: Color32,
}

impl ArtStyle {
    /// The halos, pale centres and toned lantern: the dark drawing only.
    pub(super) fn glows(&self) -> bool {
        self.kind == ArtKind::Dark
    }
}

/// Whether `p` is a high-contrast palette: its hairlines already stand out from the panel.
pub(super) fn high_contrast(p: &Palette) -> bool {
    contrast::ratio(p.border, p.panel) >= 3.0
}

/// The drawing's style for `p`.
pub(super) fn art_style(p: &Palette) -> ArtStyle {
    if high_contrast(p) {
        let (gold, rose, ice) = if p.dark { (GOLD, ROSE, ICE) } else { (ORANGE_HC_LIGHT, ROSE_HC_LIGHT, ICE_HC_LIGHT) };
        return ArtStyle {
            kind: ArtKind::HighContrast,
            band: p.panel,
            rule: Some(p.border),
            gold,
            rose,
            ice,
            lane_w: 3.0,
            wing_fill: None,
            wing_edge: p.text,
        };
    }
    let (kind, tint, wing) = if p.dark { (ArtKind::Dark, 0.10, 0.12) } else { (ArtKind::Light, 0.08, 0.06) };
    let band = contrast::mix(p.panel, p.accent, tint);
    let (gold, rose, ice) = match kind {
        ArtKind::Dark => (GOLD, contrast::mix(band, ROSE, TRAIL), contrast::mix(band, ICE, TRAIL)),
        _ => (p.lantern, ROSE_LIGHT, ICE_LIGHT),
    };
    ArtStyle {
        kind,
        band,
        rule: None,
        gold,
        rose,
        ice,
        lane_w: 2.5,
        wing_fill: Some(contrast::mix(band, p.text, wing)),
        wing_edge: contrast::mix(band, p.text, 0.5),
    }
}

/// The map from the icon's units into the art's rect, laid sideways: the icon's y runs left to
/// right (its bottom at the left edge), its x down the band (its right edge at the top).
#[derive(Clone, Copy, Debug)]
pub(super) struct Map {
    left: f32,
    top: f32,
    su: f32,
}

impl Map {
    pub(super) fn new(r: Rect) -> Self {
        Self { left: r.left(), top: r.top(), su: (r.width() - RIGHT) / SPAN }
    }

    /// An icon point in the art.
    pub(super) fn at(&self, (x, y): (f32, f32)) -> Pos2 {
        pos2(self.left + (1000.0 - y) * self.su, self.top + TOP + (792.0 - x) * SV)
    }

    /// An icon offset as a head shape takes it: at one scale both ways, so it keeps its shape.
    fn shape(&self, (dx, dy): (f32, f32)) -> Vec2 {
        vec2(-dy * SV, -dx * SV)
    }
}

/// A cubic from `p0` to `p3`, `steps` points after `p0`.
fn cubic(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2, steps: usize) -> impl Iterator<Item = Pos2> {
    (1..=steps).map(move |i| {
        let t = i as f32 / steps as f32;
        let u = 1.0 - t;
        let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        pos2(a * p0.x + b * p1.x + c * p2.x + d * p3.x, a * p0.y + b * p1.y + c * p2.y + d * p3.y)
    })
}

/// A branch lane: the app's S-shaped elbow from `from` over to `turn` (both control points at
/// the elbow's height `my`), then straight on to `end`.
fn branch(m: &Map, (from, my, turn, end): ((f32, f32), f32, (f32, f32), (f32, f32))) -> Vec<Pos2> {
    let mut points = vec![m.at(from)];
    points.extend(cubic(m.at(from), m.at((from.0, my)), m.at((turn.0, my)), m.at(turn), 32));
    points.push(m.at(end));
    points
}

/// A point of the lantern frame (`l` in its units), as an offset in the art.
fn lantern_offset(m: &Map, l: (f32, f32)) -> Vec2 {
    let (sin, cos) = LANTERN_TURN.to_radians().sin_cos();
    let d = (LANTERN_SCALE * (l.0 * cos - l.1 * sin), LANTERN_SCALE * (l.0 * sin + l.1 * cos));
    m.shape(d)
}

/// The lantern's ellipse at `scale`, as a closed polygon in the art.
pub(super) fn lantern_ellipse(m: &Map, scale: f32) -> Vec<Pos2> {
    let c = m.at(LANTERN_AT);
    (0..ELLIPSE_STEPS)
        .map(|i| {
            let a = i as f32 / ELLIPSE_STEPS as f32 * std::f32::consts::TAU;
            c + lantern_offset(m, (scale * LANTERN_R.0 * a.cos(), scale * LANTERN_R.1 * a.sin()))
        })
        .collect()
}

/// One glass wing (`draw.rs` `wing`, mirrored for the right one), as a closed polygon.
fn wing(m: &Map, mirror: bool) -> Vec<Pos2> {
    let s = if mirror { -1.0 } else { 1.0 };
    let c = m.at(LANTERN_AT);
    let q = |x: f32, y: f32| c + lantern_offset(m, (x * s * 0.68, y * 0.68 - 40.0));
    let mut points = vec![q(-24.0, -104.0)];
    let curves = [
        [(-90.0, -190.0), (-200.0, -236.0), (-236.0, -180.0)],
        [(-258.0, -144.0), (-214.0, -96.0), (-150.0, -74.0)],
        [(-100.0, -58.0), (-56.0, -62.0), (-24.0, -74.0)],
    ];
    for [c1, c2, end] in curves {
        let start = points[points.len() - 1];
        points.extend(cubic(start, q(c1.0, c1.1), q(c2.0, c2.1), q(end.0, end.1), CURVE_STEPS));
    }
    points
}

/// A head's centre and radius in the art.
pub(super) fn head(m: &Map, i: usize) -> (Pos2, f32) {
    let (at, core) = HEADS[i];
    (m.at(at), core * SV * HEAD_SCALE)
}

/// Paints the drawing into `r` (the painter clips to it).
pub(super) fn paint(p: &Painter, r: Rect, s: &ArtStyle) {
    let m = Map::new(r);
    let hues = [s.gold, s.rose, s.ice];
    if s.glows() {
        for &(scale, k) in &LANTERN_HALO {
            p.add(Shape::convex_polygon(lantern_ellipse(&m, scale), contrast::tint(s.band, GOLD, k, true), Stroke::NONE));
        }
        for (i, hue) in [ROSE, ICE].into_iter().enumerate() {
            let (c, radius) = head(&m, i);
            for &(scale, k) in &HEAD_HALO {
                p.circle_filled(c, radius * scale, contrast::tint(s.band, hue, k, true));
            }
        }
    }
    gold_lane(p, &m, r, s);
    for (lane, hue) in [(ROSE_LANE, s.rose), (ICE_LANE, s.ice)] {
        p.add(Shape::line(branch(&m, lane), Stroke::new(s.lane_w, hue)));
    }
    for (at, lane, radius) in COMMITS {
        let c = m.at(at);
        p.circle_filled(c, radius + KNOCK_OUT, s.band);
        p.circle_filled(c, radius, hues[lane]);
        if s.glows() {
            p.circle_filled(c, radius * PALE_R, PALE[lane]);
        }
    }
    for mirror in [false, true] {
        let w = wing(&m, mirror);
        if let Some(fill) = s.wing_fill {
            p.add(fan(&w, fill));
        }
        p.add(Shape::closed_line(w, Stroke::new(1.0_f32, s.wing_edge)));
    }
    lantern(p, &m, s);
    for i in 0..2 {
        let (c, radius) = head(&m, i);
        p.circle_filled(c, radius, hues[i + 1]);
        if s.glows() {
            p.circle_filled(c, radius * 0.7, HEAD_MID[i]);
            p.circle_filled(c, radius * 0.4, HEAD_CENTRE[i]);
        }
    }
}

/// The gold trunk, fading in from the band over the art's first `FADE_IN` px.
fn gold_lane(p: &Painter, m: &Map, r: Rect, s: &ArtStyle) {
    let (a, b) = (m.at(GOLD_LANE[0]), m.at(GOLD_LANE[1]));
    let fade_end = (a.x + FADE_IN).min(b.x);
    let mut x = a.x;
    while x < fade_end {
        let next = (x + PIECE).min(fade_end);
        let tone = contrast::mix(s.band, s.gold, ((x + next) / 2.0 - r.left()) / FADE_IN);
        p.line_segment([pos2(x, a.y), pos2(next, a.y)], Stroke::new(s.lane_w, tone));
        x = next;
    }
    p.line_segment([pos2(fade_end, a.y), b], Stroke::new(s.lane_w, s.gold));
}

/// The lantern: toned ellipses on the dark themes, one solid ellipse elsewhere.
fn lantern(p: &Painter, m: &Map, s: &ArtStyle) {
    if s.glows() {
        for (scale, tone) in LANTERN_TONES {
            p.add(Shape::convex_polygon(lantern_ellipse(m, scale), tone, Stroke::NONE));
        }
    } else {
        p.add(Shape::convex_polygon(lantern_ellipse(m, 1.0), s.gold, Stroke::NONE));
    }
}

/// A filled polygon as a fan from its centroid: the wings bend, which `convex_polygon` can't
/// fill, but every edge can be seen from the centroid.
fn fan(points: &[Pos2], fill: Color32) -> Shape {
    let n = points.len() as f32;
    let centre = points.iter().fold(Vec2::ZERO, |acc, p| acc + p.to_vec2()) / n;
    let mut mesh = Mesh::default();
    mesh.colored_vertex(centre.to_pos2(), fill);
    for p in points {
        mesh.colored_vertex(*p, fill);
    }
    let len = points.len() as u32;
    for i in 0..len {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % len);
    }
    Shape::mesh(mesh)
}

#[cfg(test)]
#[path = "welcome_art_tests.rs"]
mod tests;
