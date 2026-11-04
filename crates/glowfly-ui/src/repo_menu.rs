//! The toolbar repository menu's workspace commands, after Dashboard: finding repositories in
//! project folders, opening every favorite in a tab, and fetching or pulling the listed
//! repositories.

use super::common::menu_item;
use crate::widgets::menu::{separator, submenu};
use eframe::egui;
use glowfly_core::i18n::Lang;
use glowfly_model::msg::{Msg, RecentMsg, ReposMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::repo_lists::{favorites, unopened_favorites};
use glowfly_model::state::repo_sync::{SyncKind, SyncScope};

pub fn actions(ui: &mut egui::Ui, t: Lang, st: &AppState, msgs: &mut Vec<Msg>) {
    menu_item(msgs, ui, t.tr("toolbar.repo_menu.find"), Msg::Repos(ReposMsg::ShowFinder));
    if !unopened_favorites(st).is_empty() {
        menu_item(msgs, ui, t.tr("toolbar.repo_menu.open_favorites"), Msg::Recent(RecentMsg::OpenAll(favorites(&st.settings))));
    }
    if !st.settings.favorite_repos.is_empty() || !st.recent.is_empty() {
        submenu(ui, t.tr("toolbar.repo_menu.sync_menu"), |ui| sync_items(ui, t, st, msgs));
    }
}

/// Fetch or pull the favorites (when there are any) or every listed repository.
fn sync_items(ui: &mut egui::Ui, t: Lang, st: &AppState, msgs: &mut Vec<Msg>) {
    let sync = |kind, scope| Msg::Repos(ReposMsg::StartSync { kind, scope });
    if !st.settings.favorite_repos.is_empty() {
        menu_item(msgs, ui, t.tr("toolbar.repo_menu.sync_fetch_favorites"), sync(SyncKind::Fetch, SyncScope::Favorites));
        menu_item(msgs, ui, t.tr("toolbar.repo_menu.sync_pull_favorites"), sync(SyncKind::Pull, SyncScope::Favorites));
        separator(ui);
    }
    menu_item(msgs, ui, t.tr("toolbar.repo_menu.sync_fetch_listed"), sync(SyncKind::Fetch, SyncScope::Listed));
    menu_item(msgs, ui, t.tr("toolbar.repo_menu.sync_pull_listed"), sync(SyncKind::Pull, SyncScope::Listed));
}
