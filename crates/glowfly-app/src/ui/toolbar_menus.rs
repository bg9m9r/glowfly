//! The toolbar's menus and its terminal button: the repository picker's menu, the split tool
//! buttons whose caret opens a menu, and Show/Hide terminal with its right-click item.

use crate::msg::{CloneMsg, Msg, RecentMsg, RepoMsg};
use crate::state::AppState;
use crate::terminal::TerminalMsg;
use crate::ui::common::menu_item;
use crate::ui::widgets::menu::{context_menu, popup_below, separator};
use crate::ui::widgets::{self, Icon, tooltip::attach};
use eframe::egui;
use glowfly_core::i18n::Lang;

/// Shows or hides the terminal panel, filled while it is open; right-click offers `item` (Open
/// in terminal: an external one, or a new tab in the panel).
pub(super) fn terminal_button(ui: &mut egui::Ui, st: &AppState, labels: bool, item: &str, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let open = st.terminal.open;
    let name = if open { t.tr("toolbar.terminal.name_hide") } else { t.tr("toolbar.terminal.name_show") };
    let resp = widgets::tool_toggle(ui, Icon::Terminal, t.tr("toolbar.terminal.button"), name, open, labels);
    let resp = attach(resp, &st.keymap, t, || super::tips::terminal(st));
    if resp.clicked() {
        msgs.push(Msg::Terminal(TerminalMsg::Toggle));
    }
    context_menu(&resp, |ui| menu_item(msgs, ui, item, Msg::OpenTerminal));
}

/// A tool button with its dropdown caret (`widgets::split_button`): `tool` is its icon, caption
/// and accessible name; the caret is named `menu.0` and opens a menu `menu.1` wide below it,
/// whose items `contents` adds. `menu_live` says whether anything in the menu can be used.
/// Returns the button's response.
pub(super) fn split(
    ui: &mut egui::Ui,
    id: &str,
    tool: (Icon, &str, &str),
    enabled: bool,
    labels: bool,
    (menu, width, menu_live): (&str, f32, bool),
    contents: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let popup = ui.make_persistent_id(id);
    let open = ui.memory(|m| m.is_popup_open(popup));
    let (button, caret) = widgets::split_button(ui, tool, enabled, labels, menu_live, open);
    caret.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, menu));
    if caret.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup));
    }
    popup_below(ui, popup, &caret, egui::PopupCloseBehavior::CloseOnClick, |ui| {
        ui.set_min_width(width);
        contents(ui);
    });
    button
}

pub(super) fn repo_menu(ui: &mut egui::Ui, t: Lang, st: &AppState, msgs: &mut Vec<Msg>) {
    ui.set_min_width(260.0);
    menu_item(msgs, ui, t.tr("toolbar.repo_menu.open"), Msg::Repo(RepoMsg::PickRepo));
    menu_item(msgs, ui, t.tr("toolbar.repo_menu.clone"), Msg::Clone(CloneMsg::Dialog));
    menu_item(msgs, ui, t.tr("toolbar.repo_menu.init"), Msg::Repo(RepoMsg::InitRepo));
    separator(ui);
    menu_item(msgs, ui, t.tr("toolbar.repo_menu.dashboard"), Msg::Dashboard(crate::msg::DashboardMsg::Open));
    crate::ui::repo_menu::actions(ui, t, st, msgs);
    if let Some(snap) = &st.snap {
        let fav = crate::state::repo_lists::is_favorite(st, &snap.workdir);
        let label = if fav { t.tr("toolbar.repo_menu.favorite_remove") } else { t.tr("toolbar.repo_menu.favorite_add") };
        menu_item(msgs, ui, label, Msg::Recent(RecentMsg::SetFavorite { path: snap.workdir.clone(), favorite: !fav }));
    }
    crate::ui::repo_list::menu_sections(ui, st, msgs);
}
