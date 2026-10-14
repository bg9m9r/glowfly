//! The inline layout: one column of hunks, each line with its old and new number.

use super::syntax::paint_code;
use super::{DiffView, ROW_H, hunk_header, line_click, mono, paint_mark};
use crate::msg::Msg;
use crate::ui::theme::color;
use eframe::egui::{Align2, Color32, Sense, Ui, pos2, vec2};
use trellis_core::diff_rows::{DiffRow, inline_rows};
use trellis_core::highlight::Span;

/// Draws the rows and returns the first visible one.
pub(super) fn show(ui: &mut Ui, v: &DiffView, scroll_to: Option<usize>, msgs: &mut Vec<Msg>) -> usize {
    let diff = v.diff;
    // Uniform rows so huge diffs stay fast.
    let rows = inline_rows(diff);
    let widest = diff.hunks.iter().flat_map(|h| &h.lines).map(|l| l.text.chars().count()).max().unwrap_or(0);
    let mono = mono();
    let char_w = ui.fonts(|f| f.glyph_width(&mono, 'M'));
    let width = ui.available_width().max(110.0 + widest as f32 * char_w + 40.0);

    super::scroll(ui, rows.len(), scroll_to, |ui, i| {
        // Lines can be clicked (to select, or to clear the selection) unless it's a commit diff.
        let sense = if v.sel.is_some() && matches!(rows[i], DiffRow::Line { .. }) { Sense::click() } else { Sense::hover() };
        let (rect, resp) = ui.allocate_exact_size(vec2(width, ROW_H), sense);
        match rows[i] {
            DiffRow::Header(h) => hunk_header(ui, rect, v, h, msgs),
            DiffRow::Line { hunk, line } => {
                let dl = &diff.hunks[hunk].lines[line];
                let selected = v.is_selected(hunk, line);
                let hovered = v.sel.is_some() && line_click(ui, &resp, hunk, line, dl.origin, msgs);
                let (bg, fg, sign) = match dl.origin {
                    '+' if selected => (color::diff_add_sel(), color::text(), '+'),
                    '-' if selected => (color::diff_del_sel(), color::text(), '-'),
                    '+' => (color::diff_add(), color::text(), '+'),
                    '-' => (color::diff_del(), color::text(), '-'),
                    '>' | '<' | '=' => (Color32::TRANSPARENT, color::weak(), ' '),
                    _ => (Color32::TRANSPARENT, color::text(), ' '),
                };
                let p = ui.painter();
                p.rect_filled(rect, 0.0, bg);
                let num = |n: Option<u32>| n.map(|n| n.to_string()).unwrap_or_default();
                let y = rect.center().y;
                p.text(pos2(rect.left() + 40.0, y), Align2::RIGHT_CENTER, num(dl.old), mono.clone(), color::faint());
                p.text(pos2(rect.left() + 84.0, y), Align2::RIGHT_CENTER, num(dl.new), mono.clone(), color::faint());
                p.text(pos2(rect.left() + 94.0, y), Align2::LEFT_CENTER, sign, mono.clone(), fg);
                let spans: &[Span] = match (dl.origin, v.hl) {
                    ('<' | '>' | '=', _) | (_, None) => &[],
                    (_, Some(h)) => h.line(hunk, line),
                };
                let marks = match dl.origin {
                    '-' => Some((diff.intraline.line(hunk, line), color::diff_del_word())),
                    '+' => Some((diff.intraline.line(hunk, line), color::diff_add_word())),
                    _ => None,
                };
                paint_code(p, pos2(rect.left() + 110.0, y), &dl.text, spans, marks, &mono, fg);
                paint_mark(ui, rect, selected, hovered);
            }
        }
    })
}
