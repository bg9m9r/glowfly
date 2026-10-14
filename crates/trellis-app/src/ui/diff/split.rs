//! The split layout: the old side on the left, the new side on the right, with removed and
//! added runs paired row by row and a divider between the columns.

use super::style::DiffStyle;
use super::syntax::paint_line;
use super::{DiffView, fold, hunk_header, line_click, paint_mark};
use crate::msg::Msg;
use crate::ui::theme::color;
use eframe::egui::{Align2, Color32, Rect, Sense, Stroke, Ui, pos2, vec2};
use std::ops::Range;
use trellis_core::diff_rows::{SplitRow, split_rows};
use trellis_core::highlight::Span;
use trellis_core::repo::DiffLine;
use trellis_core::{tabs, whole_file};

/// A line with its syntax spans and changed-word ranges.
type CellLine<'a> = (&'a DiffLine, &'a [Span], &'a [Range<u32>]);

#[derive(Clone, Copy, PartialEq, Hash)]
enum Side {
    Old,
    New,
}

/// Draws the rows and returns the first visible one and how many rows fit.
pub(super) fn show(ui: &mut Ui, v: &DiffView, scroll_to: Option<usize>, msgs: &mut Vec<Msg>) -> (usize, usize) {
    let diff = v.lines();
    // In the whole-file view `hunk` is always 0.
    let rows = v.whole.map_or_else(|| split_rows(v.diff), |w| whole_file::split_rows(w, v.expanded));
    let st = &v.style;
    let widest = diff.hunks.iter().flat_map(|h| &h.lines).map(|l| tabs::columns(&l.text, st.tab)).max().unwrap_or(0);
    let char_w = ui.fonts(|f| f.glyph_width(&st.font, 'M'));
    // Each column is wide enough for the longest line on either side.
    let width = ui.available_width().max(2.0 * (st.split_cols().text + widest as f32 * char_w + 20.0));
    let half = (width / 2.0).round();

    super::scroll(ui, rows.len(), st.row_h, scroll_to, |ui, i| {
        let (rect, _) = ui.allocate_exact_size(vec2(width, st.row_h), Sense::hover());
        match rows[i] {
            // A change block's action row acts on its normal hunk, like that hunk's header.
            SplitRow::Header(h) | SplitRow::Block(h) => hunk_header(ui, rect, v, h, msgs),
            // Spans both columns, with no divider, like an action row.
            SplitRow::Fold(f) => {
                let cols = st.split_cols();
                fold::fold_row(ui, rect, v, f, cols.sign, cols.text, msgs)
            }
            SplitRow::Pair { hunk, old, new } => {
                let lines = &diff.hunks[hunk].lines;
                let left = Rect::from_min_max(rect.min, pos2(rect.left() + half, rect.bottom()));
                let right = Rect::from_min_max(pos2(left.right(), rect.top()), rect.max);
                let spans = |i: usize| -> &[Span] {
                    match (lines[i].origin, v.hl) {
                        ('<' | '>' | '=', _) | (_, None) => &[],
                        (_, Some(h)) => h.line(hunk, i),
                    }
                };
                for (side, r, idx) in [(Side::Old, left, old), (Side::New, right, new)] {
                    let Some(idx) = idx else {
                        cell(ui, r, None, side, st, false);
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
                        let resp = ui.interact(r, ui.id().with(("diff-cell", hunk, idx, side)), Sense::click());
                        line_click(ui, &resp, hunk, idx, origin, msgs)
                    };
                    cell(ui, r, Some((dl, spans(idx), diff.intraline.line(hunk, idx))), side, st, selected);
                    paint_mark(ui, r, selected, hovered);
                }
                ui.painter().vline(left.right(), rect.y_range(), Stroke::new(1.0_f32, color::border()));
            }
        }
    })
}

/// One side of a row: background, line number, sign and text, clipped to the column so long
/// lines never draw into the other side. `line` comes with its syntax spans and changed-word
/// ranges; `None` is an empty cell. `selected` cells get the stronger background. `st` gives the
/// font, the gutter and whether numbers and changed words are drawn.
fn cell(ui: &Ui, rect: Rect, line: Option<CellLine>, side: Side, st: &DiffStyle, selected: bool) {
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
    let y = rect.center().y;
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
    paint_line(&p, pos2(rect.left() + cols.text, y), &dl.text, spans, marks, mono, fg, st.tab);
}
