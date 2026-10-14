//! The "Publish to GitHub" dialog: where to create the repository, its name, visibility and
//! description. Creating it adds it as `origin` and pushes the current branch.

use super::theme::color;
use crate::msg::Msg;
use crate::state::publish::PublishForm;
use eframe::egui::{self, RichText};
use trellis_core::forge;

/// Draws the form; returns true when the dialog should close.
pub fn form(ui: &mut egui::Ui, f: &mut PublishForm, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    ui.heading("Publish to GitHub");
    ui.label(RichText::new(format!("This repository has no remote. Create one and push {} to it.", f.branch)).color(color::weak()));
    ui.add_space(8.0);

    if f.accounts.len() > 1 {
        ui.horizontal(|ui| {
            ui.label("Account");
            let current = f.account().map(|a| a.display()).unwrap_or_default();
            egui::ComboBox::from_id_salt("publish-account").selected_text(current).width(260.0).show_ui(ui, |ui| {
                for (i, a) in f.accounts.iter().enumerate() {
                    if ui.selectable_label(i == f.account, a.display()).clicked() {
                        msgs.push(Msg::PublishAccount(i));
                    }
                }
            });
        });
    }

    ui.horizontal(|ui| {
        if f.loading {
            ui.spinner();
            ui.label(RichText::new("Signing in to GitHub…").color(color::weak()));
            return;
        }
        egui::ComboBox::from_id_salt("publish-owner").selected_text(f.owner.as_str()).width(160.0).show_ui(ui, |ui| {
            for o in &f.owners {
                ui.selectable_value(&mut f.owner, o.clone(), o);
            }
        });
        ui.label("/");
        ui.add(egui::TextEdit::singleline(&mut f.name).hint_text("repository-name").desired_width(f32::INFINITY));
    });
    let name = f.name.trim();
    if !name.is_empty() && !forge::valid_repo_name(name) {
        ui.label(RichText::new("Use letters, digits, '.', '-' and '_' only.").color(color::amber()));
    }
    ui.add(egui::TextEdit::singleline(&mut f.description).hint_text("Description (optional)").desired_width(f32::INFINITY));
    ui.horizontal(|ui| {
        ui.radio_value(&mut f.private, true, "Private");
        ui.radio_value(&mut f.private, false, "Public");
    });
    if let Some(e) = &f.error {
        ui.add_space(4.0);
        ui.label(RichText::new(e.as_str()).color(color::red()));
    }

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        let ready = idle && !f.loading && !f.creating && f.user.is_some() && !f.owner.is_empty() && forge::valid_repo_name(name);
        let label = if f.creating { "Creating…" } else { "Create and push" };
        if ui.add_enabled(ready, egui::Button::new(RichText::new(label).strong())).clicked() {
            msgs.push(Msg::Publish);
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
        if f.creating {
            ui.spinner();
        }
    });
    close
}
