//! Preferences > SSH: the Generate New Key form. The view edits only the form's text buffers;
//! Generate is a `Msg`.

use super::{help, primary_button, row, text_field, warn};
use crate::msg::Msg;
use crate::ssh::SshMsg;
use crate::state::ssh::{MISMATCH, SshPage, TAKEN};
use crate::ui::theme::controls;
use eframe::egui::{self, Ui};
use trellis_core::ssh_keys::{SshListing, comment_problem};

fn password(ui: &mut Ui, value: &mut String) {
    controls::field(
        ui,
        egui::TextEdit::singleline(value).password(true).hint_text("Optional").desired_width(320.0).margin(egui::vec2(8.0, 5.0)),
    );
}

pub(super) fn generate_form(ui: &mut Ui, page: &mut SshPage, listing: &SshListing, msgs: &mut Vec<Msg>) {
    row(ui, "Generate new key", |ui| {
        help(ui, "Creates an Ed25519 key pair with ssh-keygen.");
    });
    row(ui, "Save location", |ui| {
        let hint = trellis_core::ssh_keys::default_location(&listing.home).display().to_string();
        text_field(ui, &mut page.location, &hint);
        if let Some(problem) = page.location_problem() {
            warn(ui, problem.message());
        } else if page.location_taken() {
            warn(ui, TAKEN);
        }
    });
    row(ui, "Comment", |ui| {
        text_field(ui, &mut page.comment, "ada@example.com");
        if let Some(problem) = comment_problem(&page.comment) {
            warn(ui, problem);
        }
        help(ui, "Stored in the public key to tell keys apart; usually your email. Empty uses ssh-keygen's default (user@host).");
    });
    row(ui, "Passphrase", |ui| {
        password(ui, &mut page.passphrase);
        help(ui, "Optional. Encrypts the private key; ssh asks for it, or your SSH agent remembers it, when the key is used.");
    });
    row(ui, "Confirm passphrase", |ui| {
        password(ui, &mut page.confirm);
        if page.passphrase_mismatch() {
            warn(ui, MISMATCH);
        }
    });
    row(ui, "", |ui| {
        ui.horizontal(|ui| {
            // Not the problems already warned about above.
            let warned = |w: &str| {
                w == MISMATCH || page.location_problem().is_some_and(|p| p.message() == w) || comment_problem(&page.comment) == Some(w)
            };
            let why = page.blocked().filter(|w| !warned(w));
            if primary_button(ui, "Generate key", page.can_generate(), why.as_deref()).clicked() {
                msgs.push(Msg::Ssh(SshMsg::Generate { overwrite: false }));
            }
            if page.generating {
                ui.add(egui::Spinner::new().size(14.0));
            }
        });
        if let Some(e) = &page.form_error {
            warn(ui, e);
        }
    });
}
