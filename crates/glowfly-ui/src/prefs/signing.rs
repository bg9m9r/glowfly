//! Preferences > Commit signing: format, program, signing key (listed from gpg or ~/.ssh),
//! where the key's passphrase is asked, key generation, and signing defaults. The switches and
//! lists write at once; the program and the allowed signers file wait for the Save / Revert bar.

use super::{check_row, help, primary_button, row, text_field, warn, warn_then};
use crate::msg::PrefsMsg;
use crate::msg::{FilePurpose, Msg};
use crate::state::PrefsState;
use crate::state::prefs::SigningState;
use crate::ui::theme::controls;
use crate::ui::theme::{color, type_scale};
use crate::ui::widgets::buttons;
use crate::ui::widgets::{self, Icon};
use eframe::egui::{self, RichText, Ui};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::Settings;
use glowfly_core::signing::SigningFormat;

/// The fallback when Extend by 2 years fails: how to extend a key by hand.
const RENEW_HELP: &str = "https://docs.github.com/en/authentication/managing-commit-signature-verification/updating-an-expired-gpg-key";

fn set(msgs: &mut Vec<Msg>, key: &str, value: &str) {
    let value = value.trim();
    msgs.push(Msg::Prefs(PrefsMsg::GitConfig { global: true, key: key.into(), value: (!value.is_empty()).then(|| value.to_owned()) }));
}

const PROMPT_HELP: &str = "Off: GnuPG shows its own passphrase window, for example one that reads the passphrase from \
your system keychain. Needs git 2.31 or newer and GnuPG 2.1.12 or newer; with older versions GnuPG's own window is used.";

/// `settings` is edited in place (the shell saves it after the frame).
pub fn signing_page(ui: &mut Ui, settings: &mut Settings, prefs: &mut PrefsState, msgs: &mut Vec<Msg>) {
    let t = settings.lang();
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
            // The Save / Revert bar writes it; the key list follows it at once.
            if text_field(ui, value, format.default_program()).lost_focus() {
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
            if widgets::icon_button(ui, Icon::Refresh, t.tr("prefs.signing.reload_keys.tip")).clicked() {
                msgs.push(Msg::Prefs(PrefsMsg::LoadSigningKeys));
            }
            let copy = buttons::secondary(t.tr("prefs.signing.copy_public_key.button")).enabled(!git.signing_key.is_empty());
            if ui.add(copy).clicked() {
                msgs.push(Msg::Prefs(PrefsMsg::CopyPublicKey));
            }
        });
        if let Some(e) = &signing.keys_error {
            warn(ui, e);
        }
        let selected = signing.keys.as_ref().and_then(|ks| ks.iter().find(|k| k.id == git.signing_key));
        match selected {
            Some(k) if k.revoked => warn(ui, t.tr("prefs.signing.revoked.warn")),
            Some(k) if k.expired => warn(ui, t.tr("prefs.signing.expired.warn")),
            _ => {}
        }
        // A revoked key can't be brought back by a new date.
        if format == SigningFormat::OpenPgp {
            extend_row(ui, t, selected.is_some_and(|k| k.expired && !k.revoked), signing, msgs);
        }
        if format == SigningFormat::Ssh {
            help(ui, "Public keys found in your .ssh folder. Git signs with the matching private key or your SSH agent.");
        }
    });

    if format == SigningFormat::OpenPgp {
        row(ui, "Passphrase prompt", |ui| {
            controls::checkbox(ui, &mut settings.gpg_prompts_in_app, "Ask for the key's passphrase in Glowfly");
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
                // The Save / Revert bar writes it.
                text_field(ui, &mut git.ssh_allowed_signers, "~/.ssh/allowed_signers");
                if ui.add(buttons::secondary("Browse")).clicked() {
                    msgs.push(Msg::Prefs(PrefsMsg::PickFile(FilePurpose::SshAllowedSigners)));
                }
            });
            help(ui, "Lists trusted signers so git can verify SSH signatures (gpg.ssh.allowedSignersFile).");
        });
    }

    if check_row(ui, "Sign commits", &mut git.gpgsign, "Sign new commits by default", None).changed() {
        set(msgs, "commit.gpgsign", &git.gpgsign.to_string());
    }
    let help = Some("Glowfly commits and tags through git, so these settings also apply on the command line.");
    if check_row(ui, "Sign tags", &mut git.tag_gpgsign, "Sign new tags by default", help).changed() {
        set(msgs, "tag.gpgsign", &git.tag_gpgsign.to_string());
    }
}

/// Extend by 2 years under the expired-key warning (`offer`); after a failure (which may have
/// extended the key but not a subkey, so it shows whether or not the key still reads expired),
/// what went wrong with a link to doing it by hand at its end.
fn extend_row(ui: &mut Ui, t: Lang, offer: bool, signing: &SigningState, msgs: &mut Vec<Msg>) {
    if offer {
        ui.horizontal(|ui| {
            let button = buttons::secondary(t.tr("prefs.signing.extend.button")).enabled(!signing.extending);
            if ui.add(button).clicked() {
                msgs.push(Msg::Prefs(PrefsMsg::ExtendKey));
            }
            if signing.extending {
                ui.add(egui::Spinner::new().size(16.0));
            }
        });
    }
    if let Some(e) = &signing.extend_error {
        warn_then(ui, &t.trf("prefs.signing.extend.failed.warn", &[("error", e)]), |ui| {
            // Whole, on the next row when it doesn't fit after the message.
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            if ui.link(RichText::new(t.tr("prefs.signing.extend.by_hand.link")).size(type_scale::SMALL)).clicked() {
                msgs.push(Msg::OpenUrl(RENEW_HELP.into()));
            }
        });
    }
}
