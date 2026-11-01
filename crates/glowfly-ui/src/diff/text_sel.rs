//! Selecting text in the diff's code as in an editor, in both layouts and every diff: a drag
//! selects characters, a double-click a word and a triple-click the whole line (a drag after
//! them grows by words or lines), and Shift-click extends from the anchor; a plain click drops
//! the selection and never touches the line selection. The code senses the pointer through one
//! widget under the rows (`sense`), so the gutters, hunk buttons, folds and comment buttons
//! drawn over it keep their clicks, and hunk headers and folds add nothing as a drag passes
//! over them. The selection paints behind the selected glyphs only, with a nub one space wide
//! where it goes on past a line's end (`paint_under`); a drag past the view's edge scrolls it
//! (`edge_step`), and Ctrl+C (Cmd+C) copies the selected code (`state::diff_text::copy`) while a
//! press in the diff last gave it the keyboard.

use super::DiffView;
use super::rows_cache::LineIndex;
use super::syntax::{Laid, SegSel};
use crate::msg::Msg;
use crate::state::diff_text::{self, LineRef, ShownLine, Span, TextPos, TextSel, TextSide, Unit};
use crate::ui::theme::color;
use eframe::egui::text::CCursor;
use eframe::egui::{Color32, Context, CursorIcon, Event, FontId, Galley, Id, Painter, Pos2, Rangef, Rect, Response, Sense, Ui, pos2, vec2};
use std::ops::Range;

/// How far past the view's edge a drag scrolls fastest, in points.
const EDGE_FULL: f32 = 96.0;
/// Rows (or columns) a frame the fastest edge scroll moves.
const EDGE_ROWS: f32 = 3.0;

/// The selection as the view keeps it through a frame, taken out of `ViewState`: what is
/// selected, and the rows and columns short of a whole one the edge scroll has moved.
pub(super) struct TextMut {
    pub sel: Option<TextSel>,
    pub edge: [f32; 2],
}

/// What the rows paint the selection from this frame.
pub(crate) struct TextView<'a> {
    /// The lines the rows show.
    pub index: &'a LineIndex,
    /// The load of the diff on screen (`DiffSlot::shown_gen`), which keys a new selection.
    pub shown_gen: u64,
    /// The selected stretch, its ends ordered by row, and the side it is on.
    pub span: Option<(TextSide, Span)>,
    /// Whether the diff has the keyboard (a press in it last, no text field focused).
    pub focused: bool,
}

/// The selected characters of a line, whether the nub follows them, and their fill.
pub(crate) struct Highlight {
    pub chars: Range<usize>,
    pub nub: bool,
    pub color: Color32,
}

impl TextView<'_> {
    /// The highlight of `line` on `side`, whose text is `raw`: `code_sel`, or `code_sel_dim`
    /// while the diff doesn't have the keyboard; `None` where nothing of it is selected.
    pub fn highlight(&self, side: TextSide, line: LineRef, raw: &str) -> Option<Highlight> {
        let (s, span) = self.span.as_ref()?;
        if *s != side || span.is_empty() {
            return None;
        }
        let row = self.index.row(line)?;
        if row < span.start_at || row > span.end_at {
            return None;
        }
        let len = raw.chars().count();
        let from = if row == span.start_at { span.start.col.min(len) } else { 0 };
        let to = if row == span.end_at { span.end.col.min(len) } else { len };
        let nub = row < span.end_at;
        let color = if self.focused { color::code_sel() } else { color::code_sel_dim() };
        (from < to || nub).then(|| Highlight { chars: from..to.max(from), nub, color })
    }
}

/// A line drawn this frame, where the pointer can land on it: its side and place, its text, the
/// row (inline) or cell (split) it is in, where its text starts and how it was laid out.
pub(super) struct Hit<'a> {
    pub side: TextSide,
    pub line: LineRef,
    pub raw: &'a str,
    pub cell: Rect,
    pub text_x: f32,
    pub laid: Laid,
}

/// The id the diff's code senses the pointer by, and keeps its keyboard and click count under.
fn body_id() -> Id {
    Id::new("diff-text")
}

/// Whether the diff has the keyboard: a press in it came last, no text field or list has taken
/// the focus since, and the diff was drawn in the last frame (one gone from the screen let go).
pub(super) fn focused(ctx: &Context) -> bool {
    let (on, pass) = ctx.data(|d| d.get_temp::<(bool, u64)>(body_id().with("focus"))).unwrap_or_default();
    on && pass + 1 >= ctx.cumulative_pass_nr() && ctx.memory(|m| m.focused().is_none())
}

