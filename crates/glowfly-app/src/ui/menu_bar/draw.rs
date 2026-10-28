//! Draws a menu's entries from the model (`state/menu_model.rs`) on the menu's shared columns:
//! plain items with their key, check items (✓), radios (a dot) and the Window menu's current tab
//! (✓) in the state column, group headers in place of their separator, separators, and the Open
//! recent and Theme submenus. A click runs the item's action and closes the menu.

use super::items::{hint, submenu_if};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::menu_keys::TopMenu;
use crate::state::menu_model::{Entry, Item, Mark, Submenu, SubmenuKind};
use crate::ui::repo_menu_list;
use crate::ui::widgets::menu;
use eframe::egui;

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
    let mut entries = entries.into_iter().peekable();
    while let Some(e) = entries.next() {
        match e {
            Entry::Item(it) => item(ui, st, it, msgs),
            // A group's header takes the place of its separator.
            Entry::Separator if matches!(entries.peek(), Some(Entry::Heading(_))) => {}
            Entry::Separator => menu::separator(ui),
            Entry::Heading(text) => {
                menu::group_header(ui, &text);
            }
            Entry::Submenu(Submenu { kind, label, blocked, entries }) => {
                submenu_if(ui, &label, blocked.as_deref(), |ui| match kind {
                    // The repository menu's rows, with their stars, swatches, × and statuses.
                    SubmenuKind::Recent => repo_menu_list::recent_submenu(ui, st, msgs),
                    SubmenuKind::Theme => {
                        ui.set_min_width(200.0);
                        self::entries(ui, st, entries, msgs);
                    }
                });
            }
            Entry::Taken => {}
        }
    }
}

fn item(ui: &mut egui::Ui, st: &mut AppState, it: Item, msgs: &mut Vec<Msg>) {
    let row = menu::item(it.label.as_str()).shortcut(hint(it.chord)).blocked(it.blocked.as_deref());
    let row = match it.mark {
        Mark::None => row,
        Mark::Check(on) => row.check(on),
        Mark::Radio(on) => row.option(on),
        Mark::Selected(on) => row.current(on),
    };
    if row.show(ui).clicked() {
        it.action.run(st, msgs);
        ui.close_menu();
    }
}
