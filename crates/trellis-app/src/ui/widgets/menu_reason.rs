//! Menu items that say why they are disabled, or that they open a setting first:
//!
//! - a blocked item has its label greyed (`color::faint()`) and a painted info mark at its right
//!   edge (left of its shortcut, when it has one). The whole reason shows in the shared tooltip
//!   on hover, or under the item while it has the keyboard focus, and screen readers get it as
//!   the item's description. The item takes the focus but never clicks;
//! - a setup item stays enabled with a weak `SET_UP` after its label, and its note in the same
//!   tooltip.
//!
//! The reason is never text in the menu, so a long one doesn't widen it, and the item's
//! accessible name stays its label.

use super::super::access;
use super::super::theme::{color, type_scale};
use super::{Icon, paint_icon_sized, tooltip};
use eframe::egui::{
    Align, Button, FontId, NumExt, Painter, Rect, Response, Sense, Stroke, StrokeKind, TextStyle, TextWrapMode, Ui, WidgetText, pos2, vec2,
};

/// A setup item's suffix: its text size and the space between the label and it.
pub const SUFFIX_SIZE: f32 = type_scale::SMALL;
pub const SUFFIX_GAP: f32 = 10.0;
/// The suffix of an entry whose program or setting isn't set yet (it opens that setting).
pub const SET_UP: &str = "Set up…";
/// The info mark's width, the least space between a blocked label and it, and the room it adds
/// to an item.
pub const INFO_W: f32 = 14.0;
pub const INFO_GAP: f32 = 12.0;
pub const INFO_ROOM: f32 = INFO_GAP + INFO_W;

