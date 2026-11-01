//! The inline layout: one column of hunks (or of the whole file), each line with its old and
//! new number (unless line numbers are off), and the unchanged lines between hunks once filled
//! in. Text selected in the code paints under its glyphs (`text_sel`).

use super::syntax::paint_line_sel;
use super::text_sel::{self, Hit, TextMut};
use super::wrap::{Pass, RowMetrics};
use super::{
    DiffView, LineHover, comment_add, drag_over, fold, gutter_rect, gutter_sign, hunk_header, line_click, line_sense, paint_mark,
    thread_marks,
};
use crate::msg::Msg;
use crate::state::diff_text::{LineRef, TextSide};
use crate::ui::theme::color;
use eframe::egui::{Align2, Color32, Sense, Ui, pos2, vec2};
use glowfly_core::diff_rows::DiffRow;
use glowfly_core::highlight::Span;

/// Draws the rows and returns the first visible one and how many rows fit.
/// `rows`, `widest` (in columns) and the rows' heights `m` come from the rows cache; `t` is the
/// text selection, which a press or drag on the code changes.
#[allow(clippy::too_many_arguments)]
pub(super) fn show(
    ui: &mut Ui,
    v: &DiffView,
    rows: &[DiffRow],
    widest: usize,
    m: RowMetrics,
    scroll_to: Option<usize>,
    t: &mut TextMut,
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
    let (mut code, mut hits) = (None, Vec::new());

    super::wrap::scroll_framed(ui, super::wrap::DIFF, rows.len(), m, scroll_to, |ui, pass| {
        let (i, h) = match pass {
            Pass::Row(i, h) => (i, h),
            Pass::Begin => {
                code = Some(text_sel::sense(ui));
                return;
            }
            Pass::End => {
                if let Some(code) = &code {
                    text_sel::handle(ui, code, &hits, v, t, msgs);
                }
                return;
            }
        };
        // Line gutters sense clicks and drags by their own stable ids (auto ids shift as rows scroll).
        let (rect, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
        match rows[i] {
            // During a line drag the header counts as the line before the hunk's first.
            DiffRow::Header(h) => {
                drag_over(ui, v, rect, h, 0, msgs);
                hunk_header(ui, rect, v, h, msgs)
            }
            // A change block's action row acts on its normal hunk, like that hunk's header.
            DiffRow::Block(h) => hunk_header(ui, rect, v, h, msgs),
            DiffRow::Fold(f) => fold::fold_row(ui, rect, v, f, cols.sign, cols.text, msgs),
            // An opened gap's line: drawn like a context line, never selectable or marked.
            DiffRow::GapLine { hunk, line } => {
                drag_over(ui, v, rect, hunk, 0, msgs);
                let Some((g, text)) = v.gap_lines.iter().find(|(g, _)| g.hunk == hunk) else { return };
                let Some(raw) = text.lines.get(line) else { return };
                let p = ui.painter();
                let y = rect.top() + st.row_h / 2.0;
                let numbers = [(cols.old, g.old_start), (cols.new, g.new_start)];
                for (x, n) in numbers.into_iter().filter_map(|(x, n)| Some((x?, n + line as u32))) {
                    p.text(pos2(rect.left() + x, y), Align2::RIGHT_CENTER, n.to_string(), mono.clone(), color::faint());
                }
                let spans: &[Span] = if v.hl.is_some() { text.spans.get(line).map_or(&[], Vec::as_slice) } else { &[] };
                let r = LineRef::Gap { hunk, line };
                let sel = v.text.highlight(TextSide::Inline, r, raw);
                let at = pos2(rect.left() + cols.text, y);
                let laid = paint_line_sel(p, at, raw, spans, None, mono, color::text(), st.tab, v.wrap, sel.as_ref());
                hits.push(Hit { side: TextSide::Inline, line: r, raw, cell: rect, text_x: at.x, laid });
                if line == 0 {
                    fold::fold_back_key(ui, rect, v, g.key, g.len, cols.sign, msgs);
                }
                let n = line as u32;
                let dl = glowfly_core::repo::DiffLine {
                    origin: ' ',
                    old: Some(g.old_start + n),
                    new: Some(g.new_start + n),
                    text: String::new(),
                };
                let right = ui.clip_rect().right().min(rect.right());
                comment_add::button(ui, v, rect, right, st.row_h, &dl, None, false, 0.0, msgs);
            }
            DiffRow::Line { hunk, line } => {
                let dl = &diff.hunks[hunk].lines[line];
                let selected = v.is_selected(hunk, line);
                let hover = match v.sel {
                    Some(_) => {
                        let resp = line_sense(ui, gutter_rect(rect, cols.text), ui.id().with(("diff-line", hunk, line)), dl.origin);
                        drag_over(ui, v, rect, hunk, line, msgs);
                        line_click(ui, &resp, v, hunk, line, dl.origin, msgs)
                    }
                    None => LineHover::default(),
                };
                let (bg, fg, sign) = match dl.origin {
                    '+' if selected => (st.sel_add, color::text(), '+'),
                    '-' if selected => (st.sel_del, color::text(), '-'),
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
                gutter_sign(p, pos2(rect.left() + cols.sign, y), sign, fg, mono, selected, hover);
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
                // "No newline at end of file" notes have no text to select.
                let r = (!matches!(dl.origin, '<' | '>' | '=')).then(|| v.line_ref(hunk, line));
                let sel = r.and_then(|r| v.text.highlight(TextSide::Inline, r, &dl.text));
                let at = pos2(rect.left() + cols.text, y);
                let laid = paint_line_sel(p, at, &dl.text, spans, marks, mono, fg, st.tab, v.wrap, sel.as_ref());
                if let Some(r) = r {
                    hits.push(Hit { side: TextSide::Inline, line: r, raw: &dl.text, cell: rect, text_x: at.x, laid });
                }
                paint_mark(ui, rect, selected, hover.gutter);
                let right = ui.clip_rect().right().min(rect.right());
                let mut taken = match v.marks.and_then(|m| thread_marks::find(m, dl, None)) {
                    Some(mark) => thread_marks::paint(ui, rect, right, st.row_h, mark, msgs),
                    None => 0.0,
                };
                taken += comment_add::draft_marks(ui, v, rect, right, st.row_h, dl, None, taken, msgs);
                comment_add::button(ui, v, rect, right, st.row_h, dl, None, true, taken, msgs);
                if let Some(f) = fold::opened_at(&v.opened, line) {
                    fold::fold_back(ui, rect, v, f, cols.sign, msgs);
                }
            }
        }
    })
}
