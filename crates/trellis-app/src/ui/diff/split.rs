//! The split layout: the old side on the left, the new side on the right, with removed and
//! added lines paired by similarity (see `trellis_core::line_pairs`) and a divider between the
//! columns.

use super::style::DiffStyle;
use super::syntax::paint_line;
use super::wrap::{RowMetrics, Wrap};
use super::{DiffView, drag_over, fold, hunk_header, line_click, line_sense, paint_mark, thread_marks};
use crate::msg::Msg;
use crate::ui::theme::color;
use eframe::egui::{Align2, Color32, Rect, Sense, Stroke, Ui, pos2, vec2};
use std::ops::Range;
use trellis_core::diff_rows::{LineSide, SplitRow};
use trellis_core::highlight::Span;
use trellis_core::repo::DiffLine;

/// A line with its syntax spans and changed-word ranges.
type CellLine<'a> = (&'a DiffLine, &'a [Span], &'a [Range<u32>]);

#[derive(Clone, Copy, PartialEq, Hash)]
enum Side {
    Old,
    New,
}

/// Draws the rows and returns the first visible one and how many rows fit.
/// `rows`, `widest` (in columns) and the rows' heights `m` come from the rows cache.
pub(super) fn show(
    ui: &mut Ui,
    v: &DiffView,
    rows: &[SplitRow],
    widest: usize,
    m: RowMetrics,
    scroll_to: Option<usize>,
    msgs: &mut Vec<Msg>,
) -> (usize, usize) {
    let diff = v.lines();
    // In the whole-file view `hunk` is always 0.
    let st = &v.style;
    let char_w = ui.fonts(|f| f.glyph_width(&st.font, 'M'));
    // Each column is wide enough for the longest line on either side, unless lines wrap to fit.
    let width = if v.wrap.is_some() {
        ui.available_width()
    } else {
        ui.available_width().max(2.0 * (st.split_cols().text + widest as f32 * char_w + 20.0))
    };
    let half = (width / 2.0).round();

    super::wrap::scroll(ui, "diff", rows.len(), m, scroll_to, |ui, i, h| {
        let (rect, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
        match rows[i] {
            // During a line drag the header counts as the line before the hunk's first.
            SplitRow::Header(h) => {
                drag_over(ui, v, rect, h, 0, msgs);
                hunk_header(ui, rect, v, h, msgs)
            }
            // A change block's action row acts on its normal hunk, like that hunk's header.
            SplitRow::Block(h) => hunk_header(ui, rect, v, h, msgs),
            // Spans both columns, with no divider, like an action row.
            SplitRow::Fold(f) => {
                let cols = st.split_cols();
                fold::fold_row(ui, rect, v, f, cols.sign, cols.text, msgs)
            }
            // An opened gap's line: the same text on both sides, like a context line.
            SplitRow::GapLine { hunk, line } => {
                drag_over(ui, v, rect, hunk, 0, msgs);
                let Some((g, text)) = v.gap_lines.iter().find(|(g, _)| g.hunk == hunk) else { return };
                let Some(raw) = text.lines.get(line) else { return };
                let n = line as u32;
                let dl = DiffLine { origin: ' ', old: Some(g.old_start + n), new: Some(g.new_start + n), text: raw.clone() };
                let spans: &[Span] = if v.hl.is_some() { text.spans.get(line).map_or(&[], Vec::as_slice) } else { &[] };
                let left = Rect::from_min_max(rect.min, pos2(rect.left() + half, rect.bottom()));
                let right = Rect::from_min_max(pos2(left.right(), rect.top()), rect.max);
                cell(ui, left, Some((&dl, spans, &[])), Side::Old, st, v.wrap, false);
                cell(ui, right, Some((&dl, spans, &[])), Side::New, st, v.wrap, false);
                ui.painter().vline(left.right(), rect.y_range(), Stroke::new(1.0_f32, color::border()));
                if line == 0 {
                    fold::fold_back_key(ui, left, v, g.key, g.len, st.split_cols().sign, msgs);
                }
            }
            SplitRow::Pair { hunk, old, new } => {
                let lines = &diff.hunks[hunk].lines;
                let left = Rect::from_min_max(rect.min, pos2(rect.left() + half, rect.bottom()));
                let right = Rect::from_min_max(pos2(left.right(), rect.top()), rect.max);
                if let Some(idx) = dragged_side(ui, left, right, old, new) {
                    drag_over(ui, v, rect, hunk, idx, msgs);
                }
                let spans = |i: usize| -> &[Span] {
                    match (lines[i].origin, v.hl) {
                        ('<' | '>' | '=', _) | (_, None) => &[],
                        (_, Some(h)) => h.line(hunk, i),
                    }
                };
                for (side, r, idx) in [(Side::Old, left, old), (Side::New, right, new)] {
                    let Some(idx) = idx else {
                        cell(ui, r, None, side, st, v.wrap, false);
                        continue;
                    };
                    let dl = &lines[idx];
                    // Only a removed line on the left or an added line on the right is selectable.
                    let origin = match (dl.origin, side) {
                        ('-', Side::Old) | ('+', Side::New) => dl.origin,
                        _ => ' ',
                    };
                    let selected = origin != ' ' && v.is_selected(hunk, idx);
                    let hovered = v.sel.is_some() && {
                        let resp = ui.interact(r, ui.id().with(("diff-cell", hunk, idx, side)), line_sense(origin));
                        line_click(ui, &resp, hunk, idx, origin, msgs)
                    };
                    cell(ui, r, Some((dl, spans(idx), diff.intraline.line(hunk, idx))), side, st, v.wrap, selected);
                    paint_mark(ui, r, selected, hovered);
                    let key = if side == Side::Old { LineSide::Old } else { LineSide::New };
                    if let Some(mark) = v.marks.and_then(|m| thread_marks::find(m, dl, Some(key))) {
                        thread_marks::paint(ui, r, ui.clip_rect().right().min(r.right()), st.row_h, mark, msgs);
                    }
                }
                ui.painter().vline(left.right(), rect.y_range(), Stroke::new(1.0_f32, color::border()));
                // An opened run folds back from its first line, at the fold row's mark position.
                if let (Some(i), Some(j)) = (old, new)
                    && i == j
                    && let Some(f) = fold::opened_at(&v.opened, i)
                {
                    fold::fold_back(ui, left, v, f, st.split_cols().sign, msgs);
                }
            }
        }
    })
}

/// The line a drag over a pair row counts: the side under the pointer, or the other side's
/// line over an empty cell.
fn dragged_side(ui: &Ui, left: Rect, right: Rect, old: Option<usize>, new: Option<usize>) -> Option<usize> {
    match (old, new) {
        (Some(o), _) if ui.rect_contains_pointer(left) => Some(o),
        (_, Some(n)) if ui.rect_contains_pointer(right) => Some(n),
        _ => old.or(new),
    }
}

/// One side of a row: background, line number, sign and text, clipped to the column so long
/// lines never draw into the other side. `line` comes with its syntax spans and changed-word
/// ranges; `None` is an empty cell. `selected` cells get the stronger background. `st` gives the
/// font, the gutter and whether numbers and changed words are drawn; `wrap` how long lines
/// continue below the first visual line, which holds the number and sign.
fn cell(ui: &Ui, rect: Rect, line: Option<CellLine>, side: Side, st: &DiffStyle, wrap: Option<Wrap>, selected: bool) {
    let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let Some((dl, spans, words)) = line else {
        p.rect_filled(rect, 0.0, color::panel());
        return;
    };
    let (bg, fg, sign) = match (dl.origin, side) {
        ('-', Side::Old) if selected => (color::diff_del_sel(), color::text(), '-'),
        ('+', Side::New) if selected => (color::diff_add_sel(), color::text(), '+'),
        ('-', Side::Old) => (color::diff_del(), color::text(), '-'),
        ('+', Side::New) => (color::diff_add(), color::text(), '+'),
        ('<' | '>' | '=', _) => (Color32::TRANSPARENT, color::weak(), ' '),
        _ => (Color32::TRANSPARENT, color::text(), ' '),
    };
    p.rect_filled(rect, 0.0, bg);
    let number = match side {
        Side::Old => dl.old,
        Side::New => dl.new,
    };
    let y = rect.top() + st.row_h / 2.0;
    let (cols, mono) = (st.split_cols(), &st.font);
    if let Some(x) = cols.num {
        let number = number.map(|n| n.to_string()).unwrap_or_default();
        p.text(pos2(rect.left() + x, y), Align2::RIGHT_CENTER, number, mono.clone(), color::faint());
    }
    p.text(pos2(rect.left() + cols.sign, y), Align2::LEFT_CENTER, sign, mono.clone(), fg);
    let marks = match (dl.origin, side) {
        _ if !st.words => None,
        ('-', Side::Old) => Some((words, color::diff_del_word())),
        ('+', Side::New) => Some((words, color::diff_add_word())),
        _ => None,
    };
    paint_line(&p, pos2(rect.left() + cols.text, y), &dl.text, spans, marks, mono, fg, st.tab, wrap);
}
