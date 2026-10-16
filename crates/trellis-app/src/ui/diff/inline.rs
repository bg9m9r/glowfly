//! The inline layout: one column of hunks (or of the whole file), each line with its old and
//! new number (unless line numbers are off).

use super::syntax::paint_line;
use super::wrap::RowMetrics;
use super::{DiffView, fold, hunk_header, line_click, paint_mark};
use crate::msg::Msg;
use crate::ui::theme::color;
use eframe::egui::{Align2, Color32, Sense, Ui, pos2, vec2};
use trellis_core::diff_rows::DiffRow;
use trellis_core::highlight::Span;

/// Draws the rows and returns the first visible one and how many rows fit.
/// `rows`, `widest` (in columns) and the rows' heights `m` come from the rows cache.
pub(super) fn show(
    ui: &mut Ui,
    v: &DiffView,
    rows: &[DiffRow],
    widest: usize,
    m: RowMetrics,
    scroll_to: Option<usize>,
    msgs: &mut Vec<Msg>,
) -> (usize, usize) {
    let diff = v.lines();
    // Only the visible rows are drawn, so huge diffs stay fast. In the whole-file view `hunk`
    // is always 0.
    let st = &v.style;
    let cols = st.inline_cols();
    let mono = &st.font;
    let char_w = ui.fonts(|f| f.glyph_width(mono, 'M'));
    // Wrapped lines fit the viewer, so nothing scrolls sideways.
    let width = if v.wrap.is_some() { ui.available_width() } else { ui.available_width().max(cols.text + widest as f32 * char_w + 40.0) };

    super::wrap::scroll(ui, rows.len(), m, scroll_to, |ui, i, h| {
        // Lines can be clicked (to select, or to clear the selection) unless it's a commit diff.
        let sense = if v.sel.is_some() && matches!(rows[i], DiffRow::Line { .. }) { Sense::click() } else { Sense::hover() };
        let (rect, resp) = ui.allocate_exact_size(vec2(width, h), sense);
        match rows[i] {
            // A change block's action row acts on its normal hunk, like that hunk's header.
            DiffRow::Header(h) | DiffRow::Block(h) => hunk_header(ui, rect, v, h, msgs),
            DiffRow::Fold(f) => fold::fold_row(ui, rect, v, f, cols.sign, cols.text, msgs),
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
                // Numbers and sign on the first visual line.
                let y = rect.top() + st.row_h / 2.0;
                for (x, n) in [(cols.old, dl.old), (cols.new, dl.new)] {
                    if let Some(x) = x {
                        p.text(pos2(rect.left() + x, y), Align2::RIGHT_CENTER, num(n), mono.clone(), color::faint());
                    }
                }
                p.text(pos2(rect.left() + cols.sign, y), Align2::LEFT_CENTER, sign, mono.clone(), fg);
                let spans: &[Span] = match (dl.origin, v.hl) {
                    ('<' | '>' | '=', _) | (_, None) => &[],
                    (_, Some(h)) => h.line(hunk, line),
                };
                let marks = match dl.origin {
                    _ if !st.words => None,
                    '-' => Some((diff.intraline.line(hunk, line), color::diff_del_word())),
                    '+' => Some((diff.intraline.line(hunk, line), color::diff_add_word())),
                    _ => None,
                };
                paint_line(p, pos2(rect.left() + cols.text, y), &dl.text, spans, marks, mono, fg, st.tab, v.wrap);
                paint_mark(ui, rect, selected, hovered);
                if let Some(f) = fold::opened_at(&v.opened, line) {
                    fold::fold_back(ui, rect, v, f, cols.sign, msgs);
                }
            }
        }
    })
}
