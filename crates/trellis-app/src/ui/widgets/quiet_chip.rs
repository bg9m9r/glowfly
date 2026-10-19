//! A quiet chip: small text on a soft toolbar fill, with no outline, that can be clicked (a
//! commit's SHA, its parents, its committer). It darkens on hover and while pressed, shows the
//! accent ring when it has the keyboard focus, and shows the pointing hand. Callers attach the
//! tooltip and any menu.

use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use eframe::egui::{self, CornerRadius, Response, Sense, Stroke, StrokeKind, Ui, WidgetText, pos2, vec2};

/// The chip's height.
const CHIP_H: f32 = 22.0;
/// Between the chip's sides and its text.
const TEXT_PAD: f32 = 8.0;
const RADIUS: u8 = 4;

/// Draws `text` (SMALL unless it sets its own font, in its own colour or the text colour) as a
/// chip named `name` for accessibility; returns its click response.
pub fn quiet_chip(ui: &mut Ui, text: impl Into<WidgetText>, name: &str) -> Response {
    let galley = text.into().into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, type_scale::font(type_scale::SMALL));
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 2.0 * TEXT_PAD, CHIP_H), Sense::click());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let radius = CornerRadius::same(RADIUS);
        let fill = if resp.is_pointer_button_down_on() {
            color::selected()
        } else if resp.hovered() {
            color::hover()
        } else {
            color::toolbar()
        };
        p.rect_filled(rect, radius, fill);
        if resp.has_focus() {
            p.rect_stroke(rect, radius, Stroke::new(1.5_f32, color::accent()), StrokeKind::Inside);
        }
        let pos = pos2(rect.left() + TEXT_PAD, rect.center().y - galley.size().y / 2.0);
        p.galley(pos, galley, color::text());
    }
    let name = name.to_owned();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &name));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}
