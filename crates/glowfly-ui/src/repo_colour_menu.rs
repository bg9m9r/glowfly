//! The Color submenu that gives a repository (or a repository group) its colour, shared by the
//! tab menu, the welcome screen's repository rows and its group headers: None, then one row per
//! repository hue with its swatch and name, the current choice checked in the state column. A
//! new tab (no repository yet) shows a blocked item that says why. For a repository in a
//! coloured group, None's tip says the group's colour shows instead.

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

/// The Color item for the repository at `path` (None: a new tab), whose own colour is
/// `current`; `grouped` when its group has a colour.
pub fn items(ui: &mut egui::Ui, t: Lang, path: Option<&Path>, (current, grouped): (Option<RepoColour>, bool), msgs: &mut Vec<Msg>) {
    let Some(path) = path else {
        reason::menu_button(ui, t.tr("repo_colour.menu"), "", ItemState::Blocked(t.tr("repo_colour.blocked")));
        return;
    };
    let set = |colour| Msg::Recent(RecentMsg::SetColour { path: path.to_path_buf(), colour });
    let none_tip = if grouped { t.tr("repo_colour.none.group_tip") } else { t.tr("repo_colour.none.tip") };
    hues(ui, t, current, none_tip, set, msgs);
}

/// The Color submenu: None (tipped `none_tip`), then each hue, `current` checked; a choice
/// pushes `set` of it. Shared by repositories and groups (`repo_list_groups`).
pub fn hues(
    ui: &mut egui::Ui,
    t: Lang,
    current: Option<RepoColour>,
    none_tip: &str,
    set: impl Fn(Option<RepoColour>) -> Msg,
    msgs: &mut Vec<Msg>,
) {
    submenu(ui, t.tr("repo_colour.menu"), |ui| {
        let none = menu::item(t.tr("repo_colour.none"))
            .current(current.is_none())
            .icon(repo_swatch::paint_slot, None)
            .show(ui)
            .on_hover_text(none_tip);
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
