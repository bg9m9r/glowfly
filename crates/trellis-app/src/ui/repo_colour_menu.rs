//! The Color submenu that gives a repository its colour, shared by the tab menu and the
//! welcome screen's repository rows: None, then one row per repository hue with its swatch and
//! name, the current choice checked. A new tab (no repository yet) shows a blocked item that
//! says why.

use super::repo_swatch;
use super::theme::color;
use super::widgets::reason::{self, ItemState};
use crate::msg::{Msg, RecentMsg};
use crate::state::repo_colours::hue_name;
use eframe::egui;
use std::path::Path;
use trellis_core::i18n::Lang;
use trellis_core::settings::RepoColour;

/// A row's label, with the check glyph when it is the current choice.
fn checked(label: &str, on: bool) -> String {
    if on { format!("✔ {label}") } else { label.to_owned() }
}

/// The Color item for the repository at `path` (None: a new tab), whose colour is `current`.
pub fn items(ui: &mut egui::Ui, t: Lang, path: Option<&Path>, current: Option<RepoColour>, msgs: &mut Vec<Msg>) {
    let Some(path) = path else {
        reason::menu_button(ui, t.tr("repo_colour.menu"), "", ItemState::Blocked(t.tr("repo_colour.blocked")));
        return;
    };
    let set = |colour| Msg::Recent(RecentMsg::SetColour { path: path.to_path_buf(), colour });
    ui.menu_button(t.tr("repo_colour.menu"), |ui| {
        let none = repo_swatch::menu_row(ui, &checked(t.tr("repo_colour.none"), current.is_none()), None)
            .on_hover_text(t.tr("repo_colour.none.tip"));
        if none.clicked() {
            msgs.push(set(None));
            ui.close_menu();
        }
        ui.separator();
        for c in RepoColour::ALL {
            let label = checked(hue_name(t, c), current == Some(c));
            if repo_swatch::menu_row(ui, &label, Some(color::repo_hue(c))).clicked() {
                msgs.push(set(Some(c)));
                ui.close_menu();
            }
        }
    });
}
