//! Preferences > Commit signing: format, program, signing key (listed from gpg or ~/.ssh),
//! where the key's passphrase is asked, key generation, and signing defaults.

use super::{help, primary_button, row, text_field, warn};
use crate::msg::PrefsMsg;
use crate::msg::{FilePurpose, Msg};
use crate::state::PrefsState;
use crate::ui::theme::color;
use crate::ui::theme::controls;
use crate::ui::widgets::buttons;
use crate::ui::widgets::{self, Icon};
use eframe::egui::{self, RichText, Ui};
use trellis_core::settings::Settings;
use trellis_core::signing::SigningFormat;

const RENEW_HELP: &str = "https://docs.github.com/en/authentication/managing-commit-signature-verification/updating-an-expired-gpg-key";

fn set(msgs: &mut Vec<Msg>, key: &str, value: &str) {
    let value = value.trim();
    msgs.push(Msg::Prefs(PrefsMsg::GitConfig { global: true, key: key.into(), value: (!value.is_empty()).then(|| value.to_owned()) }));
}

const PROMPT_HELP: &str = "Off: GnuPG shows its own passphrase window, for example one that reads the passphrase from \
your system keychain. Needs git 2.31 or newer and GnuPG 2.1.12 or newer; with older versions GnuPG's own window is used.";

