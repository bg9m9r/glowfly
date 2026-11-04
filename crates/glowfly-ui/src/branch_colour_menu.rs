//! A branch menu's Color submenu, shared by the local and remote branch menus: Automatic, then
//! one row per hue of the active preset with its dot and name, the current choice checked in the
//! state column (the dots in the icon column, so every label lines up). The
//! key is the branch's colour key, so a pin on a local branch colours its remote copies too. The
//! main branch has the same submenu: its Automatic is the gray trunk tone, with its own tooltip.

use super::branch_dot;
use super::theme::branch_colours::line;
use super::theme::{branch_names, color};
use crate::widgets::menu::{self, separator, submenu};
use eframe::egui;
use glowfly_core::branch_colour::Swatch;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;
use glowfly_model::msg::{GraphMsg, Msg};

fn pin(key: &str, slot: Option<u8>) -> Msg {
    Msg::Graph(GraphMsg::PinColour { key: key.to_owned(), slot })
}

/// The Color item for the branch key `key`.
pub fn items(ui: &mut egui::Ui, t: Lang, key: &str, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let main = snap.graph.owners.mainline.as_deref() == Some(key);
    let (auto_dot, auto_tip) = if main {
        (Some(line(Swatch::Trunk)), t.tr("branch.color.automatic.main.tip"))
    } else {
        (None, t.tr("branch.color.automatic.tip"))
    };
    let branch = color::current().branch;
    let n = branch.preset.slots();
    let current = snap.graph.colouring.pins.get(key).map(|p| p % n);
    submenu(ui, t.tr("branch.menu.color"), |ui| {
        let auto = menu::item(t.tr("branch.color.automatic"))
            .current(current.is_none())
            .icon(branch_dot::paint_slot, auto_dot)
            .show(ui)
            .on_hover_text(auto_tip);
        if auto.clicked() {
            msgs.push(pin(key, None));
            ui.close_menu();
        }
        separator(ui);
        for i in 0..n {
            let row = menu::item(branch_names::hue(t, &branch, i)).current(current == Some(i));
            if row.icon(branch_dot::paint_slot, Some(line(Swatch::Slot(i)))).show(ui).clicked() {
                msgs.push(pin(key, Some(i)));
                ui.close_menu();
            }
        }
    });
}
