//! The three kinds of message (error, warning, information) and their marks: an error circle, a
//! warning triangle and an information circle, painted as shapes at any size (the fonts lack
//! them). Field and form messages, banners and the error panes all draw them from here.

use super::super::theme::color;
use eframe::egui::{Color32, Painter, Pos2, Shape, Stroke, Vec2, pos2};

/// How serious a message is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    /// The mark's colour: red, amber or blue.
    pub fn color(self) -> Color32 {
        match self {
            Severity::Error => color::red(),
            Severity::Warning => color::amber(),
            Severity::Info => color::blue(),
        }
    }

    /// A field message's text colour: the mark's, but weak for information.
    pub fn text_color(self) -> Color32 {
        match self {
            Severity::Info => color::weak(),
            _ => self.color(),
        }
    }
}

/// The line width of a mark `size` across.
fn line_width(size: f32) -> f32 {
    (size * 0.09).clamp(1.2, 2.2)
}

/// Paints the mark of `s` centred on `c`, `size` across, in `col`.
pub fn paint(p: &Painter, c: Pos2, size: f32, s: Severity, col: Color32) {
    let w = line_width(size);
    let stroke = Stroke::new(w, col);
    let dot = (w * 0.75).max(1.0);
    match s {
        Severity::Error | Severity::Info => {
            let r = size / 2.0 - w / 2.0;
            p.circle_stroke(c, r, stroke);
            // Error: a bar over a dot; information: a dot over a bar.
            let (bar, dot_y) = if s == Severity::Error {
                ((c.y - r * 0.5, c.y + r * 0.12), c.y + r * 0.45)
            } else {
                ((c.y - r * 0.1, c.y + r * 0.5), c.y - r * 0.45)
            };
            p.line_segment([pos2(c.x, bar.0), pos2(c.x, bar.1)], stroke);
            p.circle_filled(pos2(c.x, dot_y), dot, col);
        }
        Severity::Warning => {
            let h = size * 0.88;
            let top = pos2(c.x, c.y - h * 0.55);
            let left = pos2(c.x - size / 2.0 + w / 2.0, c.y + h * 0.45);
            let right = pos2(c.x + size / 2.0 - w / 2.0, c.y + h * 0.45);
            p.add(Shape::closed_line(rounded(&[top, right, left], size * 0.08), stroke));
            p.line_segment([pos2(c.x, c.y - h * 0.18), pos2(c.x, c.y + h * 0.14)], stroke);
            p.circle_filled(pos2(c.x, c.y + h * 0.3), dot, col);
        }
    }
}

/// The polygon `corners` with each corner rounded off over `radius`: a short curve from a point
/// on the edge before the corner to one on the edge after it.
fn rounded(corners: &[Pos2], radius: f32) -> Vec<Pos2> {
    let n = corners.len();
    let mut out = Vec::with_capacity(n * 5);
    for i in 0..n {
        let v = corners[i];
        let toward = |o: Pos2| -> Pos2 {
            let d: Vec2 = o - v;
            v + d.normalized() * radius.min(d.length() / 2.0)
        };
        let (a, b) = (toward(corners[(i + n - 1) % n]), toward(corners[(i + 1) % n]));
        for step in 0..=4 {
            let t = step as f32 / 4.0;
            // A quadratic curve from `a` to `b` pulled toward the corner.
            let q = a.to_vec2() * (1.0 - t) * (1.0 - t) + v.to_vec2() * 2.0 * t * (1.0 - t) + b.to_vec2() * t * t;
            out.push(q.to_pos2());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_corners_stay_near_the_polygon() {
        let pts = rounded(&[pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(5.0, 8.0)], 2.0);
        assert_eq!(pts.len(), 15);
        assert!(pts.iter().all(|p| p.x >= -0.01 && p.x <= 10.01 && p.y >= -0.01 && p.y <= 8.01));
    }
}
