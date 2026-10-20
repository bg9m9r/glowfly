//! A branch's colour dot: a small filled circle in its line tone, painted left of its name in the
//! sidebar's branch rows, the Branch picker's menu and the commit details header.

use super::theme::color;
use super::theme::type_scale;
use eframe::egui::{self, Color32, Painter, Rect, Response, Sense, Ui, pos2, vec2};

/// The room a dot takes before a name.
pub const DOT_ROOM: f32 = 14.0;
const DOT_R: f32 = 4.0;

/// Paints a dot in colour `c`, `indent` from `row`'s left edge, centred on the row.
pub fn paint(p: &Painter, row: Rect, indent: f32, c: Color32) {
    p.circle_filled(pos2(row.left() + indent + DOT_R, row.center().y), DOT_R, c);
}

/// A menu row with the dot in colour `c` and `label`: full width, a hover fill, named for screen
/// readers. A disabled row doesn't click, shows its label weak and takes disabled hover text.
pub fn menu_row(ui: &mut Ui, label: &str, c: Color32, enabled: bool) -> Response {
    ui.add_enabled_ui(enabled, |ui| row(ui, label, c)).inner
}

fn row(ui: &mut Ui, label: &str, c: Color32) -> Response {
    let enabled = ui.is_enabled();
    let pad = ui.spacing().button_padding;
    let ink = if enabled { color::text() } else { color::weak() };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), type_scale::font(type_scale::BODY), ink);
    let w = pad.x * 2.0 + DOT_ROOM + galley.size().x;
    let h = ui.spacing().interact_size.y.max(galley.size().y + 2.0 * pad.y);
    // As a button does: a justified layout (a menu) widens the row to its width.
    let (rect, resp) = ui.allocate_at_least(vec2(w, h), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        if enabled && resp.hovered() {
            p.rect_filled(rect, ui.visuals().widgets.hovered.corner_radius, color::hover());
        }
        paint(p, rect, pad.x, c);
        p.galley(pos2(rect.left() + pad.x + DOT_ROOM, rect.center().y - galley.size().y / 2.0), galley, ink);
    }
    resp
}
