//! The tab list at the far end of the tab bar: a chevron button whose popup lists the open tabs
//! (click one to select it) and the recently closed ones, newest first (click one to reopen it).
//! It shows only when it adds something: closed tabs to reopen, or tabs too many to show at full
//! width.

use super::tabs::HEIGHT;
use super::theme::color;
use super::widgets;
use crate::msg::{Msg, TabsMsg};
use crate::state::AppState;
use crate::state::menu_bar::tab_name;
use eframe::egui::{self, Align, Button, Layout, PopupCloseBehavior, RichText, Sense, vec2};
use trellis_core::i18n::Lang;
use trellis_core::native_path::shown;

/// A full-width tab and the gap after it.
const FULL_PITCH: f32 = 221.0;

/// Whether the tab bar, `avail` wide, shows the tab list button.
pub(super) fn wanted(st: &AppState, avail: f32) -> bool {
    !st.closed_tabs.is_empty() || st.tabs.len() as f32 * FULL_PITCH > avail - 2.0 * HEIGHT
}

pub(super) fn button(ui: &mut egui::Ui, t: Lang, st: &AppState, msgs: &mut Vec<Msg>) {
    let tip = t.tr("tabs.list.tip");
    let (rect, r) = ui.allocate_exact_size(vec2(HEIGHT, HEIGHT), Sense::click());
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
    let popup = ui.make_persistent_id("tab-list");
    let open = ui.memory(|m| m.is_popup_open(popup));
    let lit = r.hovered() || open;
    if lit {
        ui.painter().rect_filled(rect.shrink(3.0), 4.0, color::hover());
    }
    widgets::chevron(ui.painter(), rect.center(), true, if lit { color::text() } else { color::weak() });
    let r = r.on_hover_text(tip);
    if r.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup));
    }
    egui::popup::popup_below_widget(ui, popup, &r, PopupCloseBehavior::CloseOnClick, |ui| {
        ui.set_min_width(260.0);
        // Rows like a menu's: frameless, full width, text on the left.
        ui.style_mut().visuals.button_frame = false;
        ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| list(ui, t, st, msgs));
    });
}

fn list(ui: &mut egui::Ui, t: Lang, st: &AppState, msgs: &mut Vec<Msg>) {
    heading(ui, t.tr("tabs.list.open"));
    for (i, tab) in st.tabs.iter().enumerate() {
        let r = ui.add(Button::new(tab_name(st, i)).selected(i == st.active_tab));
        let r = match &tab.path {
            Some(path) => r.on_hover_text(shown(path)),
            None => r,
        };
        if r.clicked() {
            msgs.push(Msg::Tabs(TabsMsg::SelectTab(i)));
        }
    }
    ui.separator();
    heading(ui, t.tr("tabs.list.closed"));
    if st.closed_tabs.is_empty() {
        ui.label(RichText::new(t.tr("menu.file.no_closed")).color(color::weak()));
    }
    for closed in st.closed_tabs.iter().rev() {
        if ui.add(Button::new(closed.name())).on_hover_text(shown(&closed.path)).clicked() {
            msgs.push(Msg::Tabs(TabsMsg::ReopenClosed(closed.path.clone())));
        }
    }
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).small().color(color::weak()));
}
