//! The title bar Trellis draws on Windows and macOS: one bar on the chrome colour with the menu
//! button (File, Edit, View, Window and Help as submenus), the repository tabs along its bottom
//! edge and empty space to grab, before the window buttons (`window_frame.rs` draws those above
//! every dialog) or after the macOS traffic lights. Dragging the empty space moves the window,
//! double-clicking it maximizes or restores, and on Windows a tap of Alt opens the menu.

use super::theme::color;
use super::widgets::{Icon, paint_icon};
use super::window_buttons::BUTTON_W;
use super::{menu_anchor, menu_bar, tabs};
use crate::msg::{AppMsg, Msg};
use crate::state::AppState;
use crate::state::window_chrome::{AltFrame, WindowCmd};
use eframe::egui::{
    self, Align, CornerRadius, Event, Id, LayerId, Layout, PointerButton, Pos2, Rect, Sense, UiBuilder, WidgetInfo, WidgetType, pos2, vec2,
};

/// The bar's height, and its tabs' (along its bottom edge).
pub(super) const TITLE_H: f32 = 44.0;
pub(super) const TAB_H: f32 = 34.0;
/// The menu button.
const MENU_W: f32 = 32.0;
const MENU_H: f32 = 30.0;
/// Empty bar always kept before the window buttons, to grab the window by.
pub(super) const MIN_DRAG: f32 = 56.0;
/// Where the bar keeps its rect and layer for the overlay (`window_frame::overlay`).
const PLACE: &str = "title-bar-place";

/// Draws the bar as the topmost panel. While Preferences is open it holds no menu or tabs, so
/// the window can still be moved, resized and closed.
pub(super) fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    egui::TopBottomPanel::top("title-bar")
        .exact_height(TITLE_H)
        .show_separator_line(false)
        .frame(egui::Frame::new().fill(color::toolbar()))
        .show(ctx, |ui| {
            let bar = ui.max_rect();
            ctx.data_mut(|d| d.insert_temp(Id::new(PLACE), (bar, ui.layer_id())));
            // First, so every widget drawn after it wins the pointer over it.
            let bg = ui.interact(bar, ui.id().with("title-bar-drag"), Sense::CLICK | Sense::DRAG);
            if bg.double_clicked() {
                msgs.push(Msg::App(AppMsg::Window(WindowCmd::SetMaximized(!st.view.window.maximized))));
            } else if bg.drag_started_by(PointerButton::Primary) {
                msgs.push(Msg::App(AppMsg::Window(WindowCmd::StartDrag)));
            }
            if st.prefs.is_some() {
                return;
            }
            let chrome = st.view.chrome;
            let inset = chrome.leading_inset(st.view.window.fullscreen);
            let x = bar.left() + inset + 8.0;
            menu_button(ui, st, pos2(x, bar.center().y), msgs);
            let buttons_w = if chrome.window_buttons() { 3.0 * BUTTON_W } else { 0.0 };
            let left = x + MENU_W + 8.0;
            let strip =
                Rect::from_min_size(pos2(left, bar.bottom() - TAB_H), vec2(tab_room(bar.width(), inset, buttons_w).max(0.0), TAB_H));
            let mut child = ui.new_child(UiBuilder::new().max_rect(strip).layout(Layout::left_to_right(Align::Min)));
            // Many tabs never cover the drag space or the window buttons.
            child.set_clip_rect(strip.intersect(ui.clip_rect()));
            tabs::strip(&mut child, st, TAB_H, msgs);
        });
}

/// The bar's rect and layer this frame, once it was drawn.
pub(super) fn place(ctx: &egui::Context) -> Option<(Rect, LayerId)> {
    ctx.data(|d| d.get_temp::<(Rect, LayerId)>(Id::new(PLACE)))
}

/// The tabs' room in a bar `bar_w` wide that leaves `inset` at its start and `buttons_w` for
/// the window buttons at its end.
fn tab_room(bar_w: f32, inset: f32, buttons_w: f32) -> f32 {
    bar_w - inset - 8.0 - MENU_W - 8.0 - buttons_w - MIN_DRAG
}

/// The menu button with its left edge at `at.x`, centred on `at.y`; a click or an Alt tap opens
/// the menu below it.
fn menu_button(ui: &mut egui::Ui, st: &mut AppState, at: Pos2, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let rect = Rect::from_min_size(pos2(at.x, at.y - MENU_H / 2.0), vec2(MENU_W, MENU_H));
    let r = ui.interact(rect, Id::new("title-bar-menu"), Sense::click());
    let (tap, fired) = st.view.alt_tap.step(alt_frame(ui.ctx()));
    st.view.alt_tap = tap;
    // Not while a dialog blocks the bar or the terminal has the keyboard.
    let toggle =
        fired && st.view.chrome.alt_opens_menu() && !st.terminal.focused && ui.ctx().memory(|m| m.allows_interaction(ui.layer_id()));
    let open = menu_anchor::menu_below(ui, Id::new("main-menu"), &r, toggle, |ui| menu_bar::menus(ui, st, msgs));
    let p = ui.painter();
    if open {
        p.rect_filled(rect, CornerRadius::same(6), color::selected());
    } else if r.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), color::hover());
    }
    paint_icon(p, rect.center(), Icon::Menu, color::text());
    let name = t.tr("title_bar.menu.tip");
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, name));
    r.on_hover_text(name);
}

/// This frame's input as the Alt tap reads it.
fn alt_frame(ctx: &egui::Context) -> AltFrame {
    ctx.input(|i| AltFrame {
        alt: i.modifiers.alt,
        other_modifiers: i.modifiers.ctrl || i.modifiers.shift || i.modifiers.command || i.modifiers.mac_cmd,
        other_input: i.events.iter().any(|e| {
            matches!(
                e,
                Event::Key { pressed: true, .. } | Event::Text(_) | Event::PointerButton { pressed: true, .. } | Event::MouseWheel { .. }
            )
        }),
        focused: i.focused,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::window_chrome::Chrome;

    #[test]
    fn the_tabs_get_what_the_bar_leaves() {
        for chrome in [Chrome::System, Chrome::Windows, Chrome::MacOs] {
            let inset = chrome.leading_inset(false);
            let buttons = if chrome.window_buttons() { 3.0 * BUTTON_W } else { 0.0 };
            let room = tab_room(1440.0, inset, buttons);
            assert_eq!(room, 1440.0 - inset - 8.0 - MENU_W - 8.0 - buttons - MIN_DRAG, "{chrome:?}");
        }
        assert_eq!(tab_room(1440.0, 0.0, 138.0), 1440.0 - 48.0 - 138.0 - 56.0);
        assert_eq!(tab_room(1440.0, 78.0, 0.0), 1440.0 - 78.0 - 48.0 - 56.0);
    }
}
