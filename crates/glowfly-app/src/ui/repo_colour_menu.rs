//! The Color submenu that gives a repository its colour, shared by the tab menu and the
//! welcome screen's repository rows: None, then one row per repository hue with its swatch and
//! name, the current choice checked in the state column. A new tab (no repository yet) shows
//! a blocked item that says why.

use super::repo_swatch;
use super::theme::color;
use super::widgets::reason::{self, ItemState};
use crate::msg::{Msg, RecentMsg};
use crate::state::repo_colours::hue_name;
use crate::ui::widgets::menu::{self, separator, submenu};
use eframe::egui;
use glowfly_core::i18n::Lang;
use glowfly_core::settings::RepoColour;
use std::path::Path;

/// The Color item for the repository at `path` (None: a new tab), whose colour is `current`.
pub fn items(ui: &mut egui::Ui, t: Lang, path: Option<&Path>, current: Option<RepoColour>, msgs: &mut Vec<Msg>) {
    let Some(path) = path else {
        reason::menu_button(ui, t.tr("repo_colour.menu"), "", ItemState::Blocked(t.tr("repo_colour.blocked")));
        return;
    };
    let set = |colour| Msg::Recent(RecentMsg::SetColour { path: path.to_path_buf(), colour });
    submenu(ui, t.tr("repo_colour.menu"), |ui| {
        let none = menu::item(t.tr("repo_colour.none"))
            .current(current.is_none())
            .icon(repo_swatch::paint_slot, None)
            .show(ui)
            .on_hover_text(t.tr("repo_colour.none.tip"));
        if none.clicked() {
            msgs.push(set(None));
            ui.close_menu();
        }
        separator(ui);
        for c in RepoColour::ALL {
            let row = menu::item(hue_name(t, c)).current(current == Some(c));
            if row.icon(repo_swatch::paint_slot, Some(color::repo_hue(c))).show(ui).clicked() {
                msgs.push(set(Some(c)));
                ui.close_menu();
            }
        }
    });
}
