//! The sidebar's list rows: one line of text with optional dim text and a marker slot at the
//! right, plus the skip-if-scrolled-away helper the long sections use.

use super::theme::color;

use eframe::egui::{self, Color32, FontId, Rect, Response, Sense, Ui, pos2, vec2};

/// One sidebar list row. Returns the click response; `right` is optional dim text at the right.
pub fn list_row(ui: &mut Ui, indent: f32, text: &str, right: Option<(&str, Color32)>, selected: bool, strong: bool) -> Response {
    list_row_slot(ui, indent, text, right, 0.0, selected, strong).0
}

/// `list_row` with `slot` px kept free at the right edge (right of the dim right-hand text), for
/// a painted marker; returns the row's response and the slot's rect (zero-width when slot is 0).
pub fn list_row_slot(
    ui: &mut Ui,
    indent: f32,
    text: &str,
    right: Option<(&str, Color32)>,
    slot: f32,
    selected: bool,
    strong: bool,
) -> (Response, Rect) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), LIST_ROW_H), Sense::click());
    // Painted text has no accessible name; give the row one (screen readers and UI tests).
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    let p = ui.painter_at(rect);
    if selected {
        p.rect_filled(rect, 0.0, color::selected());
    } else if resp.hovered() {
        p.rect_filled(rect, 0.0, color::hover());
    }
    let mut right_w = if slot > 0.0 { slot + 14.0 } else { 0.0 };
    if let Some((r, col)) = right {
        let g = p.layout_no_wrap(r.to_owned(), FontId::proportional(12.0), col);
        right_w = g.size().x + 14.0 + slot;
        let y = rect.center().y - g.size().y / 2.0;
        p.galley(pos2(rect.right() - g.size().x - 10.0 - slot, y), g, col);
    }
    let text_rect = Rect::from_min_max(pos2(rect.left() + indent, rect.top()), pos2(rect.right() - right_w, rect.bottom()));
    let font = if strong { FontId::new(13.0, egui::FontFamily::Proportional) } else { FontId::proportional(13.0) };
    let col = if strong { color::strong() } else { color::text() };
    let g = p.layout_no_wrap(text.to_owned(), font, col);
    let clipped = p.with_clip_rect(text_rect);
    clipped.galley(pos2(text_rect.left(), rect.center().y - g.size().y / 2.0), g, col);
    let marker = Rect::from_min_max(pos2(rect.right() - 10.0 - slot, rect.top()), pos2(rect.right() - 10.0, rect.bottom()));
    (resp, marker)
}

/// The height of a `list_row`.
pub const LIST_ROW_H: f32 = 24.0;

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
