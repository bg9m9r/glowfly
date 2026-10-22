//! Draws a menu's entries from the model (`state/menu_model.rs`): plain items with their key,
//! check items with the box in the gutter, the diff toggles and radios, the Window menu's tab
//! rows, headings, separators and the Open recent and Theme submenus. A click runs the item's
//! action and closes the menu.

use super::items::{check_item, clicked, hint, submenu_if};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::menu_keys::TopMenu;
use crate::state::menu_model::{Entry, Item, Mark, Submenu, SubmenuKind};
use crate::ui::repo_list;
use crate::ui::theme::{color, controls, type_scale};
use eframe::egui::{self, Button, RichText};

/// The width a menu opens at, at least.
pub(super) fn min_width(m: TopMenu) -> f32 {
    match m {
        TopMenu::File => 230.0,
        TopMenu::Edit => 250.0,
        TopMenu::View => 260.0,
        TopMenu::Window | TopMenu::Help => 240.0,
    }
}

/// Draws `entries`.
pub(super) fn entries(ui: &mut egui::Ui, st: &mut AppState, entries: Vec<Entry>, msgs: &mut Vec<Msg>) {
    rows(ui, st, entries, false, msgs);
}

/// Draws `entries`; with `buttons`, plain items are plain buttons (the Theme submenu's last row).
fn rows(ui: &mut egui::Ui, st: &mut AppState, entries: Vec<Entry>, buttons: bool, msgs: &mut Vec<Msg>) {
    let mut after_separator = true;
    for e in entries {
        let separator = matches!(e, Entry::Separator);
        match e {
            Entry::Item(it) => item(ui, st, it, buttons, msgs),
            Entry::Separator => {
                ui.separator();
            }
            Entry::Heading(text) => {
                // A gap before a group that doesn't follow a separator.
                if !after_separator {
                    ui.add_space(4.0);
                }
                ui.label(RichText::new(text).size(type_scale::SMALL).color(color::weak()));
            }
            Entry::Submenu(Submenu { kind, label, blocked, entries }) => {
                submenu_if(ui, &label, blocked.as_deref(), |ui| match kind {
                    // The rows keep their stars, swatches, × and "not found" tips.
                    SubmenuKind::Recent => {
                        ui.set_min_width(260.0);
                        repo_list::recent_submenu(ui, st, msgs);
                    }
                    SubmenuKind::Theme => {
                        ui.set_min_width(200.0);
                        rows(ui, st, entries, true, msgs);
                    }
                });
            }
            Entry::Taken => {}
        }
        after_separator = separator;
    }
}

fn item(ui: &mut egui::Ui, st: &mut AppState, it: Item, buttons: bool, msgs: &mut Vec<Msg>) {
    let label = it.label.as_str();
    let blocked = it.blocked.as_deref();
    let hit = match it.mark {
        Mark::None if buttons => ui.button(label).clicked(),
        Mark::None => clicked(ui, label, hint(it.chord), blocked),
        Mark::Check(on) => check_item(ui, label, on, hint(it.chord), blocked),
        Mark::Toggle(on) => controls::checkbox(ui, &mut { on }, label).clicked(),
        Mark::Radio(on) => controls::radio(ui, on, label).clicked(),
        Mark::Selected(on) => ui.add(Button::new(label).selected(on).shortcut_text(hint(it.chord))).clicked(),
    };
    if hit {
        it.action.run(st, msgs);
        ui.close_menu();
    }
}
