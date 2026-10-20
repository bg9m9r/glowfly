//! The empty states' icons (`empty_state.rs`): 40 px outlines painted as shapes in the weak
//! colour. The page icons mark what is (not) inside a file; the others stand for the pane that
//! is empty.

use eframe::egui::{self, Color32, CornerRadius, Painter, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};

/// What the icon shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyIcon {
    /// A page of bits: a binary file.
    Binary,
    /// A page of dotted rows: only whitespace changed.
    Whitespace,
    /// A page of plain rows: nothing changed in the text, or an empty file.
    File,
    /// A magnifier: nothing matches a search or filter.
    Search,
    /// A folder: no repositories.
    Repos,
    /// A tray with an arrow into it: nothing waiting.
    Inbox,
    /// Three commits on a line: no commits.
    Commits,
    /// A clock face with an arrow turning back: no journal states.
    Journal,
}

/// Paints `icon` centred on `c` in `col`.
pub(super) fn paint(p: &Painter, c: Pos2, icon: EmptyIcon, col: Color32) {
    let s = Stroke::new(1.6_f32, col);
    match icon {
        EmptyIcon::Binary | EmptyIcon::Whitespace | EmptyIcon::File => page(p, c, icon, col),
        EmptyIcon::Search => {
            let lens = pos2(c.x - 4.0, c.y - 4.0);
            p.circle_stroke(lens, 11.0, s);
            let d = std::f32::consts::FRAC_1_SQRT_2;
            p.line_segment([lens + vec2(11.0 * d + 1.0, 11.0 * d + 1.0), pos2(c.x + 15.0, c.y + 15.0)], Stroke::new(2.6_f32, col));
        }
        EmptyIcon::Repos => {
            let (l, r, t, b) = (c.x - 18.0, c.x + 18.0, c.y - 13.0, c.y + 14.0);
            let tab = vec![pos2(l, t + 5.0), pos2(l, t), pos2(l + 12.0, t), pos2(l + 16.0, t + 5.0)];
            p.add(egui::Shape::line(tab, s));
            let body = Rect::from_min_max(pos2(l, t + 5.0), pos2(r, b));
            p.rect_stroke(body, CornerRadius::same(2), s, StrokeKind::Middle);
        }
        EmptyIcon::Inbox => {
            let (l, r, t, b) = (c.x - 17.0, c.x + 17.0, c.y - 2.0, c.y + 15.0);
            // The tray: sloped sides, a floor, and a dip where things land.
            let tray = vec![pos2(l + 6.0, t), pos2(l, t + 9.0), pos2(l, b), pos2(r, b), pos2(r, t + 9.0), pos2(r - 6.0, t)];
            p.add(egui::Shape::line(tray, s));
            let dip = vec![
                pos2(l, t + 9.0),
                pos2(c.x - 7.0, t + 9.0),
                pos2(c.x - 5.0, t + 12.0),
                pos2(c.x + 5.0, t + 12.0),
                pos2(c.x + 7.0, t + 9.0),
                pos2(r, t + 9.0),
            ];
            p.add(egui::Shape::line(dip, s));
            p.line_segment([pos2(c.x, c.y - 18.0), pos2(c.x, c.y + 4.0)], s);
            p.add(egui::Shape::line(vec![pos2(c.x - 5.0, c.y - 1.0), pos2(c.x, c.y + 4.0), pos2(c.x + 5.0, c.y - 1.0)], s));
        }
        EmptyIcon::Commits => {
            let r = 4.5;
            let ys = [c.y - 13.0, c.y, c.y + 13.0];
            for pair in ys.windows(2) {
                p.line_segment([pos2(c.x, pair[0] + r), pos2(c.x, pair[1] - r)], s);
            }
            p.line_segment([pos2(c.x, c.y - 19.0), pos2(c.x, ys[0] - r)], s);
            p.line_segment([pos2(c.x, ys[2] + r), pos2(c.x, c.y + 19.0)], s);
            for y in ys {
                p.circle_stroke(pos2(c.x, y), r, s);
            }
        }
        EmptyIcon::Journal => clock_back(p, c, s),
    }
}

/// A clock face with an open arc around it, its arrowhead pointing counter-clockwise: going
/// back in time.
fn clock_back(p: &Painter, c: Pos2, s: Stroke) {
    use std::f32::consts::{FRAC_PI_2, PI};
    p.circle_stroke(c, 11.0, s);
    p.line_segment([c, pos2(c.x, c.y - 7.0)], s);
    p.line_segment([c, pos2(c.x + 5.0, c.y + 2.0)], s);
    // Screen angles (y down): the arc runs from just past the top, clockwise round to the left.
    let r = 18.0;
    let at = |a: f32| pos2(c.x + r * a.cos(), c.y + r * a.sin());
    let (from, to) = (-FRAC_PI_2 + 0.5, PI + 0.35);
    let steps = 28;
    let arc: Vec<Pos2> = (0..=steps).map(|i| at(from + (to - from) * i as f32 / steps as f32)).collect();
    p.add(egui::Shape::line(arc, s));
    // The arrowhead at the arc's start, pointing back along it (counter-clockwise).
    let tip = at(from);
    let back = vec2(from.sin(), -from.cos());
    let side = vec2(from.cos(), from.sin());
    let head = [tip + back * -5.0 + side * 4.0, tip, tip + back * -5.0 - side * 4.0];
    p.add(egui::Shape::line(head.to_vec(), s));
}

/// A 30 x 38 page with its top-right corner folded, centred on `c`, and its mark.
fn page(p: &Painter, c: Pos2, icon: EmptyIcon, col: Color32) {
    let s = Stroke::new(1.6_f32, col);
    let (l, r, t, b, fold) = (c.x - 15.0, c.x + 15.0, c.y - 19.0, c.y + 19.0, 9.0);
    let outline = vec![pos2(l, t), pos2(r - fold, t), pos2(r, t + fold), pos2(r, b), pos2(l, b)];
    p.add(egui::Shape::closed_line(outline, s));
    p.add(egui::Shape::line(vec![pos2(r - fold, t), pos2(r - fold, t + fold), pos2(r, t + fold)], s));
    let thin = Stroke::new(1.4_f32, col);
    match icon {
        EmptyIcon::Binary => {
            // Two rows of bits: a 1 is an upright bar, a 0 a narrow ring.
            for (row, bits) in [(c.y - 3.0, [true, false, true]), (c.y + 9.0, [false, true, true])] {
                for (i, one) in bits.into_iter().enumerate() {
                    let x = c.x - 7.0 + i as f32 * 7.0;
                    if one {
                        p.line_segment([pos2(x, row - 4.0), pos2(x, row + 4.0)], thin);
                    } else {
                        let ring = Rect::from_center_size(pos2(x, row), vec2(5.0, 8.0));
                        p.rect_stroke(ring, CornerRadius::same(3), thin, StrokeKind::Middle);
                    }
                }
            }
        }
        EmptyIcon::Whitespace => {
            // Rows of spaces, as an editor marks them: dots.
            for (row, n) in [(c.y - 4.0, 5), (c.y + 3.0, 3), (c.y + 10.0, 4)] {
                for i in 0..n {
                    p.circle_filled(pos2(l + 7.0 + i as f32 * 4.0, row), 1.1, col);
                }
            }
        }
        _ => {
            for (row, len) in [(c.y - 4.0, 16.0), (c.y + 3.0, 12.0), (c.y + 10.0, 14.0)] {
                p.line_segment([pos2(l + 7.0, row), pos2(l + 7.0 + len, row)], thin);
            }
        }
    }
}
