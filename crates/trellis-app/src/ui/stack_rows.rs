//! Drawing the LOCAL section's stacks: each stacked row indented one step per layer, and a thin
//! elbow from its lower branch's dot down to its own, painted as lines (the bundled fonts have
//! no box-drawing glyphs).

use super::branch_dot::DOT_R;
use super::widgets::ROW_INDENT;
use eframe::egui::{Color32, Painter, Rect, Stroke, pos2};
use std::collections::HashMap;

/// The indentation of one layer.
pub const STEP: f32 = 16.0;

/// How far a row `depth` layers up is indented.
pub fn indent(depth: usize) -> f32 {
    STEP * depth as f32
}

/// The vertical centres of the rows drawn so far, by branch, so a stacked row can reach up to
/// its lower branch's dot (also when that row is scrolled out of view).
#[derive(Default)]
pub struct Elbows(HashMap<String, f32>);

impl Elbows {
    pub fn note(&mut self, name: &str, centre_y: f32) {
        self.0.insert(name.to_owned(), centre_y);
    }

    /// Paints the elbow of `row`, `depth` layers up, from `lower`'s dot (or the row's top when
    /// that row is unknown) down and across to the row's own dot.
    pub fn paint(&self, p: &Painter, row: Rect, depth: usize, lower: Option<&str>, col: Color32) {
        if depth == 0 {
            return;
        }
        let x = row.left() + ROW_INDENT + indent(depth - 1) + DOT_R;
        let top = lower.and_then(|l| self.0.get(l)).map_or(row.top(), |y| y + DOT_R + 1.0);
        let (cy, end) = (row.center().y, row.left() + ROW_INDENT + indent(depth) - 2.0);
        let s = Stroke::new(1.0_f32, col);
        p.line_segment([pos2(x, top), pos2(x, cy)], s);
        p.line_segment([pos2(x, cy), pos2(end, cy)], s);
    }
}
