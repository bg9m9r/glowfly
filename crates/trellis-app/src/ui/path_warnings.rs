//! Paths Trellis won't act on: the working-tree banner about paths one file system can't keep
//! apart (and a `core.ignorecase` that disagrees with the disk), and the note after a file
//! row's path saying why its actions are off.

use super::theme::color;
use super::widgets::banner::{self, BannerKind};
use crate::state::path_hazards::warnings;
use crate::ui::theme::type_scale;
use eframe::egui::{self, RichText};
use trellis_core::repo::{FileEntry, Snapshot};

/// The banner above the working-tree lists, when there is something to say.
pub(super) fn banner(ui: &mut egui::Ui, snap: &Snapshot) {
    let lines = warnings(snap);
    if lines.is_empty() {
        return;
    }
    banner::frame(ui, BannerKind::Warning, None, |ui| {
        ui.label(type_scale::section_text(ui, "File names"));
        for line in lines {
            ui.add(egui::Label::new(RichText::new(line).color(color::text())).wrap());
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
