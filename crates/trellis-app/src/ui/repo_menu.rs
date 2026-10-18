//! The toolbar repository menu's list actions, after Dashboard: finding repositories in
//! project folders, opening every favorite in a tab, fetching or pulling the listed
//! repositories, and removing the entries that are gone.

use super::common::menu_item;
use crate::msg::{Msg, RecentMsg, ReposMsg};
use crate::state::AppState;
use crate::state::repo_lists::{favorites, missing_counts, remove_missing_msg, unopened_favorites};
use crate::state::repo_sync::{SyncKind, SyncScope};
use eframe::egui;
use trellis_core::i18n::Lang;

pub fn actions(ui: &mut egui::Ui, t: Lang, st: &AppState, msgs: &mut Vec<Msg>) {
    menu_item(msgs, ui, t.tr("toolbar.repo_menu.find"), Msg::Repos(ReposMsg::ShowFinder));
    if !unopened_favorites(st).is_empty() {
        menu_item(msgs, ui, t.tr("toolbar.repo_menu.open_favorites"), Msg::Recent(RecentMsg::OpenAll(favorites(&st.settings))));
    }
    if !st.settings.favorite_repos.is_empty() || !st.recent.is_empty() {
        ui.menu_button(t.tr("toolbar.repo_menu.sync_menu"), |ui| sync_items(ui, t, st, msgs));
    }
    let (recent, favs) = missing_counts(st);
    if let Some(ask) = remove_missing_msg(st) {
        menu_item(msgs, ui, t.trn("toolbar.repo_menu.remove_missing", recent + favs, &[]), ask);
    }
}

/// Fetch or pull the favorites (when there are any) or every listed repository.
fn sync_items(ui: &mut egui::Ui, t: Lang, st: &AppState, msgs: &mut Vec<Msg>) {
    let sync = |kind, scope| Msg::Repos(ReposMsg::StartSync { kind, scope });
    if !st.settings.favorite_repos.is_empty() {
        menu_item(msgs, ui, t.tr("toolbar.repo_menu.sync_fetch_favorites"), sync(SyncKind::Fetch, SyncScope::Favorites));
        menu_item(msgs, ui, t.tr("toolbar.repo_menu.sync_pull_favorites"), sync(SyncKind::Pull, SyncScope::Favorites));
        ui.separator();
    }
    menu_item(msgs, ui, t.tr("toolbar.repo_menu.sync_fetch_listed"), sync(SyncKind::Fetch, SyncScope::Listed));
    menu_item(msgs, ui, t.tr("toolbar.repo_menu.sync_pull_listed"), sync(SyncKind::Pull, SyncScope::Listed));
}
