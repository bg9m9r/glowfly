//! File: tabs, opening, cloning and starting repositories, recent ones, Preferences and Quit.

use super::items::{item, submenu_if};
use crate::keymap::BindingId as I;
use crate::msg::{AppMsg, CloneMsg, Msg, PrefsMsg, RepoMsg, TabsMsg};
use crate::state::{AppState, PrefPage};
use crate::ui::repo_list;
use eframe::egui;

pub(super) fn menu(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    ui.menu_button("File", |ui| {
        ui.set_min_width(230.0);
        let keys = &st.keymap;
        item(ui, keys, "New tab", Some(I::NewTab), None, Msg::Tabs(TabsMsg::NewTab), msgs);
        item(ui, keys, "Open repository…", None, None, Msg::Repo(RepoMsg::PickRepo), msgs);
        item(ui, keys, "Clone…", None, None, Msg::Clone(CloneMsg::Dialog), msgs);
        item(ui, keys, "Init…", None, None, Msg::Repo(RepoMsg::InitRepo), msgs);
        let no_recent = (!repo_list::has_entries(st)).then_some("No recent repositories yet");
        submenu_if(ui, "Open recent", no_recent, |ui| {
            ui.set_min_width(260.0);
            repo_list::recent_submenu(ui, st, msgs);
        });
        ui.separator();
        let no_closed = st.closed_tabs.is_empty().then_some("No closed tabs to reopen");
        item(ui, keys, "Reopen closed tab", Some(I::ReopenTab), no_closed, Msg::Tabs(TabsMsg::ReopenClosedTab), msgs);
        item(ui, keys, "Close tab", Some(I::CloseTab), None, Msg::Tabs(TabsMsg::CloseTab(usize::MAX)), msgs);
        ui.separator();
        item(ui, keys, "Preferences…", Some(I::Prefs), None, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)), msgs);
        ui.separator();
        item(ui, keys, "Quit", None, None, Msg::App(AppMsg::Quit), msgs);
    });
}