/// `settings` is edited in place (the shell saves it after the frame).
pub fn signing_page(ui: &mut Ui, settings: &mut Settings, prefs: &mut PrefsState, msgs: &mut Vec<Msg>) {
    let Some(git) = prefs.git.as_mut() else { return };
    let signing = &mut prefs.signing;
    let format = SigningFormat::from_config(&git.signing_format);

    row(ui, "Signing format", |ui| {
        let mut chosen = format;
        egui::ComboBox::from_id_salt("gpg-format").selected_text(format.label()).width(230.0).show_ui(ui, |ui| {
            for f in SigningFormat::ALL {
                ui.selectable_value(&mut chosen, f, f.label());
            }
        });
        if chosen != format {
            git.signing_format = chosen.config_value().to_owned();
            set(msgs, "gpg.format", chosen.config_value());
            msgs.push(Msg::Prefs(PrefsMsg::LoadSigningKeys));
        }
    });

    let program_label = match format {
        SigningFormat::OpenPgp => "GPG program",
        SigningFormat::Ssh => "SSH program",
        SigningFormat::X509 => "X.509 program",
    };
    row(ui, program_label, |ui| {
        ui.horizontal(|ui| {
            let value = match format {
                SigningFormat::OpenPgp => &mut git.gpg_program,
                SigningFormat::Ssh => &mut git.ssh_program,
                SigningFormat::X509 => &mut git.x509_program,
            };
            if text_field(ui, value, format.default_program()).lost_focus() {
                set(msgs, format.program_key(), value);
                msgs.push(Msg::Prefs(PrefsMsg::LoadSigningKeys));
            }
            if ui.add(buttons::secondary("Browse")).clicked() {
                msgs.push(Msg::Prefs(PrefsMsg::PickFile(FilePurpose::SigningProgram)));
            }
        });
        let text = match format {
            SigningFormat::OpenPgp => "Usually \"gpg\" or \"gpg2\" on your PATH, or browse to the program. Leave empty for git's default.",
            SigningFormat::Ssh => "The program that signs with SSH keys; \"ssh-keygen\" by default.",
            SigningFormat::X509 => "The S/MIME signing program; \"gpgsm\" by default.",
        };
        help(ui, text);
    });

    row(ui, "Signing key", |ui| {
        ui.horizontal(|ui| {
            let current = if git.signing_key.is_empty() { "<None>".to_owned() } else { git.signing_key.clone() };
            let mut chosen: Option<String> = None;
            egui::ComboBox::from_id_salt("signing-key").selected_text(current).width(300.0).show_ui(ui, |ui| {
                if ui.selectable_label(git.signing_key.is_empty(), "<None>").clicked() {
                    chosen = Some(String::new());
                }
                match &signing.keys {
                    None => {
                        ui.label(RichText::new("Loading…").color(color::weak()));
                    }
                    Some(keys) => {
                        for k in keys {
                            if ui.selectable_label(git.signing_key == k.id, k.describe()).clicked() {
                                chosen = Some(k.id.clone());
                            }
                        }
                    }
                }
            });
            if let Some(id) = chosen {
                git.signing_key = id.clone();
                set(msgs, "user.signingkey", &id);
            }
            if widgets::icon_button(ui, Icon::Refresh, "Reload keys").clicked() {
                msgs.push(Msg::Prefs(PrefsMsg::LoadSigningKeys));
            }
            if ui.add(buttons::secondary("Copy public key").enabled(!git.signing_key.is_empty())).clicked() {
                msgs.push(Msg::Prefs(PrefsMsg::CopyPublicKey));
            }
        });
        if let Some(e) = &signing.keys_error {
            warn(ui, e);
        }
        let selected = signing.keys.as_ref().and_then(|ks| ks.iter().find(|k| k.id == git.signing_key));
        if selected.is_some_and(|k| k.expired) {
            warn(ui, "This key has expired; commits signed with it won't verify.");
        }
        if format == SigningFormat::OpenPgp && ui.link("How do I renew an expired key?").clicked() {
            msgs.push(Msg::OpenUrl(RENEW_HELP.into()));
        }
        if format == SigningFormat::Ssh {
            help(ui, "Public keys found in your .ssh folder. Git signs with the matching private key or your SSH agent.");
        }
    });

    if format == SigningFormat::OpenPgp {
        row(ui, "Passphrase prompt", |ui| {
            controls::checkbox(ui, &mut settings.gpg_prompts_in_app, "Ask for the key's passphrase in Trellis");
            help(ui, PROMPT_HELP);
        });
        row(ui, "Generate new GPG key", |ui| {
            ui.horizontal(|ui| {
                controls::field(
                    ui,
                    egui::TextEdit::singleline(&mut signing.passphrase)
                        .password(true)
                        .hint_text("Passphrase (optional)")
                        .desired_width(220.0),
                );
                let can = !signing.generating && !git.user_name.is_empty() && !git.user_email.is_empty();
                // While generating, the spinner says why.
                let unnamed = git.user_name.is_empty() || git.user_email.is_empty();
                let why = (!signing.generating && unnamed).then_some("Set your name and email on the Git page first");
                if primary_button(ui, "Generate", can, why).clicked() {
                    msgs.push(Msg::Prefs(PrefsMsg::GenerateGpgKey));
                }
                if signing.generating {
                    ui.add(egui::Spinner::new().size(14.0));
                }
            });
            if git.user_name.is_empty() || git.user_email.is_empty() {
                warn(ui, "Set your name and email under Profile first.");
            }
            help(
                ui,
                "Creates a 4096-bit RSA key for the name and email in your profile, valid for 2 years, and makes it your signing key.",
            );
        });
    }

    if format == SigningFormat::Ssh {
        row(ui, "Allowed signers file", |ui| {
            ui.horizontal(|ui| {
                if text_field(ui, &mut git.ssh_allowed_signers, "~/.ssh/allowed_signers").lost_focus() {
                    set(msgs, "gpg.ssh.allowedSignersFile", &git.ssh_allowed_signers);
                }
                if ui.add(buttons::secondary("Browse")).clicked() {
                    msgs.push(Msg::Prefs(PrefsMsg::PickFile(FilePurpose::SshAllowedSigners)));
                }
            });
            help(ui, "Lists trusted signers so git can verify SSH signatures (gpg.ssh.allowedSignersFile).");
        });
    }

    row(ui, "Sign commits by default", |ui| {
        if controls::checkbox(ui, &mut git.gpgsign, "").changed() {
            set(msgs, "commit.gpgsign", &git.gpgsign.to_string());
        }
    });
    row(ui, "Sign tags by default", |ui| {
        if controls::checkbox(ui, &mut git.tag_gpgsign, "").changed() {
            set(msgs, "tag.gpgsign", &git.tag_gpgsign.to_string());
        }
        help(ui, "Trellis commits and tags through git, so these settings also apply on the command line.");
    });
}