/// A menu item's state: usable, disabled with the whole reason, or usable while what it needs
/// isn't set up yet (carrying the hover note; it opens the setting).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemState<'a> {
    Ready,
    Blocked(&'a str),
    Setup(&'a str),
}

impl<'a> ItemState<'a> {
    /// Blocked by `why` when there is a reason, else ready.
    pub fn blocked_by(why: Option<&'a str>) -> Self {
        why.map_or(Self::Ready, Self::Blocked)
    }
}

/// A menu item: `label` (never wrapped), `shortcut` at the right when not empty. A blocked item
/// is greyed with an info mark and its reason in a tooltip; a setup item stays enabled with a
/// weak "Set up…" after its label, the menu widened to fit it.
pub fn menu_button(ui: &mut Ui, label: impl Into<WidgetText>, shortcut: impl Into<WidgetText>, state: ItemState<'_>) -> Response {
    let shortcut = shortcut.into();
    match state {
        ItemState::Ready => {
            let mut b = Button::new(label).wrap_mode(TextWrapMode::Extend);
            if !shortcut.is_empty() {
                b = b.shortcut_text(shortcut);
            }
            ui.add(b)
        }
        ItemState::Blocked(why) => blocked_item(ui, label.into(), shortcut, why),
        ItemState::Setup(note) => setup_item(ui, label.into(), shortcut, note),
    }
}

/// An enabled item with the weak `SET_UP` after its label and `note` in the tooltip.
fn setup_item(ui: &mut Ui, label: WidgetText, shortcut: WidgetText, note: &str) -> Response {
    // Placeholder colour (unless the label has its own): egui paints the text colour.
    let label = label.into_galley(ui, Some(TextWrapMode::Extend), f32::INFINITY, TextStyle::Button);
    let suffix = ui.fonts(|f| f.layout_no_wrap(SET_UP.to_owned(), FontId::proportional(SUFFIX_SIZE), color::weak()));
    let s = ui.spacing();
    let (pad, spacing) = (s.button_padding.x, s.item_spacing.x);
    let label_w = label.size().x;
    let mut min_w = 2.0 * pad + label_w + SUFFIX_GAP + suffix.size().x;
    let mut button = Button::new(WidgetText::Galley(label)).wrap_mode(TextWrapMode::Extend);
    if !shortcut.is_empty() {
        let key = shortcut.clone().into_galley(ui, Some(TextWrapMode::Extend), f32::INFINITY, TextStyle::Button);
        min_w += spacing + key.size().x;
        button = button.shortcut_text(shortcut);
    }
    let r = ui.add(button.min_size(vec2(min_w, 0.0)));
    let at = pos2(r.rect.left() + pad + label_w + SUFFIX_GAP, r.rect.center().y - suffix.size().y / 2.0);
    ui.painter().galley(at, suffix, color::weak());
    tooltip::attach_text(r, note)
}

/// A blocked item: laid out as egui's `Button` lays out its text and shortcut, plus the info
/// mark's room. It is allocated in the enabled ui with hover and focus sense only, so the
/// keyboard reaches it and nothing clicks it; screen readers hear it as disabled.
fn blocked_item(ui: &mut Ui, label: WidgetText, shortcut: WidgetText, why: &str) -> Response {
    let galley = label.into_galley(ui, Some(TextWrapMode::Extend), f32::INFINITY, TextStyle::Button);
    let key = (!shortcut.is_empty()).then(|| shortcut.into_galley(ui, Some(TextWrapMode::Extend), f32::INFINITY, TextStyle::Button));
    let s = ui.spacing();
    let (pad, spacing, min_h) = (s.button_padding, s.item_spacing.x, s.interact_size.y);
    let key_room = key.as_ref().map_or(0.0, |k| spacing + k.size().x);
    let w = 2.0 * pad.x + galley.size().x + INFO_ROOM + key_room;
    let text_h = key.as_ref().map_or(galley.size().y, |k| galley.size().y.max(k.size().y));
    let h = (text_h + 2.0 * pad.y).at_least(min_h);
    let (rect, resp) = ui.allocate_at_least(vec2(w, h), Sense::hover() | Sense::FOCUSABLE);
    let name = galley.text().to_owned();
    if ui.is_rect_visible(rect) {
        let inner = rect.shrink2(pad);
        let painter = ui.painter();
        let mut at = ui.layout().align_size_within_rect(galley.size(), inner).min;
        if key.is_some() {
            at.x = rect.min.x + pad.x;
        }
        painter.galley_with_override_text_color(at, galley, color::faint());
        if let Some(key) = key {
            // Right-aligned, as `Button` places a shortcut.
            let layout = if ui.layout().is_horizontal() {
                ui.layout().with_main_align(Align::Max)
            } else {
                ui.layout().with_cross_align(Align::Max)
            };
            let at = layout.align_size_within_rect(key.size(), inner).min;
            painter.galley_with_override_text_color(at, key, color::faint());
        }
        paint_info(painter, rect, pad.x + key_room, resp.hovered() || resp.has_focus());
        paint_focus(ui, &resp);
    }
    access::name_button(&resp, false, &name);
    explain(resp, why)
}

/// Paints the info mark at `row`'s right, `right_inset` in from its edge, centred on it:
/// `color::text()` while `lit` (the row hovered or focused), else `color::weak()`.
pub fn paint_info(p: &Painter, row: Rect, right_inset: f32, lit: bool) {
    let c = pos2(row.right() - right_inset - INFO_W / 2.0, row.center().y);
    paint_icon_sized(p, c, Icon::Info, if lit { color::text() } else { color::weak() }, INFO_W);
}

/// The accent ring inside `resp`'s rect while it has the keyboard focus.
pub fn paint_focus(ui: &Ui, resp: &Response) {
    if resp.has_focus() {
        let radius = ui.visuals().widgets.hovered.corner_radius;
        ui.painter().rect_stroke(resp.rect, radius, Stroke::new(1.5_f32, color::accent()), StrokeKind::Inside);
    }
}

/// Gives a blocked row `why` as its description and in the tooltip on hover or focus.
pub fn explain(resp: Response, why: &str) -> Response {
    access::describe(&resp, why);
    tooltip::attach_text(resp, why)
}

#[cfg(test)]
#[path = "menu_reason_tests.rs"]
mod tests;