/// Keeps the diff's focus flag fresh in a frame it is drawn: what `focused` says, or what a
/// press now says (`Some(inside)`), stamped with this frame.
fn stamp_focus(ctx: &Context, press: Option<bool>) {
    // Read outside the data lock: the context can't be asked from inside it.
    let now = ctx.cumulative_pass_nr();
    let kept = ctx.data(|d| d.get_temp::<(bool, u64)>(body_id().with("focus")));
    let on = press.unwrap_or(kept.is_some_and(|(on, pass)| on && pass + 1 >= now));
    ctx.data_mut(|d| d.insert_temp(body_id().with("focus"), (on, now)));
}

/// The code's pointer sense, under the rows: added before them, so whatever a row draws on top
/// takes its own clicks. In the content `Ui`, whose clip rect is the view.
pub(super) fn sense(ui: &Ui) -> Response {
    ui.interact(ui.clip_rect(), body_id(), Sense::CLICK | Sense::DRAG)
}

/// After the rows: the press, drag, release and copy key on the code (`bg`, from `sense`),
/// against the lines drawn (`hits`). Writes the selection in `t` directly, as the terminal's is.
pub(super) fn handle(ui: &Ui, bg: &Response, hits: &[Hit], v: &DiffView, t: &mut TextMut, msgs: &mut Vec<Msg>) {
    let ctx = ui.ctx();
    let view = ui.clip_rect();
    let (pressed, down, shift, latest) =
        ui.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.modifiers.shift, i.pointer.latest_pos()));
    // Pressing anywhere in the diff gives it the keyboard; pressing anywhere else takes it away.
    if ui.input(|i| i.pointer.any_pressed()) {
        let inside = ui.rect_contains_pointer(view);
        stamp_focus(ctx, Some(inside));
    } else {
        stamp_focus(ctx, None);
    }
    // An I-beam over code; the gutters show their own pointer.
    if bg.hover_pos().is_some_and(|p| hits.iter().any(|h| h.cell.contains(p) && p.x >= h.text_x)) {
        ctx.set_cursor_icon(CursorIcon::Text);
    }
    // Whether the button went down on code and is still held: the selection follows it.
    let selecting = body_id().with("selecting");
    let mut held = ctx.data(|d| d.get_temp::<bool>(selecting)).unwrap_or(false) && down;
    if pressed && bg.is_pointer_button_down_on() {
        let at = ui.input(|i| i.pointer.press_origin()).or(latest);
        held = at.is_some_and(|at| press(ui, at, shift, hits, v, t));
        t.edge = [0.0; 2];
    } else if held && let Some(p) = latest {
        let before = t.sel.as_ref().map(|s| s.head);
        // Keep scrolling and following while the pointer is held still past the edge.
        if drag(ui, p, view, hits, v, t) || t.sel.as_ref().map(|s| s.head) != before {
            ctx.request_repaint();
        }
        ctx.set_cursor_icon(CursorIcon::Text);
    }
    ctx.data_mut(|d| d.insert_temp(selecting, held));
    if bg.clicked() {
        // A plain click selects nothing: on code it leaves an empty selection, dropped here;
        // anywhere else in the diff (a header's empty part, below the rows) it drops the text.
        let empty = t.sel.as_ref().is_some_and(|s| s.unit == Unit::Char && s.anchor == s.head);
        let on_code = bg.interact_pointer_pos().is_some_and(|p| hits.iter().any(|h| h.cell.contains(p) && p.x >= h.text_x));
        if empty || (!on_code && !shift) {
            t.sel = None;
        }
    }
    if bg.drag_stopped() || !down {
        t.edge = [0.0; 2];
    }
    let copy_key = ui.input(|i| i.events.iter().any(|e| matches!(e, Event::Copy)));
    if copy_key && focused(ctx) {
        let text = t.sel.as_ref().map(|s| copy(v, s)).unwrap_or_default();
        if !text.is_empty() {
            msgs.push(Msg::Copy(text));
        }
    }
}

