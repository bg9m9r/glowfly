//! The "Trust this repository?" dialog: what in the repository's own config and hooks makes git
//! run programs, and the Trust / Don't trust choice (update/trust.rs records it). When a git
//! job the user started waits for the decision, it names that job and offers Trust and run or
//! Cancel instead.

use super::theme::color;
use super::widgets::{self, Icon};
use crate::msg::{Msg, TrustMsg};
use crate::state::trust::{TrustPrompt, hooks_label, shown_value};
use eframe::egui::{self, RichText, Ui};
use std::path::Path;
use trellis_core::text::printable;
use trellis_core::trust::TrustScan;

/// Draws the prompt; returns true when Cancel was clicked (the dialog closes without a
/// decision). The other buttons send a decision and update closes it.
pub fn show(ui: &mut Ui, p: &TrustPrompt, msgs: &mut Vec<Msg>) -> bool {
    ui.heading("Trust this repository?");
    ui.add_space(2.0);
    ui.label(RichText::new(printable(&p.name)).strong());
    ui.add(egui::Label::new(RichText::new(trellis_core::native_path::shown(&p.workdir)).color(color::weak())).wrap());
    if let Some(label) = &p.held {
        let what = format!("{} needs git to run this repository's settings and hooks.", printable(label));
        ui.add(egui::Label::new(RichText::new(what).strong()).wrap());
    }
    if p.refused {
        ui.label(RichText::new("You chose not to trust it before.").color(color::weak()));
    }
    ui.add_space(8.0);
    if p.scan.is_empty() {
        ui.add(egui::Label::new("Nothing in its settings or hooks runs programs right now.").wrap());
    } else {
        ui.add(
            egui::Label::new("This repository's own settings or hooks make git run programs. So far Trellis has only read its files.")
                .wrap(),
        );
        ui.add_space(8.0);
        egui::ScrollArea::vertical().max_height(180.0).auto_shrink([false, true]).show(ui, |ui| {
            findings(ui, &p.scan, &p.workdir);
            for m in &p.scan.submodules {
                ui.add_space(4.0);
                ui.label(RichText::new(format!("In submodule {}", printable(&m.path))).strong());
                ui.indent(("trust-submodule", &m.path), |ui| findings(ui, &m.scan, &p.workdir));
            }
        });
    }
    ui.add_space(8.0);
    ui.add(
        egui::Label::new(
            "Trust it only if you know where it came from. Until then, Trellis won't fetch in the background, \
             ask its credential helper for sign-ins or let git read its status, and it asks before running git \
             commands you start.",
        )
        .wrap(),
    );
    ui.add_space(12.0);
    let mut cancel = false;
    ui.horizontal(|ui| {
        if p.held.is_some() {
            if ui.button("Trust and run").clicked() {
                msgs.push(Msg::Trust(TrustMsg::Decide { real: p.real.clone(), trusted: true }));
            }
            cancel = ui.button("Cancel").clicked();
            return;
        }
        for (label, trusted) in [("Trust", true), ("Don't trust", false)] {
            if ui.button(label).clicked() {
                msgs.push(Msg::Trust(TrustMsg::Decide { real: p.real.clone(), trusted }));
            }
        }
    });
    cancel
}

/// The settings and hooks of one repository (the top one or a submodule).
fn findings(ui: &mut Ui, scan: &TrustScan, workdir: &Path) {
    if !scan.config.is_empty() {
        ui.label(RichText::new("Settings that run commands").strong());
        for c in &scan.config {
            let row = format!("{} = {}", printable(&c.key), shown_value(&c.value));
            ui.add(egui::Label::new(RichText::new(row).monospace()).wrap());
        }
        ui.add_space(6.0);
    }
    if !scan.hooks.is_empty() {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
            widgets::paint_icon(ui.painter(), rect.center(), Icon::Hook, color::text());
            ui.label(RichText::new("Hooks").strong());
            if let Some(dir) = &scan.hooks_dir {
                ui.label(RichText::new(format!("in {}", hooks_label(dir, workdir))).color(color::weak()));
            }
        });
        for h in &scan.hooks {
            ui.label(RichText::new(printable(h)).monospace());
        }
    }
}
