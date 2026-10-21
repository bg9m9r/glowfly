//! Painting over the terminal's cells: the selection, the cursor in its chosen style, the dim
//! overlay and the history's scroll thumb.

use crate::ui::theme::color;
use eframe::egui::{self, Rect, Stroke, StrokeKind, pos2, vec2};
use glowfly_core::settings::CursorStyle;
use glowfly_core::terminal::TermPoint;

/// The thumb's width and its smallest height.
const THUMB_W: f32 = 3.0;
const THUMB_MIN_H: f32 = 12.0;

/// Highlights `start..end` (reading order) over the view, whose top row is line `top`.
/// `cell` gives a cell's top-left corner.
pub(super) fn selection(
    painter: &egui::Painter,
    (start, end): (TermPoint, TermPoint),
    top: u64,
    (rows, cols): (u16, u16),
    (cw, rh): (f32, f32),
    cell: impl Fn(u16, u16) -> egui::Pos2,
) {
    let fill = color::accent().gamma_multiply(0.35);
    for r in 0..rows {
        let line = top + u64::from(r);
        if line < start.line || line > end.line {
            continue;
        }
        let from = if line == start.line { start.col } else { 0 };
        let to = if line == end.line { end.col } else { cols };
        if to > from {
            painter.rect_filled(Rect::from_min_size(cell(r, from), vec2(f32::from(to - from) * cw, rh)), 0.0, fill);
        }
    }
}

/// The cursor over the cell `block`: filled, a line under it or a bar at its left edge.
/// Without the keyboard a block is an outline and the lines are fainter.
pub(super) fn cursor(painter: &egui::Painter, block: Rect, style: CursorStyle, focused: bool) {
    let line = if focused { color::accent() } else { color::accent().gamma_multiply(0.5) };
    match style {
        CursorStyle::Block if focused => painter.rect_filled(block, 0.0, color::accent().gamma_multiply(0.6)),
        CursorStyle::Block => painter.rect_stroke(block, 0.0, Stroke::new(1.0_f32, color::accent()), StrokeKind::Inside),
        CursorStyle::Underline => painter.rect_filled(Rect::from_min_max(pos2(block.left(), block.bottom() - 2.0), block.max), 0.0, line),
        CursorStyle::Bar => painter.rect_filled(Rect::from_min_size(block.min, vec2(2.0, block.height())), 0.0, line),
    };
}

/// Shades the grid while it doesn't have the keyboard.
pub(super) fn dim(painter: &egui::Painter, rect: Rect) {
    painter.rect_filled(rect, 0.0, color::input().gamma_multiply(0.4));
}

/// Where the view is in the history, along the grid's right edge (only while scrolled back).
pub(super) fn thumb(painter: &egui::Painter, rect: Rect, view: usize, history: usize, rows: u16) {
    if view == 0 {
        return;
    }
    let total = (history + usize::from(rows)) as f32;
    let h = rect.height();
    let len = (f32::from(rows) / total * h).max(THUMB_MIN_H).min(h);
    // 0 at the oldest row, 1 at the screen.
    let at = (history - view.min(history)) as f32 / history.max(1) as f32;
    let top = rect.top() + at * (h - len);
    let r = Rect::from_min_size(pos2(rect.right() - THUMB_W - 1.0, top), vec2(THUMB_W, len));
    painter.rect_filled(r, 1.5, color::weak().gamma_multiply(0.6));
}
