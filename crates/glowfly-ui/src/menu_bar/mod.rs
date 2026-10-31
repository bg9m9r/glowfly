//! The menus: File, Edit, View, Window and Help, drawn from the menu model
//! (`state/menu_model.rs`, which the macOS system menu bar shows too) by `draw.rs`. With the
//! system frame they form a menu bar above the tab bar (`show`, drawn by
//! `bar.rs` while `settings.show_menu_bar` is on); a drawn title bar opens them from its menu
//! button instead (`main_menu`): as a list of the five as submenus, or, opened from the
//! keyboard, one menu under a row of the five titles. Each menu has an Alt+letter mnemonic,
//! underlined while Alt is held (`keys.rs`; not on macOS). Inside a menu the keyboard is every
//! menu's (`ui/menu_keys`); Left and Right switch between the bar's menus (`bar.rs`). Every item pushes the message the toolbar, palette or key
//! already sends, with the key it has in the keymap; a disabled item says why on hover.
//! `about.rs` draws Help > About Glowfly (`Modal::About`), and `notices.rs` its Third-party
//! notices (`Modal::Notices`), the bundled licenses.

use super::menu_anchor;
use super::theme::color;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::menu_keys::TopMenu;
use crate::state::menu_model;
use crate::ui::widgets::menu::{separator, submenu};
use eframe::egui::{self, Button, Frame, Id, Key, Margin, Modifiers};

mod about;
mod bar;
mod draw;
mod items;
mod keys;
mod notices;

pub(crate) use crate::ui::menu_keys::may_step;
pub(super) use about::about;
#[cfg(test)]
pub(super) use items::hint;
pub(crate) use keys::{marks, take};
pub(super) use notices::notices;

/// The in-window bar's menus (`menu_anchor` state).
const BAR: &str = "menu-bar";
/// The drawn title bar's menu.
pub(crate) const MAIN_MENU: &str = "main-menu";

/// Draws the bar as the topmost panel (call before the other top panels), with the system
/// frame only.
pub(super) fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    if st.view.chrome.drawn() || !st.settings.show_menu_bar {
        hide(ctx);
        return;
    }
    egui::TopBottomPanel::top("menu-bar").frame(Frame::new().fill(color::toolbar()).inner_margin(Margin::symmetric(6, 2))).show(
        ctx,
        |ui| {
            crate::ui::widgets::menu::bar(ui, |ui| bar::draw(ui, st, msgs));
        },
    );
}

/// Closes the bar's menu for a frame that doesn't draw the bar (Preferences in its place, a
/// hidden bar or a drawn title bar): a menu not drawn can't stay open, or Esc would only close it.
pub(super) fn hide(ctx: &egui::Context) {
    menu_anchor::close(ctx, Id::new(BAR));
}

/// Whether a menu of the bar or of the title bar is open (as of the last frame's drawing).
pub(crate) fn any_open(ctx: &egui::Context) -> bool {
    menu_anchor::open_id(ctx, Id::new(BAR)).is_some() || menu_anchor::open_id(ctx, Id::new(MAIN_MENU)).is_some()
}

/// Puts the focus back in the open menu of the bar or title bar when it moved out to a widget
/// behind it. Call before the bar is drawn.
pub(crate) fn keep_inside(ctx: &egui::Context) {
    if any_open(ctx) {
        crate::ui::menu_keys::keep_inside(ctx);
    }
}

/// The first item of the menu `ui` draws takes the focus (once it can) when the menu was opened
/// from the keyboard or switched to.
fn focus_first(ui: &egui::Ui, st: &mut AppState) {
    if std::mem::take(&mut st.view.menu_keys.focus_first) {
        crate::ui::menu_keys::focus_first(ui);
    }
}

/// The title bar's menu: the menu its title row shows, or the five-row list.
pub(crate) fn main_menu(ui: &mut egui::Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    focus_first(ui, st);
    match st.view.menu_keys.only {
        Some(m) => single(ui, st, msgs, m),
        None => menus(ui, st, msgs),
    }
}

/// `m`'s items.
fn contents(m: TopMenu, ui: &mut egui::Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    focus_first(ui, st);
    ui.set_min_width(draw::min_width(m));
    let entries = menu_model::entries(st, m);
    draw::entries(ui, st, entries, msgs);
}

/// The five menus as submenus, opening on hover. From the keyboard, Enter, Space or Right on
/// a row shows that menu alone (`single`, so its rows don't open their submenus from the keyboard
/// themselves), and so does a menu's letter while a row is focused.
fn menus(ui: &mut egui::Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let marks = keys::marks(st);
    let show = keys::shown(ui.ctx(), st);
    crate::ui::menu_keys::caller_opens(ui);
    let mut picked = None;
    let mut list_focused = false;
    for m in TopMenu::ALL {
        let r = submenu(ui, keys::title(ui, t, m, &marks, show), |ui| contents(m, ui, st, msgs)).response;
        if st.view.menu_keys.back_to == Some(m) {
            r.request_focus();
            st.view.menu_keys.back_to = None;
        }
        if r.has_focus() {
            list_focused = true;
            let go = ui.input_mut(|i| [Key::Enter, Key::Space, Key::ArrowRight].into_iter().any(|k| i.consume_key(Modifiers::NONE, k)));
            picked = picked.or(go.then_some(m));
        }
    }
    if list_focused && picked.is_none() {
        picked = TopMenu::ALL.into_iter().zip(marks).find_map(|(m, mark)| {
            let letter = mark?.letter;
            let key = Key::from_name(&letter.to_ascii_uppercase().to_string())?;
            // The letter's text too, so the menu's typeahead doesn't move to a row as well.
            let taken = ui.input_mut(|i| i.consume_key(Modifiers::NONE, key));
            if taken {
                ui.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Text(s) if s.eq_ignore_ascii_case(&letter.to_string()))));
            }
            taken.then_some(m)
        });
    }
    if let Some(m) = picked {
        st.view.menu_keys.opened_by_key(m);
        ui.ctx().request_repaint();
    }
}

/// Menu `m` alone, under a row of the five titles (`m`'s selected; a click shows another).
fn single(ui: &mut egui::Ui, st: &mut AppState, msgs: &mut Vec<Msg>, m: TopMenu) {
    let t = st.settings.lang();
    let marks = keys::marks(st);
    let show = keys::shown(ui.ctx(), st);
    ui.horizontal(|ui| {
        for x in TopMenu::ALL {
            if ui.add(Button::new(keys::title(ui, t, x, &marks, show)).selected(x == m)).clicked() {
                st.view.menu_keys.only = Some(x);
                st.view.menu_keys.focus_first = true;
            }
        }
    });
    separator(ui);
    contents(m, ui, st, msgs);
}

#[cfg(test)]
pub(crate) mod keys_tests;
#[cfg(test)]
mod model_tests;
#[cfg(test)]
mod quit_tests;
#[cfg(test)]
pub(crate) mod snapshot_tests;
