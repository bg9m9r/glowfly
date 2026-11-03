//! Drawing helpers for the commit graph: lanes (with their widths and dash patterns) and the
//! details panel's avatar.

use super::LANE_W;
use super::lane_colours::RowColours;
use crate::ui::avatars::Avatars;
use crate::ui::theme::branch_colours::initials_tones;
use crate::ui::widgets;
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Rect, Stroke, epaint::CubicBezierShape, epaint::RectShape, pos2, vec2};
use glowfly_core::graph::GraphRow;

/// The centre of `lane` in a GRAPH column whose content starts at `graph_x`.
pub fn lane_x(graph_x: f32, lane: usize) -> f32 {
    graph_x + lane as f32 * LANE_W + LANE_W / 2.0
}

/// How a lane line is dashed. The colour-blind safe colours give lanes away from HEAD's a
/// pattern by their distance from it, as hues alone may look alike.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    Solid,
    /// 6 px on, 4 off.
    Dashed,
    /// 2 px on, 3 off.
    Dotted,
    /// 6 on, 3 off, 2 on, 3 off.
    DashDot,
}

impl Pattern {
    /// The pattern of a lane `d` lanes from HEAD's: solid on it, then dashed, dotted and
    /// dash-dot, again from the fourth.
    pub fn for_distance(d: usize) -> Self {
        match d {
            0 => Self::Solid,
            d if d % 3 == 1 => Self::Dashed,
            d if d % 3 == 2 => Self::Dotted,
            _ => Self::DashDot,
        }
    }

    /// Its on and off lengths in px, starting with on.
    fn steps(self) -> &'static [f32] {
        match self {
            Self::Solid => &[],
            Self::Dashed => &[6.0, 4.0],
            Self::Dotted => &[2.0, 3.0],
            Self::DashDot => &[6.0, 3.0, 2.0, 3.0],
        }
    }
}

/// How one lane line is drawn: its colour, width (2.5 px for HEAD's branch, 2 px otherwise) and
/// pattern, which runs from `origin`, the top of the graph's content on screen.
#[derive(Clone, Copy, Debug)]
pub struct LaneLine {
    pub colour: Color32,
    pub width: f32,
    pub pattern: Pattern,
    pub origin: f32,
}

fn edge(p: &egui::Painter, a: egui::Pos2, b: egui::Pos2, line: LaneLine) {
    let stroke = Stroke::new(line.width, line.colour);
    let straight = (a.x - b.x).abs() < 0.5;
    // The pattern's phase at `a`: how far down the graph's content it is, so a lane's dashes
    // run on across rows.
    let phase = a.y - line.origin;
    let mut draw = |seg: [egui::Pos2; 2]| {
        p.line_segment(seg, stroke);
    };
    match (straight, line.pattern) {
        (true, Pattern::Solid) => {
            p.line_segment([a, b], stroke);
        }
        (true, pattern) => dashes(&[a, b], pattern.steps(), phase, &mut draw),
        (false, pattern) => {
            let my = (a.y + b.y) / 2.0;
            let curve = CubicBezierShape::from_points_stroke([a, pos2(a.x, my), pos2(b.x, my), b], false, Color32::TRANSPARENT, stroke);
            if pattern == Pattern::Solid {
                p.add(curve);
            } else {
                dashes(&curve.flatten(Some(0.25)), pattern.steps(), phase, &mut draw);
            }
        }
    }
}

/// Hands `emit` each "on" piece of the path through `points` dashed by `steps` (on, off, on,
/// ...), the pattern already `phase` px along at the first point.
fn dashes(points: &[egui::Pos2], steps: &[f32], phase: f32, emit: &mut impl FnMut([egui::Pos2; 2])) {
    let period: f32 = steps.iter().sum();
    // Where in the pattern the path starts: step `k`, with `left` px of it to go.
    let mut at = phase.rem_euclid(period);
    let mut k = 0;
    while at >= steps[k] {
        at -= steps[k];
        k = (k + 1) % steps.len();
    }
    let mut left = steps[k] - at;
    for w in points.windows(2) {
        let (mut from, to) = (w[0], w[1]);
        let mut len = from.distance(to);
        while len > 0.0 {
            let step = left.min(len);
            let next = from + (to - from) * (step / len);
            if k % 2 == 0 && step > 0.0 {
                emit([from, next]);
            }
            from = next;
            len -= step;
            left -= step;
            if left <= 1e-4 {
                k = (k + 1) % steps.len();
                left = steps[k];
            }
        }
    }
}

