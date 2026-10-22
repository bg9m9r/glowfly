//! Window: the open tabs (with their Go to tab keys), stepping between them and closing others.

use super::items::item;
use crate::keymap::BindingId as I;
use crate::msg::{Msg, TabsMsg};
use crate::state::AppState;
use crate::state::menu_bar::{tab_hint, tab_name};
use eframe::egui::{self, Button};

pub(super) fn items(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    ui.set_min_width(240.0);
    let keys = &st.keymap;
    let n = st.tabs.len();
    for i in 0..n {
        let hint = tab_hint(keys, i, n).unwrap_or_default();
        let tab = Button::new(tab_name(st, i)).selected(i == st.active_tab).shortcut_text(hint);
        if ui.add(tab).clicked() {
            msgs.push(Msg::Tabs(TabsMsg::SelectTab(i)));
            ui.close_menu();
        }
    }
    ui.separator();
    let one = (n < 2).then_some(t.tr("menu.window.one_tab"));
    item(ui, keys, t.tr("menu.window.next_tab"), Some(I::NextTab), one, Msg::Tabs(TabsMsg::CycleTab(1)), msgs);
    item(ui, keys, t.tr("menu.window.prev_tab"), Some(I::PrevTab), one, Msg::Tabs(TabsMsg::CycleTab(-1)), msgs);
    let active = st.active_tab;
    let no_others = one.map(|_| t.tr("menu.window.no_others"));
    item(ui, keys, t.tr("menu.window.close_others"), None, no_others, Msg::Tabs(TabsMsg::CloseOtherTabs(active)), msgs);
    let none_right = (active + 1 >= n).then_some(t.tr("menu.window.none_right"));
    let close_right = Msg::Tabs(TabsMsg::CloseTabsToRight(active));
    item(ui, keys, t.tr("menu.window.close_right"), None, none_right, close_right, msgs);
}
