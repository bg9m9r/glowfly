//! The split layout: the old side on the left, the new side on the right, with removed and
//! added runs paired row by row and a divider between the columns.

use super::syntax::paint_code;
use super::{DiffView, ROW_H, hunk_header, line_click, mono, paint_mark};
use crate::msg::Msg;
use crate::ui::theme::color;
use eframe::egui::{Align2, Color32, FontId, Rect, Sense, Stroke, Ui, pos2, vec2};
use trellis_core::diff_rows::{SplitRow, split_rows};
use trellis_core::highlight::Span;
use trellis_core::repo::DiffLine;

#[derive(Clone, Copy, PartialEq, Hash)]
enum Side {
    Old,
    New,
}

/// Draws the rows and returns the first visible one.
pub(super) fn show(ui: &mut Ui, v: &DiffView, scroll_to: Option<usize>, msgs: &mut Vec<Msg>) -> usize {
    let diff = v.diff;
    let rows = split_rows(diff);
    let widest = diff.hunks.iter().flat_map(|h| &h.lines).map(|l| l.text.chars().count()).max().unwrap_or(0);
    let mono = mono();
    let char_w = ui.fonts(|f| f.glyph_width(&mono, 'M'));
    // Each column is wide enough for the longest line on either side.
    let width = ui.available_width().max(2.0 * (62.0 + widest as f32 * char_w + 20.0));
    let half = (width / 2.0).round();

    super::scroll(ui, rows.len(), scroll_to, |ui, i| {
        let (rect, _) = ui.allocate_exact_size(vec2(width, ROW_H), Sense::hover());
        match rows[i] {
            SplitRow::Header(h) => hunk_header(ui, rect, v, h, msgs),
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
                        cell(ui, r, None, side, &mono, false);
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
                    cell(ui, r, Some((dl, spans(idx))), side, &mono, selected);
                    paint_mark(ui, r, selected, hovered);
                }
                ui.painter().vline(left.right(), rect.y_range(), Stroke::new(1.0_f32, color::border()));
            }
        }
    })
}

/// One side of a row: background, line number, sign and text, clipped to the column so long
/// lines never draw into the other side. `line` comes with its syntax spans; `None` is an
/// empty cell. `selected` cells get the stronger background.
fn cell(ui: &Ui, rect: Rect, line: Option<(&DiffLine, &[Span])>, side: Side, mono: &FontId, selected: bool) {
    let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let Some((dl, spans)) = line else {
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
    let number = number.map(|n| n.to_string()).unwrap_or_default();
    p.text(pos2(rect.left() + 40.0, y), Align2::RIGHT_CENTER, number, mono.clone(), color::faint());
    p.text(pos2(rect.left() + 48.0, y), Align2::LEFT_CENTER, sign, mono.clone(), fg);
    paint_code(&p, pos2(rect.left() + 62.0, y), &dl.text, spans, mono, fg);
}
