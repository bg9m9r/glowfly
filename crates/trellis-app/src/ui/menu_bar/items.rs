//! Menu items with their key on the right and, when disabled, a short form of the reason after
//! the label (the whole one on hover), and check items with a check box in the gutter before
//! the label. A disabled submenu is drawn as a disabled item: it can't open.

use crate::keymap::{BindingId, Keymap};
use crate::msg::Msg;
use crate::ui::theme::controls::paint_check_box;
use crate::ui::widgets::reason::{self, ItemState};
use eframe::egui::{self, Color32, Rect, TextStyle, WidgetInfo, WidgetText, WidgetType, pos2, vec2};

/// One item: `label`, the current key of `key` right-aligned, disabled with `blocked` as the
/// hover text, pushing `msg` and closing the menu when clicked.
pub(super) fn item(
    ui: &mut egui::Ui,
    keys: &Keymap,
    label: impl Into<WidgetText>,
    key: Option<BindingId>,
    blocked: Option<&str>,
    msg: Msg,
    msgs: &mut Vec<Msg>,
) {
    if clicked(ui, label, hint(keys, key), blocked) {
        msgs.push(msg);
        ui.close_menu();
    }
}

/// A check item: a check box showing `on`, then `label`, with `key`'s current key on the right;
/// disabled with `blocked` as the hover text. Returns whether it was clicked (the caller acts and
/// closes the menu). Screen readers get a checkbox with its state.
pub(super) fn check_item(ui: &mut egui::Ui, keys: &Keymap, label: &str, on: bool, key: Option<BindingId>, blocked: Option<&str>) -> bool {
    let font = TextStyle::Button.resolve(ui.style());
    // Placeholder colour: the button paints the text in its own interaction colour.
    let mut job = egui::text::LayoutJob::simple_singleline(label.to_owned(), font, Color32::PLACEHOLDER);
    let gutter = ui.spacing().icon_width + ui.spacing().icon_spacing;
    if let Some(first) = job.sections.first_mut() {
        first.leading_space = gutter;
    }
    let enabled = blocked.is_none();
    let r = reason::menu_button(ui, job, hint(keys, key), ItemState::blocked_by(blocked));
    let pad = ui.spacing().button_padding.x;
    let gutter_rect = Rect::from_min_size(pos2(r.rect.left() + pad, r.rect.top()), vec2(ui.spacing().icon_width, r.rect.height()));
    paint_check_box(ui, gutter_rect, on, r.hovered());
    r.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, on, label));
    r.clicked()
}

/// Adds an item of `label` with `shortcut` at the right, enabled unless `blocked` (then a short
/// form of the reason follows the label); whether it was clicked.
pub(super) fn clicked(ui: &mut egui::Ui, label: impl Into<WidgetText>, shortcut: impl Into<WidgetText>, blocked: Option<&str>) -> bool {
    reason::menu_button(ui, label, shortcut, ItemState::blocked_by(blocked)).clicked()
}

/// The key text an item shows for `key`: its first key in the keymap, or nothing.
pub(in crate::ui) fn hint(keys: &Keymap, key: Option<BindingId>) -> String {
    key.and_then(|k| keys.hint(k)).unwrap_or_default().to_owned()
}

/// A submenu, or while `blocked` a disabled plain item with the reason's short form and no
/// arrow (it can't open, and egui's submenu button can't make room for the suffix).
pub(super) fn submenu_if(ui: &mut egui::Ui, label: &str, blocked: Option<&str>, add_contents: impl FnOnce(&mut egui::Ui)) {
    match blocked {
        Some(why) => {
            reason::menu_button(ui, label, "", ItemState::Blocked(why.into()));
        }
        None => {
            ui.menu_button(label, add_contents);
        }
    }
}
