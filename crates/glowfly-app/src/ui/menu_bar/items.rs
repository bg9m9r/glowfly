//! The menu bar's item helpers: the key text an item shows, and submenus that, while disabled,
//! are drawn as a disabled item (greyed with an info mark, the reason in a tooltip on hover or
//! focus) that can't open.

use crate::keymap::Chord;
use crate::ui::widgets::menu::submenu;
use crate::ui::widgets::reason::{self, ItemState};
use eframe::egui;

/// The key text an item shows for its `chord` (the binding's first key, `menu_model::Item`), or
/// nothing.
pub(in crate::ui) fn hint(chord: Option<Chord>) -> String {
    chord.map(|c| c.text()).unwrap_or_default()
}

/// A submenu, or while `blocked` a disabled plain item with the info mark and no arrow (it
/// can't open).
pub(super) fn submenu_if(ui: &mut egui::Ui, label: &str, blocked: Option<&str>, add_contents: impl FnOnce(&mut egui::Ui)) {
    match blocked {
        Some(why) => {
            reason::menu_button(ui, label, "", ItemState::Blocked(why));
        }
        None => {
            submenu(ui, label, add_contents);
        }
    }
}
