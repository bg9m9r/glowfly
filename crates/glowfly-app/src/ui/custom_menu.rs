//! The Commands submenu of the commit, local branch and file menus: the custom commands shown
//! there (`state::custom_commands::MenuCommands` decides each item), after a separator, or
//! nothing when none apply. A disabled item says why on hover; an enabled one shows the
//! command it runs, or that it asks to trust the repository first.

use super::widgets::reason::{self, ItemState};
use super::widgets::tooltip;
use crate::custom_commands::Target;
use crate::msg::Msg;
use crate::state::custom_commands::MenuCommands;
use eframe::egui;
use glowfly_core::custom_commands::Place;
use glowfly_core::i18n::Lang;

pub fn group(ui: &mut egui::Ui, t: Lang, cmds: &MenuCommands, place: Place, at: Target, msgs: &mut Vec<Msg>) {
    let items = cmds.items(place, &at, cfg!(windows));
    if items.is_empty() {
        return;
    }
    ui.separator();
    ui.menu_button(t.tr("commands.menu.title"), |ui| {
        for it in items {
            let mut r = reason::menu_button(ui, it.label.as_str(), "", ItemState::blocked_by(it.blocked));
            if let (None, Some(tip)) = (it.blocked, it.tip.as_deref()) {
                r = tooltip::attach_text(r, tip);
            }
            if r.clicked() {
                msgs.push(it.msg);
                ui.close_menu();
            }
        }
    });
}
