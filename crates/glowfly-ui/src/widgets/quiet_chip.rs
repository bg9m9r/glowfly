//! A quiet chip: a pill of small text on a soft toolbar fill, with no outline, that can be
//! clicked (a commit's SHA, its parents, its committer). It is a quiet target over that fill
//! (`button_look::paint_target`): the hover layer, the pressed layer while held, and the inset
//! accent ring on its pill when it has the keyboard focus; and it shows the pointing hand.
//! Callers attach the tooltip and any menu.

use super::button_look::{Press, Target, paint_target};
use crate::ui::theme::color;
use crate::ui::theme::sizes::ControlSize;
use crate::ui::theme::tokens::radius;
use crate::ui::theme::type_scale;
use eframe::egui::{self, CornerRadius, Response, Sense, Ui, WidgetText, pos2, vec2};

/// The chip's height.
const CHIP_H: f32 = 22.0;
/// Between the chip's sides and its text.
const TEXT_PAD: f32 = 8.0;

/// Draws `text` (SMALL unless it sets its own font, in its own colour or the text colour) as a
/// chip named `name` for accessibility; returns its click response.
pub fn quiet_chip(ui: &mut Ui, text: impl Into<WidgetText>, name: &str) -> Response {
    let galley = text.into().into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, type_scale::font(type_scale::SMALL));
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 2.0 * TEXT_PAD, CHIP_H), Sense::click());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let corners = CornerRadius::same(radius::pill(CHIP_H));
        p.rect_filled(rect, corners, color::toolbar());
        let press = Press::of(resp.hovered(), resp.is_pointer_button_down_on(), false);
        // A chip's size: 22 px of small text, quiet, its focus ring inset.
        paint_target(p, Target { rect, corners, edge: None }, ControlSize::Compact, press, resp.has_focus());
        let pos = pos2(rect.left() + TEXT_PAD, rect.center().y - galley.size().y / 2.0);
        p.galley(pos, galley, color::text());
    }
    let name = name.to_owned();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &name));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}
