//! Preferences > SSH: the key pairs in ~/.ssh, each with a Copy public key button, and the
//! Generate New Key form. The view edits only the form's text buffers; everything else is a
//! `Msg`.

use super::{help, primary_button, row, text_field, warn};
use crate::msg::Msg;
use crate::ssh::SshMsg;
use crate::state::ssh::{MISMATCH, NO_KEYGEN, SshPage, TAKEN};
use crate::ui::theme::color;
use crate::ui::widgets::{self, Icon};
use eframe::egui::{self, CornerRadius, RichText, Ui};
use trellis_core::ssh_keys::{SshKeyPair, SshListing, comment_problem};

pub fn ssh_page(ui: &mut Ui, page: &mut SshPage, msgs: &mut Vec<Msg>) {
    if let Some(error) = &page.error {
        row(ui, "Key Pairs", |ui| {
            warn(ui, error);
            ui.add_space(4.0);
            if ui.button("Reload").clicked() {
                msgs.push(Msg::Ssh(SshMsg::Reload));
            }
        });
        return;
    }
    let Some(listing) = page.listing.clone() else {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new());
            ui.label(RichText::new("Reading your .ssh folder…").color(color::weak()));
        });
        return;
    };
    key_pairs(ui, &listing, msgs);
    generate_form(ui, page, &listing, msgs);
}

fn key_pairs(ui: &mut Ui, listing: &SshListing, msgs: &mut Vec<Msg>) {
    row(ui, "Key Pairs", |ui| {
        ui.horizontal(|ui| {
            help(ui, "Private keys in your .ssh folder that have a matching .pub file. ssh and git use them to reach SSH remotes.");
            if widgets::icon_button(ui, Icon::Refresh, "Reload keys").clicked() {
                msgs.push(Msg::Ssh(SshMsg::Reload));
            }
        });
        ui.add_space(4.0);
        if listing.keygen.is_none() {
            warn(ui, NO_KEYGEN);
            ui.add_space(4.0);
        }
        if listing.keys.is_empty() {
            help(ui, &format!("No key pairs in {} yet. Generate one below.", listing.dir.display()));
        }
        for key in &listing.keys {
            key_row(ui, key, msgs);
        }
    });
}

/// One key pair: name and kind, comment, fingerprint, and the copy button on the right.
fn key_row(ui: &mut Ui, key: &SshKeyPair, msgs: &mut Vec<Msg>) {
    egui::Frame::new().fill(color::panel()).corner_radius(CornerRadius::same(6)).inner_margin(egui::Margin::symmetric(10, 8)).show(
        ui,
        |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(key.file_name()).strong().color(color::text()));
                        ui.label(RichText::new(key.kind_label()).size(12.0).color(color::weak()));
                    });
                    let comment = if key.comment.is_empty() { "No comment" } else { key.comment.as_str() };
                    ui.label(RichText::new(comment).size(12.0).color(color::weak()));
                    match &key.fingerprint {
                        Some(fp) => ui.label(RichText::new(fp).monospace().size(11.5).color(color::faint())),
                        None => ui.label(RichText::new("Fingerprint unavailable").size(11.5).color(color::faint())),
                    };
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    if ui.button("Copy public key").clicked() {
                        msgs.push(Msg::Ssh(SshMsg::CopyPublicKey(key.private.clone())));
                    }
                });
            });
        },
    );
    ui.add_space(4.0);
}

fn password(ui: &mut Ui, value: &mut String) {
    ui.add(egui::TextEdit::singleline(value).password(true).hint_text("Optional").desired_width(320.0).margin(egui::vec2(8.0, 5.0)));
}

fn generate_form(ui: &mut Ui, page: &mut SshPage, listing: &SshListing, msgs: &mut Vec<Msg>) {
    row(ui, "Generate New Key", |ui| {
        help(ui, "Creates an Ed25519 key pair with ssh-keygen.");
    });
    row(ui, "Save Location", |ui| {
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
    row(ui, "Confirm Passphrase", |ui| {
        password(ui, &mut page.confirm);
        if page.passphrase_mismatch() {
            warn(ui, MISMATCH);
        }
    });
    row(ui, "", |ui| {
        ui.horizontal(|ui| {
            if primary_button(ui, "Generate Key", page.can_generate()).clicked() {
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
