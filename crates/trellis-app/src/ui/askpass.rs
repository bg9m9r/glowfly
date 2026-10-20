//! The credential prompt: a modal on top of everything (Preferences and other dialogs
//! included) for what git or ssh asks through the askpass helper. Edits only the prompt's text
//! buffers and view flags; answering and cancelling go through `Msg`s.

use super::dialog_frame::{self, Cx, Dialog, Footer, Size};
use super::theme::color;
use super::widgets::text_field::{self, TextField};
use super::widgets::{self, FieldGroup, Icon, buttons};
use crate::msg::CredentialMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::askpass::PendingPrompt;
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use crate::ui::widgets::form_message::Message;
use eframe::egui::{self, Id, Key, RichText};
use trellis_core::askpass::{PromptKind, host, user_and_host};
use trellis_core::git_credential::is_http_url;

/// The wording and fields for one kind of prompt.
struct Layout {
    title: String,
    key_icon: bool,
    line: Option<String>,
    /// Under the line: who owns the key (if known) and its id.
    detail: Option<(Option<String>, String)>,
    /// The plain field's label (drawn above it, and its accessible name).
    user: Option<&'static str>,
    /// The masked field's label.
    secret: Option<&'static str>,
    token_hint: bool,
    /// Offer "Remember this sign-in" (an HTTPS sign-in).
    remember: bool,
    /// Small help above the footer.
    help: Option<&'static str>,
    /// A yes/no question: danger tone (Cancel is the default) and no line about saving.
    confirm: bool,
    /// Show ssh's unknown-host question as fields ([`super::askpass_host_key`]); otherwise a
    /// confirmation shows the whole prompt text.
    host_key: bool,
    primary: &'static str,
}

impl Layout {
    fn new(title: String, primary: &'static str) -> Self {
        Layout {
            title,
            key_icon: true,
            line: None,
            detail: None,
            user: None,
            secret: None,
            token_hint: false,
            remember: false,
            help: None,
            confirm: false,
            host_key: false,
            primary,
        }
    }
}

fn layout(p: &PendingPrompt) -> Layout {
    match &p.prompt.kind {
        PromptKind::Username { url } => Layout {
            line: Some(format!("Git needs credentials for {url}.")),
            user: Some("Username"),
            secret: Some(TOKEN_LABEL),
            token_hint: true,
            remember: is_http_url(url),
            ..Layout::new(format!("Sign in to {}", host(&p.prompt.kind).unwrap_or_else(|| url.clone())), "Sign in")
        },
        PromptKind::Password { target, user } => {
            let who = match (target.contains("://"), user, user_and_host(target).1) {
                (true, Some(u), Some(h)) => format!("{u}@{h}"),
                (true, None, Some(h)) => h,
                _ => target.clone(),
            };
            let http = is_http_url(target);
            let label = if http { TOKEN_LABEL } else { "Password" };
            Layout { secret: Some(label), remember: http, ..Layout::new(format!("Password for {who}"), "OK") }
        }
        PromptKind::Passphrase { key } => Layout {
            line: Some(format!("Enter the passphrase for {key}.")),
            secret: Some("Passphrase"),
            ..Layout::new("Unlock SSH key".into(), "Unlock")
        },
        PromptKind::GpgPassphrase { key, user, .. } => Layout {
            line: Some("Git is signing with your OpenPGP key. Enter its passphrase.".into()),
            detail: Some((user.clone(), key.clone())),
            secret: Some("Passphrase"),
            help: Some(GPG_HELP),
            ..Layout::new("Unlock signing key".into(), "Unlock")
        },
        PromptKind::Confirm => Layout { key_icon: false, confirm: true, ..Layout::new("Confirm SSH connection".into(), "Yes") },
        PromptKind::HostKey(_) => {
            Layout { key_icon: false, confirm: true, host_key: true, ..Layout::new("Unknown SSH host".into(), "Trust and connect") }
        }
        PromptKind::Other { secret } => {
            let field = Some("Answer");
            Layout {
                key_icon: false,
                line: Some(p.prompt.text.trim().to_owned()),
                user: if *secret { None } else { field },
                secret: if *secret { field } else { None },
                ..Layout::new("Git needs input".into(), "OK")
            }
        }
    }
}

/// The secret field's label where git accepts either an account password or a token.
const TOKEN_LABEL: &str = "Password or personal access token";

/// Help for a signing passphrase: why it isn't asked every time, and how to get GnuPG's window.
const GPG_HELP: &str = "gpg-agent remembers the passphrase for a while, so Trellis asks again only once it has \
forgotten. To use GnuPG's own window instead, turn this off in Preferences > Commit signing.";

/// The notice under the fields for a prompt asked again, or for a host whose credentials
/// failed last time.
fn notice(p: &PendingPrompt) -> Option<Message<'static>> {
    if p.retry {
        let text = match p.prompt.kind {
            PromptKind::Passphrase { .. } | PromptKind::GpgPassphrase { .. } => "That passphrase didn't unlock the key. Try again.",
            _ => "That didn't work. Try again.",
        };
        return Some(Message::error(text));
    }
    let h = host(&p.prompt.kind).filter(|_| p.rejected_before)?;
    Some(Message::warning(format!("The credentials last used for {h} were not accepted.")))
}

