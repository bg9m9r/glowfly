//! The Move to group submenu of a favorite's row menu on the welcome screen (the way to group
//! favorites without dragging): Not in a group, then one item per group with its swatch in the
//! icon column, the favorite's own choice checked in the state column, then New group…, which
//! opens the New group dialog with the favorite moving into the group it makes.

use super::repo_swatch;
use super::theme::color;
use crate::msg::{GroupMsg, Msg};
use crate::ui::widgets::menu::{self, separator, submenu};
use eframe::egui;
use glowfly_core::i18n::Lang;
use glowfly_core::settings::RepoGroup;
use std::path::Path;

/// The submenu for the favorite at `path`, now in group `current` (None: no group).
pub fn items(ui: &mut egui::Ui, t: Lang, groups: &[RepoGroup], path: &Path, current: Option<usize>, msgs: &mut Vec<Msg>) {
    let to = |group| Msg::Groups(GroupMsg::MoveRepo { path: path.to_path_buf(), group, index: usize::MAX });
    submenu(ui, t.tr("repos.row_menu.move_to_group"), |ui| {
        let swatches = groups.iter().any(|g| g.hue().is_some());
        let mut none = menu::item(t.tr("repos.row_menu.no_group")).current(current.is_none());
        if swatches {
            none = none.icon(repo_swatch::paint_slot, None);
        }
        if none.show(ui).clicked() && current.is_some() {
            msgs.push(to(None));
            ui.close_menu();
        }
        separator(ui);
        for (i, g) in groups.iter().enumerate() {
            let mut row = menu::item(g.name.as_str()).current(current == Some(i));
            if swatches {
                row = row.icon(repo_swatch::paint_slot, g.hue().map(color::repo_hue));
            }
            if row.show(ui).clicked() && current != Some(i) {
                msgs.push(to(Some(i)));
                ui.close_menu();
            }
        }
        if !groups.is_empty() {
            separator(ui);
        }
        if menu::item(t.tr("repos.row_menu.new_group")).show(ui).clicked() {
            msgs.push(Msg::Groups(GroupMsg::NewDialog { repo: Some(path.to_path_buf()) }));
            ui.close_menu();
        }
    });
}
