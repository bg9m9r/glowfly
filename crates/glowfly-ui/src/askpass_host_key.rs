//! The body of the credential prompt for ssh's unknown-host question: one sentence, the host
//! and key as labelled fields, and ssh's whole text under a collapsed Details.

use super::dialog_frame::Open;
use super::theme::{color, type_scale};
use eframe::egui::{self, RichText, Ui};
use glowfly_core::askpass::HostKey;

const EXPLAIN: &str = "Glowfly hasn't connected to this server before: compare the fingerprint with the one the \
server's owner publishes before you trust it.";

/// Draws the host-key fields for prompt `id`, with `raw` (ssh's text) under Details.
pub(super) fn body(ui: &mut Ui, d: &mut Open, k: &HostKey, raw: &str, id: u64) {
    d.body(ui, |ui| {
        ui.add(egui::Label::new(RichText::new(EXPLAIN).color(color::weak())).wrap());
        ui.add_space(8.0);
        egui::Grid::new(("askpass-host-key", id)).num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
            let host = match k.port {
                Some(p) => format!("{} (port {p})", k.host),
                None => k.host.clone(),
            };
            field(ui, "Host", egui::Label::new(RichText::new(host).color(color::text())));
            if let Some(a) = &k.address {
                field(ui, "Address", egui::Label::new(RichText::new(a).color(color::text())));
            }
            field(ui, "Key type", egui::Label::new(RichText::new(&k.key_type).color(color::text())));
            let fp = RichText::new(&k.fingerprint).monospace().color(color::text());
            field(ui, "Fingerprint", egui::Label::new(fp).selectable(true).wrap());
        });
        ui.add_space(6.0);
        egui::CollapsingHeader::new("Details").id_salt(("askpass-details", id)).default_open(false).show(ui, |ui| {
            let text = RichText::new(raw.trim_end()).monospace().size(type_scale::SMALL).color(color::weak());
            ui.add(egui::Label::new(text).selectable(true).wrap());
        });
    });
}

/// One grid row: a weak name, then its value.
fn field(ui: &mut Ui, name: &str, value: egui::Label) {
    ui.label(RichText::new(name).color(color::weak()));
    ui.add(value);
    ui.end_row();
}
