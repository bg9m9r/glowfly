//! A repository's colour swatch: a small rounded square in its hue, painted on its tab, before
//! its path in the repository lists and on the Color submenu's rows. Square, so it never reads
//! as a branch's round dot.

use super::theme::color;
use super::theme::type_scale;
use eframe::egui::{self, Color32, CornerRadius, Painter, Pos2, Rect, Response, Sense, Ui, pos2, vec2};

/// The swatch's size in lists and menus.
pub const SIZE: f32 = 10.0;
/// The room a swatch takes before a label or path.
pub const ROOM: f32 = 16.0;

/// Paints a swatch `size` across in `c`, its left edge's middle at `left_centre`.
pub fn paint(p: &Painter, left_centre: Pos2, size: f32, c: Color32) {
    let rect = Rect::from_min_size(pos2(left_centre.x, left_centre.y - size / 2.0), vec2(size, size));
    p.rect_filled(rect, CornerRadius::same((size * 0.25).round() as u8), c);
}

/// A menu row with the swatch in `c` and `label`: full width, a hover fill, named for screen
/// readers. With no colour the swatch's room stays empty, so labels line up.
pub fn menu_row(ui: &mut Ui, label: &str, c: Option<Color32>) -> Response {
    let pad = ui.spacing().button_padding;
    let ink = color::text();
    let galley = ui.painter().layout_no_wrap(label.to_owned(), type_scale::font(type_scale::BODY), ink);
    let w = pad.x * 2.0 + ROOM + galley.size().x;
    let h = ui.spacing().interact_size.y.max(galley.size().y + 2.0 * pad.y);
    // As a button does: a justified layout (a menu) widens the row to its width.
    let (rect, resp) = ui.allocate_at_least(vec2(w, h), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        if resp.hovered() {
            p.rect_filled(rect, ui.visuals().widgets.hovered.corner_radius, color::hover());
        }
        if let Some(c) = c {
            paint(p, pos2(rect.left() + pad.x, rect.center().y), SIZE, c);
        }
        p.galley(pos2(rect.left() + pad.x + ROOM, rect.center().y - galley.size().y / 2.0), galley, ink);
    }
    resp
}
