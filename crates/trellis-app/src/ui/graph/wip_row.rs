//! This tab's own WIP row in the message column: "Uncommitted changes", the M / A / D chips and,
//! right-aligned, the working tree's total `+N −M`.

use super::paint::Columns;
use crate::ui::line_counts;
use crate::ui::theme::color;
use crate::ui::widgets::paint_cut_text;
use eframe::egui::{FontId, Painter, pos2};
use trellis_core::repo::{ChangeCounts, LineCount};

/// Between the label and the chips.
const LABEL_GAP: f32 = 10.0;
/// The total's inset from the message column's right edge.
const TOTAL_INSET: f32 = 8.0;
/// The least space between the chips and the total; closer, the total is hidden.
const TOTAL_GAP: f32 = 12.0;

/// Draws the row's message cell, centred on `mid`. `total` is None while the counts aren't known
/// (or are incomplete). When the column is narrow the total goes first, then the label is cut.
pub(super) fn draw(mp: &Painter, cols: &Columns, mid: f32, label: &str, changes: ChangeCounts, total: Option<LineCount>) {
    let ctx = mp.ctx();
    let font = FontId::proportional(13.0);
    let (left, right) = (cols.msg.0, cols.msg.1 - TOTAL_INSET);
    let label_w = ctx.fonts(|f| f.layout_no_wrap(label.to_owned(), font.clone(), color::text()).size().x);
    let chips_w = line_counts::chips_width(ctx, changes);
    let chips_end = left + label_w + LABEL_GAP + chips_w;
    let total = total.filter(|t| chips_end + TOTAL_GAP <= right - line_counts::total_width(ctx, *t));
    let label_max = (right - left - LABEL_GAP - chips_w).max(0.0);
    let cut = paint_cut_text(mp, pos2(left, mid), label, font, color::text(), label_max);
    line_counts::paint_chips(mp, cut.rect.right() + LABEL_GAP, mid, changes);
    if let Some(t) = total {
        line_counts::paint_total_right(mp, pos2(right, mid), t);
    }
}