/// A press at `at`: on code, a selection of nothing there growing by characters, words or
/// lines (one, two or three presses), or with Shift the head moved there. Returns whether it
/// was on code (a header or a gutter is not).
fn press(ui: &Ui, at: Pos2, shift: bool, hits: &[Hit], v: &DiffView, t: &mut TextMut) -> bool {
    let hit = hits.iter().find(|h| h.cell.contains(at) && at.x >= h.text_x);
    // Shift-click always extends: it never counts toward a double-click, nor starts one.
    if shift
        && let (Some(h), Some(s)) = (hit, t.sel.as_mut())
        && s.side == h.side
    {
        s.head = TextPos { line: h.line, col: col_at(h, at, v.style.tab) };
        ui.ctx().data_mut(|d| d.remove_temp::<(f64, Pos2, usize)>(body_id().with("clicks")));
        return true;
    }
    let count = clicks(ui, at);
    let Some(h) = hit else { return false };
    let pos = TextPos { line: h.line, col: col_at(h, at, v.style.tab) };
    let unit = [Unit::Char, Unit::Word, Unit::Line][count - 1];
    t.sel = Some(TextSel::at(pos, unit, h.side, (v.target.clone(), v.text.shown_gen)));
    true
}

/// How many presses in a row this one at `at` makes (1 to 3): one within the double-click
/// time and distance of the last adds one.
fn clicks(ui: &Ui, at: Pos2) -> usize {
    let ctx = ui.ctx();
    let (delay, dist) = ctx.options(|o| (o.input_options.max_double_click_delay, o.input_options.max_click_dist));
    let now = ui.input(|i| i.time);
    let id = body_id().with("clicks");
    let last = ctx.data(|d| d.get_temp::<(f64, Pos2, usize)>(id));
    let n = match last {
        Some((then, p, n)) if now - then <= delay && p.distance(at) <= dist => (n + 1).min(3),
        _ => 1,
    };
    ctx.data_mut(|d| d.insert_temp(id, (now, at, n)));
    n
}

/// The pointer at `p` during a drag: past the view's edge it scrolls (sideways too when lines
/// don't wrap), and the head follows to the place under it on the selection's side, kept
/// inside the view. Over a hunk header or fold the head stays. Returns whether it is past the
/// edge, where it keeps scrolling.
fn drag(ui: &Ui, p: Pos2, view: Rect, hits: &[Hit], v: &DiffView, t: &mut TextMut) -> bool {
    let (dy, dx) = (past(p.y, view.y_range()), if v.wrap.is_some() { 0.0 } else { past(p.x, view.x_range()) });
    let (rows, cols) = (edge_step(dy, &mut t.edge[0]), edge_step(dx, &mut t.edge[1]));
    if rows != 0 || cols != 0 {
        let char_w = ui.fonts(|f| f.glyph_width(&v.style.font, 'M'));
        ui.scroll_with_delta(vec2(-(cols as f32) * char_w, -(rows as f32) * v.style.row_h));
    }
    let inside = pos2(p.x.clamp(view.left(), view.right() - 1.0), p.y.clamp(view.top(), view.bottom() - 1.0));
    if let Some(sel) = t.sel.as_mut()
        && let Some(h) = hits.iter().find(|h| h.side == sel.side && h.cell.y_range().contains(inside.y))
    {
        sel.head = TextPos { line: h.line, col: col_at(h, inside, v.style.tab) };
    }
    dy != 0.0 || dx != 0.0
}

/// How far `v` is past `range`: negative before it, positive after, 0 inside.
fn past(v: f32, range: Rangef) -> f32 {
    if v < range.min {
        v - range.min
    } else if v > range.max {
        v - range.max
    } else {
        0.0
    }
}

/// The whole rows (or columns) a drag `d` points past the view's edge scrolls this frame:
/// `min(|d|, 96) / 96 × 3`, forward past the end and back before the start, the part of one
/// carried in `rem`. Inside the view nothing scrolls and `rem` is dropped.
pub(super) fn edge_step(d: f32, rem: &mut f32) -> i32 {
    if d == 0.0 {
        *rem = 0.0;
        return 0;
    }
    let total = *rem + d.signum() * d.abs().min(EDGE_FULL) / EDGE_FULL * EDGE_ROWS;
    let whole = total.trunc();
    *rem = total - whole;
    whole as i32
}

/// The character of `h`'s line nearest `at`, hit-tested on the visual line under it.
fn col_at(h: &Hit, at: Pos2, tab: usize) -> usize {
    let parts = &h.laid.parts;
    let Some((origin, galley, start)) = parts.iter().rev().find(|(o, _, _)| o.y <= at.y).or(parts.first()) else { return 0 };
    raw_col(h.raw, tab, start + galley.cursor_from_pos(at - *origin).ccursor.index)
}

