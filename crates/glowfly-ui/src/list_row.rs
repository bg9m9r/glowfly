//! The sidebar's list rows: one line of text (cut with "…" when it doesn't fit) with optional dim
//! text (words, or counts with painted arrows) and a marker slot at the right, plus the
//! skip-if-scrolled-away helper the long sections use; the file lists' fixed-height rows
//! (`file_row`, and `file_row_with_end` with a part at the right edge), each one click target
//! with its selection painted across the row and into the panel padding (`keyed_file_row*` for
//! a list that takes the keyboard focus as a whole, and `file_row_focus` for its ring), and
//! `row_text` for their unnamed path; and the side panels' row heights and insets. Selection and hover look
//! like the graph's (`widgets::row_fill`).

#[path = "file_tree.rs"]
pub(super) mod file_tree;

use super::theme::{color, spacing, tokens};
use super::widgets::{row_fill, truncate};
use crate::ui::theme::type_scale;

use std::sync::Arc;

use eframe::egui::{
    self, Align, Color32, Galley, InnerResponse, Layout, Painter, Pos2, Rect, Response, RichText, Sense, Stroke, TextStyle, TextWrapMode,
    Ui, UiBuilder, WidgetInfo, WidgetText, WidgetType, pos2, vec2,
};

/// One piece of a list row's dim right-hand text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Part<'a> {
    /// Words, drawn as text.
    Text(&'a str),
    /// Commits ahead of the upstream: the count, then a painted up arrow.
    Ahead(usize),
    /// Commits behind the upstream: the count, then a painted down arrow.
    Behind(usize),
}

/// One sidebar list row. Returns the click response; `right` is optional dim text at the right.
/// A row whose text is cut short shows the whole text on hover.
pub fn list_row(ui: &mut Ui, indent: f32, text: &str, right: Option<(&str, Color32)>, selected: bool, strong: bool) -> Response {
    let parts = right.map(|(r, _)| [Part::Text(r)]);
    let right = right.zip(parts.as_ref()).map(|((_, c), p)| (&p[..], c));
    list_row_slot(ui, indent, text, right, 0.0, selected, strong).0
}

/// `list_row` with `slot` px kept free at the right edge (right of the dim right-hand parts), for
/// a painted marker; returns the row's response and the slot's rect (zero-width when slot is 0).
fn list_row_slot(
    ui: &mut Ui,
    indent: f32,
    text: &str,
    right: Option<(&[Part<'_>], Color32)>,
    slot: f32,
    selected: bool,
    strong: bool,
) -> (Response, Rect) {
    let (r, marker, cut) = list_row_cut(ui, indent, text, right, slot, selected, strong);
    (if cut { r.on_hover_text(text) } else { r }, marker)
}

/// `list_row_slot` without a hover of its own: also returns whether the text was cut (ending in
/// "…"), for a caller whose tooltip holds the whole text or gets it as its first line.
pub fn list_row_cut(
    ui: &mut Ui,
    indent: f32,
    text: &str,
    right: Option<(&[Part<'_>], Color32)>,
    slot: f32,
    selected: bool,
    strong: bool,
) -> (Response, Rect, bool) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), LIST_ROW_H), Sense::click());
    // Painted text has no accessible name; give the row one (screen readers and UI tests).
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    let p = ui.painter_at(rect);
    row_fill::paint(&p, rect, selected, resp.hovered());
    let mut right_w = if slot > 0.0 { slot + PAD_X + PARTS_GAP } else { 0.0 };
    if let Some((parts, col)) = right {
        let (total, pieces) = layout_parts(&p, parts, col);
        right_w = total + PAD_X + PARTS_GAP + slot;
        let mut x = rect.right() - PAD_X - slot - total;
        let cy = rect.center().y;
        for (i, piece) in pieces.into_iter().enumerate() {
            if i > 0 {
                x += GAP;
            }
            let w = piece.galley.size().x;
            p.galley(pos2(x, cy - piece.galley.size().y / 2.0), piece.galley, col);
            x += w;
            if let Some(up) = piece.arrow {
                paint_arrow(&p, pos2(x + ARROW_GAP + ARROW_W / 2.0, cy), up, col);
                x += ARROW_GAP + ARROW_W;
            }
        }
    }
    let text_rect = Rect::from_min_max(pos2(rect.left() + indent, rect.top()), pos2(rect.right() - right_w, rect.bottom()));
    let font = if strong { type_scale::strong(ui, type_scale::BODY) } else { type_scale::font(type_scale::BODY) };
    let col = if strong { color::strong() } else { color::text() };
    let cut = truncate::paint_cut(&p, pos2(text_rect.left(), rect.center().y), truncate::cut_job(text, font, col), text_rect.width());
    let marker = Rect::from_min_max(pos2(rect.right() - PAD_X - slot, rect.top()), pos2(rect.right() - PAD_X, rect.bottom()));
    (resp, marker, cut.cut)
}

