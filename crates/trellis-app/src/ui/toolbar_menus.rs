//! The toolbar's menus and its terminal button: the repository picker's menu, the dropdown a
//! caret opens next to a tool button, and Show/Hide terminal with its right-click item.

use crate::msg::{CloneMsg, Msg, RecentMsg, RepoMsg};
use crate::state::AppState;
use crate::terminal::TerminalMsg;
use crate::ui::common::menu_item;
use crate::ui::theme::color;
use crate::ui::widgets::{self, Icon, tooltip::attach};
use eframe::egui;
use trellis_core::i18n::Lang;

/// Shows or hides the terminal panel; right-click offers `item` (Open in terminal: an external
/// one, or a new tab in the panel). A bar under the button marks the panel as open.
pub(super) fn terminal_button(ui: &mut egui::Ui, st: &AppState, labels: bool, item: &str, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let open = st.terminal.open;
    let name = if open { t.tr("toolbar.terminal.name_hide") } else { t.tr("toolbar.terminal.name_show") };
    let resp = widgets::tool_button(ui, Icon::Terminal, t.tr("toolbar.terminal.button"), name, true, labels);
    let resp = attach(resp, &st.keymap, t, || super::tips::terminal(st));
    if open {
        let r = resp.rect;
        let bar = egui::Rect::from_min_max(egui::pos2(r.left() + 6.0, r.bottom() - 2.0), egui::pos2(r.right() - 6.0, r.bottom()));
        ui.painter().rect_filled(bar, 1.0, color::accent());
    }
    if resp.clicked() {
        msgs.push(Msg::Terminal(TerminalMsg::Toggle));
    }
    resp.context_menu(|ui| menu_item(msgs, ui, item, Msg::OpenTerminal));
}

/// The small caret button right against a tool button, opening a menu below it. The painted
/// caret gets `name` as its accessible name (screen readers and UI tests).
pub(super) fn dropdown(ui: &mut egui::Ui, id: &str, name: &str, width: f32, labels: bool, contents: impl FnOnce(&mut egui::Ui)) {
    let caret = widgets::caret_button(ui, labels);
    caret.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    let popup = ui.make_persistent_id(id);
    if caret.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup));
    }
    egui::popup::popup_below_widget(ui, popup, &caret, egui::PopupCloseBehavior::CloseOnClick, |ui| {
        ui.set_min_width(width);
        contents(ui);
    });
}

pub(super) fn repo_menu(ui: &mut egui::Ui, t: Lang, st: &AppState, msgs: &mut Vec<Msg>) {
    ui.set_min_width(260.0);
    menu_item(msgs, ui, t.tr("toolbar.repo_menu.open"), Msg::Repo(RepoMsg::PickRepo));
    menu_item(msgs, ui, t.tr("toolbar.repo_menu.clone"), Msg::Clone(CloneMsg::Dialog));
    menu_item(msgs, ui, t.tr("toolbar.repo_menu.init"), Msg::Repo(RepoMsg::InitRepo));
    ui.separator();
    menu_item(msgs, ui, t.tr("toolbar.repo_menu.dashboard"), Msg::Dashboard(crate::msg::DashboardMsg::Open));
    crate::ui::repo_menu::actions(ui, t, st, msgs);
    if let Some(snap) = &st.snap {
        let fav = crate::state::repo_lists::is_favorite(st, &snap.workdir);
        let label = if fav { t.tr("toolbar.repo_menu.favorite_remove") } else { t.tr("toolbar.repo_menu.favorite_add") };
        menu_item(msgs, ui, label, Msg::Recent(RecentMsg::SetFavorite { path: snap.workdir.clone(), favorite: !fav }));
    }
    crate::ui::repo_list::menu_sections(ui, st, msgs);
}