/// The text `sel` selects, as Ctrl+C copies it: the lines of its side as shown.
pub(super) fn copy(v: &DiffView, sel: &TextSel) -> String {
    let shown: Vec<ShownLine> =
        v.text.index.lines(sel.side).filter_map(|line| line_text(v, line).map(|(origin, text)| ShownLine { line, origin, text })).collect();
    diff_text::copy(&shown, sel, false)
}

/// The selected stretch of `sel` in `v`'s rows, with its side; `None` when an end isn't shown.
pub(super) fn span(v: &DiffView, sel: &TextSel) -> Option<(TextSide, Span)> {
    let s = sel.span(|r| v.text.index.row(r), |r| line_text(v, r).map(|(_, t)| t))?;
    Some((sel.side, s))
}

/// A shown line's origin and text.
fn line_text<'a>(v: &'a DiffView, r: LineRef) -> Option<(char, &'a str)> {
    let line = |hunk: usize, line: usize| v.lines().hunks.get(hunk)?.lines.get(line).map(|l| (l.origin, l.text.as_str()));
    match r {
        LineRef::Hunk { hunk, line: l } => line(hunk, l),
        LineRef::Whole { line: l } => line(0, l),
        LineRef::Gap { hunk, line } => v.gap_lines.iter().find(|(g, _)| g.hunk == hunk)?.1.lines.get(line).map(|t| (' ', t.as_str())),
    }
}

/// Fills the selected characters `s` of a visual line laid out as `galley` at `origin`, and the
/// nub one space of `font` wide after them, square, behind the glyphs only.
pub(super) fn paint_under(p: &Painter, origin: Pos2, galley: &Galley, s: &SegSel, font: &FontId) {
    let x = |c: usize| origin.x + galley.pos_from_ccursor(CCursor::new(c)).min.x;
    let (x0, x1) = (x(s.chars.start), x(s.chars.end));
    let right = if s.nub { x1 + p.ctx().fonts(|f| f.glyph_width(font, ' ')) } else { x1 };
    if right > x0 {
        p.rect_filled(Rect::from_min_max(pos2(x0, origin.y), pos2(right, origin.y + galley.size().y)), 0.0, s.color);
    }
}

/// The column of the tab-expanded line where character `col` of `raw` starts (tab stops every
/// `tab` columns; each other character is one column).
pub(super) fn expanded_col(raw: &str, tab: usize, col: usize) -> usize {
    let tab = tab.max(1);
    raw.chars().take(col).fold(0, |c, ch| if ch == '\t' { (c / tab + 1) * tab } else { c + 1 })
}

/// The character of `raw` nearest column `exp` of its tab-expanded form: inside a tab's spaces,
/// the nearer of its two sides.
pub(super) fn raw_col(raw: &str, tab: usize, exp: usize) -> usize {
    let tab = tab.max(1);
    let mut c = 0;
    for (i, ch) in raw.chars().enumerate() {
        let next = if ch == '\t' { (c / tab + 1) * tab } else { c + 1 };
        if exp < next {
            return if exp - c <= next - exp { i } else { i + 1 };
        }
        c = next;
    }
    raw.chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_map_both_ways() {
        let raw = "a\tb";
        assert_eq!([0, 1, 2, 3].map(|c| expanded_col(raw, 4, c)), [0, 1, 4, 5]);
        // Inside the tab's spaces (columns 1 to 3), the nearer side of the tab.
        assert_eq!([0, 1, 2, 3, 4, 5, 9].map(|e| raw_col(raw, 4, e)), [0, 1, 1, 2, 2, 3, 3]);
        assert_eq!(raw_col("abc", 4, 2), 2, "no tabs: the same");
    }

    #[test]
    fn the_edge_scroll_speeds_up_to_three_rows_a_frame() {
        let mut rem = 0.0;
        assert_eq!(edge_step(96.0, &mut rem), 3);
        assert_eq!(edge_step(500.0, &mut rem), 3, "no faster past 96 points");
        assert_eq!(edge_step(-200.0, &mut rem), -3, "back above the top");
        // 16 points out is half a row a frame: one row every other frame.
        let steps: Vec<i32> = (0..4).map(|_| edge_step(16.0, &mut rem)).collect();
        assert_eq!(steps, [0, 1, 0, 1]);
        assert_eq!(edge_step(0.0, &mut rem), 0);
        assert_eq!(rem, 0.0, "inside the view the part is dropped");
    }
}