/// Draws the oldest waiting prompt, if any. A click outside does not cancel it; Esc and the
/// close button do.
pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let Some(front) = st.askpass.front().cloned() else { return };
    let l = layout(&front);
    let p = &mut st.askpass;
    let secret_id = Id::new("trellis-credentials-secret");
    let id = Id::new("trellis-credentials");
    // On top of everything, so it takes the keyboard.
    let cx = Cx { t: st.settings.lang(), keyboard: true, key: id };
    // The response's should_close() is deliberately ignored: the backdrop must not cancel.
    let resp = super::theme::dialog(ctx, id).show(ctx, |ui| {
        let mut d = Dialog::new(cx, l.title.as_str());
        if l.key_icon {
            d = d.icon(Icon::Key);
        }
        if l.host_key {
            // Room for a SHA256 fingerprint on one line.
            d = d.size(Size::Wide);
        }
        let mut d = d.begin(ui);
        if let Some(line) = &l.line {
            ui.add(egui::Label::new(RichText::new(line).color(color::weak())).wrap());
        }
        if let Some((user, key)) = &l.detail {
            ui.add_space(4.0);
            if let Some(user) = user {
                ui.add(egui::Label::new(RichText::new(user).color(color::text())).wrap());
            }
            ui.label(RichText::new(format!("Key {key}")).monospace().color(color::weak()));
        }
        if l.host_key
            && let PromptKind::HostKey(k) = &front.prompt.kind
        {
            super::askpass_host_key::body(ui, &mut d, k, &front.prompt.text, front.id);
        } else if l.confirm {
            d.body(ui, |ui| ui.add(egui::Label::new(RichText::new(front.prompt.text.trim_end())).wrap()));
        }
        if l.line.is_some() || l.detail.is_some() || l.confirm {
            ui.add_space(12.0);
        }
        let focus = std::mem::take(&mut p.focus_pending);
        // The notice for a retry goes under the last field.
        let mut notice = notice(&front);
        let user_notice = if l.secret.is_none() { notice.take() } else { None };
        let mut submit = false;
        if let Some(label) = l.user {
            let r = FieldGroup::new(label).note(user_notice).show(ui, |ui| ui.add(TextField::new(&mut p.user)));
            if focus {
                r.request_focus();
            }
            if text_field::entered(ui, &r) {
                // With the password still empty, Enter moves on to it.
                if l.secret.is_some() && p.secret.is_empty() {
                    ui.memory_mut(|m| m.request_focus(secret_id));
                } else {
                    submit = true;
                }
            }
        }
        if let Some(label) = l.secret {
            let r = FieldGroup::new(label).note(notice).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let width = ui.available_width() - 32.0;
                    let r = ui.add(TextField::new(&mut p.secret).id(secret_id).password(!p.reveal).width(width));
                    let (icon, tip) = if p.reveal { (Icon::EyeOff, "Hide") } else { (Icon::Eye, "Show") };
                    if widgets::icon_button(ui, icon, tip).clicked() {
                        p.reveal = !p.reveal;
                    }
                    r
                })
                .inner
            });
            if focus && l.user.is_none() {
                r.request_focus();
            }
            submit |= text_field::entered(ui, &r);
        }
        if l.token_hint {
            ui.add_space(2.0);
            let hint = "Most hosting services expect a personal access token here, not your account password.";
            ui.add(egui::Label::new(RichText::new(hint).color(color::weak()).size(type_scale::SMALL)).wrap());
        }
        if l.remember {
            ui.add_space(4.0);
            controls::checkbox(ui, &mut p.remember, "Remember this sign-in");
        }
        if let Some(help) = l.help {
            ui.add_space(2.0);
            ui.add(egui::Label::new(RichText::new(help).color(color::weak()).size(type_scale::SMALL)).wrap());
        }
        if !l.confirm {
            let saved = if l.remember && p.remember {
                "Trellis saves this sign-in with git's credential helper once git accepts it."
            } else {
                "Trellis doesn't save what you type here."
            };
            ui.add_space(6.0);
            ui.label(RichText::new(saved).color(color::weak()).size(type_scale::SMALL));
        }
        let valid = p.can_submit();
        let cancel = ui.input(|i| i.key_pressed(Key::Escape));
        let button = buttons::primary(l.primary).enabled(valid);
        // A yes/no question isn't answered by a stray Enter: its button keeps the primary style,
        // but the dialog's tone makes Cancel the default.
        let footer = if l.confirm { Footer::new().danger(button) } else { Footer::new().primary(button) };
        let out = d.finish(ui, footer.cancel().submit(submit));
        (cancel || out.dismiss, out.primary && valid)
    });
    // Decided after the dialog is shown, so its close button counts too.
    let (cancel, submit) = resp.inner;
    if cancel || dialog_frame::take_closed(&resp) {
        msgs.push(Msg::Credentials(CredentialMsg::CancelCredentials));
    } else if submit {
        msgs.push(Msg::Credentials(CredentialMsg::AnswerCredentials));
    }
}
