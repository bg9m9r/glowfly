//! The toolbar repository menu's list actions, after Dashboard: finding repositories in
//! project folders, opening every favorite in a tab, and removing the entries that are gone.

use super::common::menu_item;
use crate::msg::{Msg, RecentMsg, ReposMsg};
use crate::state::AppState;
use crate::state::repo_lists::{favorites, missing_counts, remove_missing_msg, unopened_favorites};
use eframe::egui;
use trellis_core::i18n::Lang;

pub fn actions(ui: &mut egui::Ui, t: Lang, st: &AppState, msgs: &mut Vec<Msg>) {
    menu_item(msgs, ui, t.tr("toolbar.repo_menu.find"), Msg::Repos(ReposMsg::ShowFinder));
    if !unopened_favorites(st).is_empty() {
        menu_item(msgs, ui, t.tr("toolbar.repo_menu.open_favorites"), Msg::Recent(RecentMsg::OpenAll(favorites(&st.settings))));
    }
    let (recent, favs) = missing_counts(st);
    if let Some(ask) = remove_missing_msg(st) {
        menu_item(msgs, ui, t.trn("toolbar.repo_menu.remove_missing", recent + favs, &[]), ask);
    }
}
