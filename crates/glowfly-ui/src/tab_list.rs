//! The tab list right after the tab bar's + button: a chevron button whose popup lists the open tabs
//! (click one to select it) and the recently closed ones, newest first (click one to reopen it).
//! It shows only when it adds something: closed tabs to reopen, or tabs too many to show at
//! their own widths.

use super::theme::color;
use super::widgets;
use crate::theme::sizes::ControlSize;
use crate::widgets::menu::{self, popup_below};
use crate::widgets::quiet_target::{self, State};
use eframe::egui::{self, Align, CornerRadius, Layout, PopupCloseBehavior, Rect, RichText, Sense, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::native_path::shown;
use glowfly_model::msg::{Msg, TabsMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::menu_bar::tab_name;

/// The tab list button's size: a Regular icon-only square.
const SIZE: ControlSize = ControlSize::Regular;

/// Whether the tab list button shows: when there are closed tabs, or the tabs at their own
/// widths (`needed`, with the gaps between them) are wider than the `room` the strip has for them.
pub(super) fn wanted(st: &AppState, needed: f32, room: f32) -> bool {
    !st.closed_tabs.is_empty() || needed > room
}

/// The tab list button in a `slot` square, and its popup. The button is a Regular quiet
/// icon-only target centred in the slot: its hit area is the square it lights (hovered, held,
/// or latched while the popup is open), and it rings that square while focused.
pub(super) fn button(ui: &mut egui::Ui, t: Lang, st: &AppState, slot: f32, msgs: &mut Vec<Msg>) {
    let tip = t.tr("tabs.list.tip");
    let (slot_rect, _) = ui.allocate_exact_size(vec2(slot, slot), Sense::hover());
    let m = SIZE.metrics();
    let rect = Rect::from_center_size(slot_rect.center(), vec2(m.h, m.h));
    let r = ui.interact(rect, ui.id().with("tab-list-button"), Sense::click());
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
    let popup = ui.make_persistent_id("tab-list");
    let open = ui.memory(|m| m.is_popup_open(popup));
    let state = State::of(&r, open);
    quiet_target::paint(ui.painter(), rect, CornerRadius::same(m.radius), SIZE, state);
    widgets::caret(ui.painter(), rect.center(), if state.lit() { color::strong() } else { color::weak() });
    let r = r.on_hover_text(tip);
    if r.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup));
    }
    popup_below(ui, popup, &r, PopupCloseBehavior::CloseOnClick, |ui| {
        ui.set_min_width(260.0);
        ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| list(ui, t, st, msgs));
    });
}

fn list(ui: &mut egui::Ui, t: Lang, st: &AppState, msgs: &mut Vec<Msg>) {
    menu::group_header(ui, t.tr("tabs.list.open"));
    for (i, tab) in st.tabs.iter().enumerate() {
        let r = menu::item(tab_name(st, i)).current(i == st.active_tab).show(ui);
        let r = match &tab.path {
            Some(path) => r.on_hover_text(shown(path)),
            None => r,
        };
        if r.clicked() {
            msgs.push(Msg::Tabs(TabsMsg::SelectTab(i)));
        }
    }
    menu::group_header(ui, t.tr("tabs.list.closed"));
    if st.closed_tabs.is_empty() {
        menu::inset(ui, |ui| ui.label(RichText::new(t.tr("menu.file.no_closed")).color(color::weak())));
    }
    for closed in st.closed_tabs.iter().rev() {
        if menu::item(closed.name()).show(ui).on_hover_text(shown(&closed.path)).clicked() {
            msgs.push(Msg::Tabs(TabsMsg::ReopenClosed(closed.path.clone())));
        }
    }
}
