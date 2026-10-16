//! Menu items with their key on the right and, when disabled, the reason on hover.

use crate::keymap::{BindingId, Keymap};
use crate::msg::Msg;
use eframe::egui::{self, Button, WidgetText};

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
    let button = Button::new(label).shortcut_text(hint(keys, key));
    if clicked(ui, button, blocked) {
        msgs.push(msg);
        ui.close_menu();
    }
}

/// Adds `button`, enabled unless `blocked` (then the reason shows on hover); whether it was
/// clicked.
pub(super) fn clicked(ui: &mut egui::Ui, button: Button, blocked: Option<&str>) -> bool {
    let mut r = ui.add_enabled(blocked.is_none(), button);
    if let Some(why) = blocked {
        r = r.on_disabled_hover_text(why);
    }
    r.clicked()
}

/// The key text an item shows for `key`: its first key in the keymap, or nothing.
pub(in crate::ui) fn hint(keys: &Keymap, key: Option<BindingId>) -> String {
    key.and_then(|k| keys.hint(k)).unwrap_or_default().to_owned()
}

/// A submenu, disabled with `blocked` as the hover text.
pub(super) fn submenu_if(ui: &mut egui::Ui, label: &str, blocked: Option<&str>, add_contents: impl FnOnce(&mut egui::Ui)) {
    let r = ui.add_enabled_ui(blocked.is_none(), |ui| ui.menu_button(label, add_contents)).inner.response;
    if let Some(why) = blocked {
        r.on_disabled_hover_text(why);
    }
}
