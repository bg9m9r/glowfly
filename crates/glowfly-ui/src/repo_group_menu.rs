//! The Move to group submenu of a favorite's row menu on the welcome screen (the way to group
//! favorites without dragging): Not in a group, then one item per group with its swatch in the
//! icon column, the favorite's own choice checked in the state column, then New group…, which
//! opens the New group dialog with the favorite moving into the group it makes. A Recent row's
//! Add to group submenu has the same group items (none checked) and New group…, and stars it.

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
        if let Some(i) = group_rows(ui, groups, current, true) {
            msgs.push(to(Some(i)));
            ui.close_menu();
        }
        new_group(ui, t, path, msgs);
    });
}

/// The Add to group submenu of a Recent row at `path`: one item per group, then New group….
/// Either stars the repository as it goes in.
pub fn add_items(ui: &mut egui::Ui, t: Lang, groups: &[RepoGroup], path: &Path, msgs: &mut Vec<Msg>) {
    submenu(ui, t.tr("repos.row_menu.add_to_group"), |ui| {
        if let Some(group) = group_rows(ui, groups, None, false) {
            msgs.push(Msg::Groups(GroupMsg::AddRepos { group, paths: vec![path.to_path_buf()] }));
            ui.close_menu();
        }
        new_group(ui, t, path, msgs);
    });
}

/// One item per group, its swatch in the icon column while any group has a colour and, with
/// `choice`, `current` checked among them; then a separator. Returns the group picked other than
/// `current`.
fn group_rows(ui: &mut egui::Ui, groups: &[RepoGroup], current: Option<usize>, choice: bool) -> Option<usize> {
    let swatches = groups.iter().any(|g| g.hue().is_some());
    let mut picked = None;
    for (i, g) in groups.iter().enumerate() {
        let mut row = menu::item(g.name.as_str());
        if choice {
            row = row.current(current == Some(i));
        }
        if swatches {
            row = row.icon(repo_swatch::paint_slot, g.hue().map(color::repo_hue));
        }
        if row.show(ui).clicked() && current != Some(i) {
            picked = Some(i);
        }
    }
    if !groups.is_empty() {
        separator(ui);
    }
    picked
}

/// New group…, which opens the New group dialog with `path` going into the group it makes.
fn new_group(ui: &mut egui::Ui, t: Lang, path: &Path, msgs: &mut Vec<Msg>) {
    if menu::item(t.tr("repos.row_menu.new_group")).show(ui).clicked() {
        msgs.push(Msg::Groups(GroupMsg::NewDialog { repo: Some(path.to_path_buf()) }));
        ui.close_menu();
    }
}
