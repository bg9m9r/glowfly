//! File: tabs, opening, cloning and starting repositories, recent ones, Preferences and Quit.

use super::items::{item, submenu_if};
use crate::keymap::BindingId as I;
use crate::msg::{AppMsg, CloneMsg, Msg, PrefsMsg, RepoMsg, TabsMsg};
use crate::state::{AppState, PrefPage};
use crate::ui::repo_list;
use eframe::egui;

pub(super) fn items(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    ui.set_min_width(230.0);
    let keys = &st.keymap;
    item(ui, keys, t.tr("menu.file.new_tab"), Some(I::NewTab), None, Msg::Tabs(TabsMsg::NewTab), msgs);
    item(ui, keys, t.tr("menu.file.open"), None, None, Msg::Repo(RepoMsg::PickRepo), msgs);
    item(ui, keys, t.tr("menu.file.clone"), None, None, Msg::Clone(CloneMsg::Dialog), msgs);
    item(ui, keys, t.tr("menu.file.init"), None, None, Msg::Repo(RepoMsg::InitRepo), msgs);
    let no_recent = (!repo_list::has_entries(st)).then_some(t.tr("menu.file.no_recent"));
    submenu_if(ui, t.tr("menu.file.recent"), no_recent, |ui| {
        ui.set_min_width(260.0);
        repo_list::recent_submenu(ui, st, msgs);
    });
    ui.separator();
    let no_closed = st.closed_tabs.is_empty().then_some(t.tr("menu.file.no_closed"));
    item(ui, keys, t.tr("menu.file.reopen_tab"), Some(I::ReopenTab), no_closed, Msg::Tabs(TabsMsg::ReopenClosedTab), msgs);
    item(ui, keys, t.tr("menu.file.close_tab"), Some(I::CloseTab), None, Msg::Tabs(TabsMsg::CloseTab(usize::MAX)), msgs);
    ui.separator();
    let prefs = Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General));
    item(ui, keys, t.tr("menu.file.prefs"), Some(I::Prefs), None, prefs, msgs);
    ui.separator();
    item(ui, keys, t.tr("menu.file.quit"), Some(I::Quit), None, Msg::App(AppMsg::Quit), msgs);
}
