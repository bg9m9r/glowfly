//! The menu bar: one row of File, Edit, View, Window and Help menus above the tab strip, drawn
//! on every OS while `settings.show_menu_bar` is on. Every item pushes the message the toolbar,
//! palette or key already sends, with the key it has in the keymap; a disabled item says why on
//! hover. `about.rs` draws Help > About Trellis (`Modal::About`).

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

/// Draws the bar as the topmost panel (call before the other top panels).
pub(super) fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    if !st.settings.show_menu_bar {
        return;
    }
    egui::TopBottomPanel::top("menu-bar").frame(Frame::new().fill(color::bg()).inner_margin(Margin::symmetric(6, 2))).show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            file::menu(ui, st, msgs);
            edit::menu(ui, st, msgs);
            view::menu(ui, st, msgs);
            window::menu(ui, st, msgs);
            help::menu(ui, st, msgs);
        });
    });
}

#[cfg(test)]
mod snapshot_tests;
