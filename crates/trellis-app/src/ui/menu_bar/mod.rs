//! The menus: File, Edit, View, Window and Help (`menus`). With the system frame they form a
//! menu bar above the tab bar (`show`, while `settings.show_menu_bar` is on); a drawn title bar
//! opens them as submenus of its menu button instead. Every item pushes the message the
//! toolbar, palette or key already sends, with the key it has in the keymap; a disabled item
//! says why on hover. `about.rs` draws Help > About Trellis (`Modal::About`).

use super::theme::color;
use crate::msg::Msg;
use crate::state::AppState;
use eframe::egui::{self, Frame, Margin};

mod about;
mod edit;
mod file;
mod help;
mod items;
mod view;
mod window;

pub(super) use about::about;
#[cfg(test)]
pub(super) use items::hint;

/// Draws the bar as the topmost panel (call before the other top panels), with the system
/// frame only.
pub(super) fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    if st.view.chrome.drawn() || !st.settings.show_menu_bar {
        return;
    }
    egui::TopBottomPanel::top("menu-bar").frame(Frame::new().fill(color::toolbar()).inner_margin(Margin::symmetric(6, 2))).show(
        ctx,
        |ui| {
            egui::menu::bar(ui, |ui| menus(ui, st, msgs));
        },
    );
}

/// The five menus in order, as menu bar buttons or (inside a menu) as submenus.
pub(super) fn menus(ui: &mut egui::Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    file::menu(ui, st, msgs);
    edit::menu(ui, st, msgs);
    view::menu(ui, st, msgs);
    window::menu(ui, st, msgs);
    help::menu(ui, st, msgs);
}

#[cfg(test)]
mod snapshot_tests;