/// The space between two right-hand parts (a space's width at 12 px).
const GAP: f32 = 6.0;
/// The least space between a row's (cut) text and its right-hand parts or marker slot.
const PARTS_GAP: f32 = spacing::GRID;
/// The space between a count and its arrow, and the arrow's width.
const ARROW_GAP: f32 = 1.5;
const ARROW_W: f32 = 6.0;

/// A laid-out right-hand part: its text, and for a count, which way its arrow points.
struct Piece {
    galley: Arc<Galley>,
    arrow: Option<bool>,
}

/// Lays out `parts` in the 12 px dim font; returns their total width (gaps included) and pieces.
fn layout_parts(p: &Painter, parts: &[Part<'_>], col: Color32) -> (f32, Vec<Piece>) {
    let font = type_scale::font(type_scale::SMALL);
    let pieces: Vec<Piece> = parts
        .iter()
        .map(|part| {
            let (text, arrow) = match *part {
                Part::Text(t) => (t.to_owned(), None),
                Part::Ahead(n) => (n.to_string(), Some(true)),
                Part::Behind(n) => (n.to_string(), Some(false)),
            };
            Piece { galley: p.layout_no_wrap(text, font.clone(), col), arrow }
        })
        .collect();
    let widths: f32 = pieces.iter().map(|pc| pc.galley.size().x + if pc.arrow.is_some() { ARROW_GAP + ARROW_W } else { 0.0 }).sum();
    let gaps = GAP * pieces.len().saturating_sub(1) as f32;
    (widths + gaps, pieces)
}

/// A small arrow centred on `c`, pointing up (ahead) or down (behind). Painted because the
/// bundled fonts have no arrow glyphs.
fn paint_arrow(p: &Painter, c: Pos2, up: bool, col: Color32) {
    let s = Stroke::new(1.3_f32, col);
    // The down arrow sits half a pixel higher: its tip's round caps reach lower than a stem end.
    let (tip, tail) = if up { (c.y - 3.5, c.y + 3.5) } else { (c.y + 3.0, c.y - 4.0) };
    let back = if up { 2.6 } else { -2.6 };
    p.line_segment([pos2(c.x, tail), pos2(c.x, tip)], s);
    p.line_segment([pos2(c.x - 2.6, tip + back), pos2(c.x, tip)], s);
    p.line_segment([pos2(c.x + 2.6, tip + back), pos2(c.x, tip)], s);
}

/// The height of a `list_row`, and of a file list's rows (`file_row`).
pub const LIST_ROW_H: f32 = 36.0;
/// The height of a sidebar section header (`widgets::section_header`).
pub const SECTION_H: f32 = 36.0;
/// The height of a remote's or a Git Flow group's row.
pub const GROUP_ROW_H: f32 = 32.0;
/// The sidebar's inset: the filter field's from the panel's edges, the section chevrons' centre,
/// and the right-hand parts' and counts' from the right edge.
pub const PAD_X: f32 = spacing::PANEL_PAD_F;
/// Where a section header's title starts.
pub const TITLE_X: f32 = 32.0;
/// Where a list row's text starts.
pub const ROW_INDENT: f32 = 40.0;
/// Where a row nested under a remote or a Git Flow group starts.
pub const NESTED_INDENT: f32 = 48.0;
/// The centre of a row's hidden or show-only marker (`ref_visibility::row_marker`), left of its text.
pub const GUTTER_X: f32 = 24.0;
/// The same for a nested row.
pub const NESTED_GUTTER_X: f32 = 32.0;

/// For a `list_row` about to be drawn: when it would be scrolled out of view, takes its space
/// (and one widget id, as the row does) and returns true, so the caller skips it. Long sections
/// of the left panel then cost little more than their visible rows.
pub fn list_row_hidden(ui: &mut Ui) -> bool {
    let size = vec2(ui.available_width(), LIST_ROW_H);
    if ui.is_rect_visible(Rect::from_min_size(ui.cursor().min, size)) {
        return false;
    }
    ui.allocate_space(size);
    true
}

/// How far a file row's selection reaches past each side of the row, into the list's panel
/// padding (every host panel has at least this much), so the content does not move.
pub const FILE_ROW_BLEED: f32 = spacing::GRID;

/// One row of a file list drawn with `ScrollArea::show_rows(ui, LIST_ROW_H, ..)`: `add`'s widgets
/// left to right, vertically centred in a row exactly `LIST_ROW_H` tall. The whole row is one
/// click target named `name`: `.response` is the row's click response, and only real buttons
/// inside it take their own clicks. `selected` paints the selection across the row.
pub fn file_row<R>(ui: &mut Ui, selected: bool, name: &str, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    file_row_in(ui, selected, name, Sense::click(), Layout::left_to_right(Align::Center), add)
}

/// `file_row` for a list that takes the keyboard focus as a whole (`ui/file_list_keys.rs`): the
/// row takes clicks but is no Tab stop of its own.
pub fn keyed_file_row<R>(ui: &mut Ui, selected: bool, name: &str, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    file_row_in(ui, selected, name, Sense::CLICK, Layout::left_to_right(Align::Center), add)
}

/// `file_row` with an `end` part flush with the row's right edge (a file's line counts): `end` is
/// laid out first, right to left, so it keeps its width and `add`'s widgets (the path) are cut
/// before it. `.response` is the whole row's click response, as for `file_row`.
pub fn file_row_with_end<R>(
    ui: &mut Ui,
    selected: bool,
    name: &str,
    end: impl FnOnce(&mut Ui),
    add: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    file_row_ended(ui, selected, name, Sense::click(), end, add)
}

/// `file_row_with_end` for a list that takes the keyboard focus as a whole (`ui/wip_keys.rs`):
/// the row takes clicks but is no Tab stop of its own.
pub fn keyed_file_row_with_end<R>(
    ui: &mut Ui,
    selected: bool,
    name: &str,
    end: impl FnOnce(&mut Ui),
    add: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    file_row_ended(ui, selected, name, Sense::CLICK, end, add)
}

/// The focus ring of a focused list's current row (`row`, a file row's rect), across the same
/// area as its selection fill.
pub fn file_row_focus(ui: &Ui, row: Rect) {
    let clip = ui.clip_rect().expand2(vec2(FILE_ROW_BLEED, 0.0));
    let p = Painter::new(ui.ctx().clone(), ui.layer_id(), clip);
    tokens::focus_ring_inset(&p, row.expand2(vec2(FILE_ROW_BLEED, 0.0)), 0);
}

fn file_row_ended<R>(
    ui: &mut Ui,
    selected: bool,
    name: &str,
    sense: Sense,
    end: impl FnOnce(&mut Ui),
    add: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    file_row_in(ui, selected, name, sense, Layout::right_to_left(Align::Center), |ui| {
        end(ui);
        ui.with_layout(Layout::left_to_right(Align::Center), add).inner
    })
}

/// The file rows' core: the row is allocated (and so registered for clicks) before its
/// contents, so the buttons added later sit on top and win their own clicks.
fn file_row_in<R>(
    ui: &mut Ui,
    selected: bool,
    name: &str,
    sense: Sense,
    layout: Layout,
    add: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), LIST_ROW_H), sense);
    let enabled = ui.is_enabled();
    resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, enabled, selected, name));
    if ui.is_rect_visible(rect) {
        // Wider than the list, but no taller, so rows scrolled out of view stay hidden.
        let clip = ui.clip_rect().expand2(vec2(FILE_ROW_BLEED, 0.0));
        let p = Painter::new(ui.ctx().clone(), ui.layer_id(), clip);
        // A list that takes the focus as a whole (its rows are no Tab stops) shows its selection
        // as focused only while it has the focus.
        let active = sense.is_focusable() || super::list_focus::rows_active(ui.ctx());
        // `contains_pointer` stays true while the pointer is over the row's buttons.
        row_fill::paint_in(&p, rect.expand2(vec2(FILE_ROW_BLEED, 0.0)), selected, active, resp.contains_pointer());
    }
    let mut child = ui.new_child(UiBuilder::new().max_rect(rect).layout(layout));
    // Labels only sense hover then, so their clicks reach the row; their tooltips still show.
    child.style_mut().interaction.selectable_labels = false;
    child.set_min_height(LIST_ROW_H);
    InnerResponse::new(add(&mut child), resp)
}

