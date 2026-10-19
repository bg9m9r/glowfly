//! The header's controls: a ghost button (an icon, a label or both; transparent until hovered),
//! its width, the thin divider between groups, and the rich tooltip for icon-only controls.

use crate::keymap::Keymap;
use crate::state::tip::Tip;
use crate::ui::access;
use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use crate::ui::widgets::{Icon, paint_icon_sized, tooltip};
use eframe::egui::{Align2, Color32, CornerRadius, CursorIcon, FontId, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use trellis_core::i18n::Lang;

/// Every control's height and corners.
pub(super) const HEIGHT: f32 = 28.0;
pub(super) const RADIUS: u8 = 6;
/// Room either side of a labelled control's content, and between its icon and label.
const PAD_X: f32 = 8.0;
const ICON_GAP: f32 = 6.0;
/// Icons are drawn this size.
pub(super) const ICON: f32 = 16.0;
/// Space between the controls of one group.
pub(super) const SPACE: f32 = 4.0;
/// The divider's line height and the room either side of it.
const DIVIDER_H: f32 = 20.0;
const DIVIDER_PAD: f32 = 8.0;
/// A divider with its margins.
pub(super) const DIVIDER_W: f32 = 1.0 + 2.0 * DIVIDER_PAD;

pub(super) fn font() -> FontId {
    type_scale::font(type_scale::BODY)
}

/// The width of `text` in `font`.
pub(super) fn text_w(ui: &Ui, text: &str, font: FontId) -> f32 {
    ui.painter().layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER).size().x
}

/// The width a ghost control takes: a 28 px square for an icon alone, else its content with
/// 8 px either side. `extra` is room for something painted after the label (a chevron).
pub(super) fn ghost_width(ui: &Ui, icon: bool, label: Option<&str>, extra: f32) -> f32 {
    match label {
        None => HEIGHT,
        Some(text) => {
            let icon_w = if icon { ICON + ICON_GAP } else { 0.0 };
            2.0 * PAD_X + icon_w + text_w(ui, text, font()) + extra
        }
    }
}

/// A ghost control: transparent until hovered (`hover` fill, `selected` while pressed, an
/// accent ring when focused), `faint` when disabled, named `name` for screen readers. A
/// disabled control still senses hover and focus, so it can explain itself.
pub(super) fn ghost(ui: &mut Ui, icon: Option<Icon>, label: Option<&str>, name: &str, enabled: bool) -> Response {
    ghost_with(ui, icon, label, name, enabled, 0.0)
}

/// `ghost` with `extra` px after the label (the caller paints there).
pub(super) fn ghost_with(ui: &mut Ui, icon: Option<Icon>, label: Option<&str>, name: &str, enabled: bool, extra: f32) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() | Sense::FOCUSABLE };
    let w = ghost_width(ui, icon.is_some(), label, extra);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, HEIGHT), sense);
    let p = ui.painter();
    let radius = CornerRadius::same(RADIUS);
    if enabled && resp.is_pointer_button_down_on() {
        p.rect_filled(rect, radius, color::selected());
    } else if enabled && resp.hovered() {
        p.rect_filled(rect, radius, color::hover());
    }
    if resp.has_focus() {
        p.rect_stroke(rect, radius, Stroke::new(1.5_f32, color::accent()), StrokeKind::Inside);
    }
    let fg = if !enabled {
        color::faint()
    } else if resp.hovered() {
        color::strong()
    } else {
        color::text()
    };
    match label {
        None => {
            if let Some(icon) = icon {
                paint_icon_sized(p, rect.center(), icon, fg, ICON);
            }
        }
        Some(text) => {
            let mut x = rect.left() + PAD_X;
            if let Some(icon) = icon {
                paint_icon_sized(p, pos2(x + ICON / 2.0, rect.center().y), icon, fg, ICON);
                x += ICON + ICON_GAP;
            }
            p.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, text, font(), fg);
        }
    }
    access::name_button(&resp, enabled, name);
    if enabled { resp.on_hover_cursor(CursorIcon::PointingHand) } else { resp }
}

/// The 1 x 20 px line between two groups, with 8 px either side.
pub(super) fn divider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(DIVIDER_W, HEIGHT), Sense::hover());
    let x = rect.center().x.round() + 0.5;
    let y = rect.center().y;
    ui.painter().line_segment([pos2(x, y - DIVIDER_H / 2.0), pos2(x, y + DIVIDER_H / 2.0)], Stroke::new(1.0_f32, color::border()));
}

/// `resp` with the shared rich tooltip `tip`.
pub(super) fn icon_tip(resp: Response, keys: &Keymap, t: Lang, tip: Tip) -> Response {
    tooltip::attach(resp, keys, t, || tip)
}
