//! Paths Trellis won't act on: the working-tree banner about paths one file system can't keep
//! apart (and a `core.ignorecase` that disagrees with the disk), and the note after a file
//! row's path saying why its actions are off.

use super::theme::color;
use crate::state::path_hazards::warnings;
use eframe::egui::{self, RichText};
use trellis_core::repo::{FileEntry, Snapshot};

/// The banner above the working-tree lists, when there is something to say.
pub(super) fn banner(ui: &mut egui::Ui, snap: &Snapshot) {
    let lines = warnings(snap);
    if lines.is_empty() {
        return;
    }
    egui::Frame::group(ui.style()).fill(color::banner()).show(ui, |ui| {
        ui.label(RichText::new("File names").strong());
        for line in lines {
            ui.add(egui::Label::new(line).wrap());
        }
    });
    ui.add_space(6.0);
}

/// After a row's path: what is wrong with it, with the full reason on hover.
pub(super) fn mark(ui: &mut egui::Ui, f: &FileEntry) {
    if let Some(h) = &f.hazard {
        ui.label(RichText::new(format!("({})", h.label())).italics().color(color::weak())).on_hover_text(h.reason());
    }
}
