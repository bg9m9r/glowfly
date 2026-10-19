//! The "Publish to GitHub" dialog: where to create the repository, its name, visibility and
//! description. Creating it adds it as `origin` and pushes the current branch.

use super::dialog_frame::{Cx, Dialog, Footer};
use super::theme::color;
use super::widgets::buttons;
use crate::msg::Msg;
use crate::msg::PublishMsg;
use crate::state::publish::PublishForm;
use crate::ui::theme::controls;
use crate::ui::widgets::form_message::{self, Message};
use eframe::egui::{self, RichText};
use trellis_core::forge;

/// Draws the form; returns true when the dialog should close.
pub fn form(ui: &mut egui::Ui, cx: Cx, f: &mut PublishForm, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let subtitle = format!("This repository has no remote. Create one and push {} to it.", f.branch);
    let d = Dialog::new(cx, "Publish to GitHub").subtitle(subtitle).begin(ui);

    if f.accounts.len() > 1 {
        ui.horizontal(|ui| {
            ui.label("Account");
            let current = f.account().map(|a| a.display()).unwrap_or_default();
            egui::ComboBox::from_id_salt("publish-account").selected_text(current).width(260.0).show_ui(ui, |ui| {
                for (i, a) in f.accounts.iter().enumerate() {
                    if ui.selectable_label(i == f.account, a.display()).clicked() {
                        msgs.push(Msg::Publish(PublishMsg::PublishAccount(i)));
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
        let r = controls::field(ui, egui::TextEdit::singleline(&mut f.name).hint_text("repository-name").desired_width(f32::INFINITY));
        form_message::track(&mut f.touched, "name", &r);
    });
    let name = f.name.trim();
    let bad = (!name.is_empty() && !forge::valid_repo_name(name)).then(|| Message::error("Use letters, digits, '.', '-' and '_' only."));
    form_message::field(ui, &f.touched, "name", bad);
    controls::field(ui, egui::TextEdit::singleline(&mut f.description).hint_text("Description (optional)").desired_width(f32::INFINITY));
    ui.horizontal(|ui| {
        controls::radio_value(ui, &mut f.private, true, "Private");
        controls::radio_value(ui, &mut f.private, false, "Public");
    });
    if let Some(e) = &f.error {
        ui.add_space(6.0);
        form_message::form(ui, Message::error(e.as_str()));
    }
    if f.creating {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new("Creating the repository…").color(color::weak()));
        });
    }

    let ready = idle && !f.loading && !f.creating && f.user.is_some() && !f.owner.is_empty() && forge::valid_repo_name(name);
    let label = if f.creating { "Creating…" } else { "Create and push" };
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary(label).enabled(ready)));
    if out.attempted {
        f.touched.attempt();
    }
    if out.primary {
        msgs.push(Msg::Publish(PublishMsg::Publish));
    }
    out.dismiss
}