/// A file row's path (or a commit's summary), painted with no accessible name of its own (the
/// row has it) and cut with "…" to the room left, whole on hover. It takes a selectable label's
/// size, so rows lay out as they did; a selected row's text is strong.
pub fn row_text(ui: &mut Ui, text: RichText, selected: bool) -> Response {
    row_text_reserving(ui, text, selected, 0.0)
}

/// `row_text` that leaves `reserve` px after it for what follows on the row (a file's line
/// counts), cutting the text sooner.
pub fn row_text_reserving(ui: &mut Ui, text: RichText, selected: bool, reserve: f32) -> Response {
    let (resp, elided) = paint_row_text(ui, text, selected, reserve);
    match elided {
        Some(whole) => resp.on_hover_text(whole),
        None => resp,
    }
}

/// `row_text_reserving` whose hover text is always `hover`, cut or not (a tree's file row,
/// which shows only the file's name: its whole path).
pub fn row_text_hover(ui: &mut Ui, text: RichText, selected: bool, reserve: f32, hover: &str) -> Response {
    paint_row_text(ui, text, selected, reserve).0.on_hover_text(hover)
}

/// The row text painted; with its whole text when it was cut.
fn paint_row_text(ui: &mut Ui, text: RichText, selected: bool, reserve: f32) -> (Response, Option<String>) {
    let pad = ui.spacing().button_padding;
    let wrap_w = (ui.available_width() - 2.0 * pad.x - reserve).max(0.0);
    let galley = WidgetText::from(text).into_galley(ui, Some(TextWrapMode::Truncate), wrap_w, TextStyle::Button);
    let mut size = galley.size() + 2.0 * pad;
    size.y = size.y.max(ui.spacing().interact_size.y);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    if ui.is_rect_visible(rect) {
        let col = if selected { color::strong() } else { ui.visuals().text_color() };
        let pos = ui.layout().align_size_within_rect(galley.size(), rect.shrink2(pad)).min;
        ui.painter().galley(pos, galley.clone(), col);
    }
    let whole = galley.elided.then(|| galley.text().to_owned());
    (resp, whole)
}

#[cfg(test)]
#[path = "list_row_tests.rs"]
mod tests;
