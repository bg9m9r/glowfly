//! A branch menu's Color submenu, shared by the local and remote branch menus: Automatic, then
//! one row per hue of the active preset with its dot and name, the current choice checked. The
//! key is the branch's colour key, so a pin on a local branch colours its remote copies too. The
//! main branch always takes the accent: its item is disabled and says so on hover.

use super::branch_dot;
use super::common::menu_item_if;
use super::theme::branch_colours::line;
use super::theme::{branch_names, color};
use crate::msg::{GraphMsg, Msg};
use crate::state::branches::MAIN_USES_ACCENT;
use eframe::egui;
use glowfly_core::branch_colour::Swatch;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;

fn pin(key: &str, slot: Option<u8>) -> Msg {
    Msg::Graph(GraphMsg::PinColour { key: key.to_owned(), slot })
}

/// A row's label, with the check glyph when it is the current choice.
fn checked(label: &str, on: bool) -> String {
    if on { format!("✔ {label}") } else { label.to_owned() }
}

/// The Color item for the branch key `key`.
pub fn items(ui: &mut egui::Ui, t: Lang, key: &str, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    if snap.graph.owners.mainline.as_deref() == Some(key) {
        menu_item_if(msgs, ui, t.tr("branch.menu.color"), Some(MAIN_USES_ACCENT), pin(key, None));
        return;
    }
    let branch = color::current().branch;
    let n = branch.preset.slots();
    let current = snap.graph.colouring.pins.get(key).map(|p| p % n);
    ui.menu_button(t.tr("branch.menu.color"), |ui| {
        let auto = branch_dot::menu_row(ui, &checked(t.tr("branch.color.automatic"), current.is_none()), None, true)
            .on_hover_text(t.tr("branch.color.automatic.tip"));
        if auto.clicked() {
            msgs.push(pin(key, None));
            ui.close_menu();
        }
        ui.separator();
        for i in 0..n {
            let label = checked(branch_names::hue(t, &branch, i), current == Some(i));
            if branch_dot::menu_row(ui, &label, Some(line(Swatch::Slot(i))), true).clicked() {
                msgs.push(pin(key, Some(i)));
                ui.close_menu();
            }
        }
    });
}