/// Draws row `g`'s lane lines in `rect`, each in the colour of the branch owning it there.
pub(super) fn draw_lanes(p: &egui::Painter, graph_x: f32, rect: Rect, g: &GraphRow, colours: &RowColours) {
    let (top, mid, bottom) = (rect.top(), rect.center().y, rect.bottom());
    let x = |i| lane_x(graph_x, i);
    for i in g.through.iter() {
        edge(p, pos2(x(i), top), pos2(x(i), bottom), colours.line_above(i, i));
    }
    // A curve takes the pattern of the lane it ends in.
    for &(i, c) in &g.top {
        edge(p, pos2(x(i), top), pos2(x(c), mid), colours.line_above(i, c));
    }
    for &(c, j) in &g.bottom {
        edge(p, pos2(x(c), mid), pos2(x(j), bottom), colours.line_below(j));
    }
}

/// An author avatar inside a `ring` (the branch's colour): the image when available, otherwise
/// initials in the person's colours (`initials_tones`).
pub fn draw_avatar(p: &egui::Painter, c: egui::Pos2, r: f32, name: &str, email: &str, ring: Color32, avatars: Option<&mut Avatars>) {
    p.circle_filled(c, r, ring);
    let inner = r - 2.0;
    if let Some(tex) = avatars.and_then(|a| a.get(email)) {
        let rect = Rect::from_center_size(c, vec2(inner * 2.0, inner * 2.0));
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        p.add(RectShape::filled(rect, CornerRadius::same(inner as u8), Color32::WHITE).with_texture(tex.id(), uv));
    } else {
        let (fill, ink) = initials_tones(email, name);
        p.circle_filled(c, inner, fill);
        p.text(c, Align2::CENTER_CENTER, widgets::initials(name), FontId::proportional(r * 0.85), ink);
    }
}

pub fn paint_pencil(p: &egui::Painter, c: egui::Pos2, col: Color32) {
    let s = Stroke::new(2.0_f32, col);
    p.line_segment([pos2(c.x - 4.0, c.y + 4.0), pos2(c.x + 4.0, c.y - 4.0)], s);
    p.line_segment([pos2(c.x - 5.0, c.y + 5.0), pos2(c.x - 3.0, c.y + 5.0)], Stroke::new(1.5_f32, col));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lanes_take_patterns_by_their_distance_from_head() {
        let p: Vec<Pattern> = (0..8).map(Pattern::for_distance).collect();
        use Pattern::*;
        assert_eq!(p, [Solid, Dashed, Dotted, DashDot, Dashed, Dotted, DashDot, Dashed]);
    }

    #[test]
    fn dashes_run_on_from_the_phase() {
        let line = [pos2(0.0, 0.0), pos2(0.0, 20.0)];
        let ys = |phase| {
            let mut out = Vec::new();
            dashes(&line, Pattern::Dashed.steps(), phase, &mut |s: [egui::Pos2; 2]| out.push((s[0].y, s[1].y)));
            out
        };
        assert_eq!(ys(0.0), [(0.0, 6.0), (10.0, 16.0)]);
        // 3 px into the first dash: its last 3 px, then the next dash 4 px after.
        assert_eq!(ys(3.0), [(0.0, 3.0), (7.0, 13.0), (17.0, 20.0)]);
        // The next row down (20 px on) goes on where this one stopped: its last gap ended the row.
        let (a, b) = (ys(0.0), ys(20.0));
        assert_eq!((a.last().unwrap().1, b.first().copied()), (16.0, Some((0.0, 6.0))), "4 px of gap, then a new dash");
    }
}
