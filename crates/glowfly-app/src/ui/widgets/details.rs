//! The collapsed "Details" disclosure under a plain-words message: the literal text behind it
//! (a command, a service's own error) in small, weak, selectable monospace. egui's
//! `CollapsingHeader` paints its own triangle, so no font glyph is needed.

use crate::ui::theme::{color, type_scale};
use eframe::egui::{self, RichText, Ui};

/// A disclosure labelled `label`, closed at first, that shows `text` when opened. `id` keeps its
/// open state apart from other disclosures with the same label.
pub fn disclosure(ui: &mut Ui, id: impl std::hash::Hash, label: &str, text: &str) {
    egui::CollapsingHeader::new(label).id_salt(id).default_open(false).show(ui, |ui| {
        let text = RichText::new(text.trim_end()).monospace().size(type_scale::SMALL).color(color::weak());
        ui.add(egui::Label::new(text).selectable(true).wrap());
    });
}
