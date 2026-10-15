//! The credential prompt: a modal on top of everything (Preferences and other dialogs
//! included) for what git or ssh asks through the askpass helper. Edits only the prompt's text
//! buffers and view flags; answering and cancelling go through `Msg`s.

use super::theme::color;
use super::widgets::{self, Icon};
use crate::msg::CredentialMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::askpass::PendingPrompt;
use eframe::egui::{self, Id, Key, RichText};
use trellis_core::askpass::{PromptKind, host, user_and_host};

/// The wording and fields for one kind of prompt.
struct Layout {
    title: String,
    key_icon: bool,
    line: Option<String>,
    /// The plain field and its hint.
    user: Option<&'static str>,
    /// The masked field and its hint.
    secret: Option<&'static str>,
    token_hint: bool,
    /// Show the whole prompt text (a host-key question).
    full_text: bool,
    primary: &'static str,
    secondary: &'static str,
}

impl Layout {
    fn new(title: String, primary: &'static str) -> Self {
        Layout {
            title,
            key_icon: true,
            line: None,
            user: None,
            secret: None,
            token_hint: false,
            full_text: false,
            primary,
            secondary: "Cancel",
        }
    }
}

fn layout(p: &PendingPrompt) -> Layout {
    match &p.prompt.kind {
        PromptKind::Username { url } => Layout {
            line: Some(format!("Git needs credentials for {url}.")),
            user: Some("Username"),
            secret: Some("Password or token"),
            token_hint: true,
            ..Layout::new(format!("Sign in to {}", host(&p.prompt.kind).unwrap_or_else(|| url.clone())), "Sign in")
        },
        PromptKind::Password { target, user } => {
            let who = match (target.contains("://"), user, user_and_host(target).1) {
                (true, Some(u), Some(h)) => format!("{u}@{h}"),
                (true, None, Some(h)) => h,
                _ => target.clone(),
            };
            Layout { secret: Some("Password"), ..Layout::new(format!("Password for {who}"), "OK") }
        }
        PromptKind::Passphrase { key } => Layout {
            line: Some(format!("Enter the passphrase for {key}.")),
            secret: Some("Passphrase"),
            ..Layout::new("Unlock SSH key".into(), "Unlock")
        },
        PromptKind::Confirm => {
            Layout { key_icon: false, full_text: true, secondary: "No", ..Layout::new("Confirm SSH connection".into(), "Yes") }
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

/// The notice under the fields for a prompt asked again, or for a host whose credentials
/// failed last time.
fn notice(p: &PendingPrompt) -> Option<(String, egui::Color32)> {
    if p.retry {
        let text = match p.prompt.kind {
            PromptKind::Passphrase { .. } => "That passphrase didn't unlock the key. Try again.",
            _ => "That didn't work. Try again.",
        };
        return Some((text.into(), color::red()));
    }
    let h = host(&p.prompt.kind).filter(|_| p.rejected_before)?;
    Some((format!("The credentials last used for {h} were not accepted."), color::amber()))
}

/// Draws the oldest waiting prompt, if any. A click outside does not cancel it; Esc does.
pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let Some(front) = st.askpass.front().cloned() else { return };
    let l = layout(&front);
    let p = &mut st.askpass;
    let secret_id = Id::new("trellis-credentials-secret");
    // The response's should_close() is deliberately ignored: the backdrop must not cancel.
    egui::Modal::new(Id::new("trellis-credentials")).show(ctx, |ui| {
        ui.set_width(420.0);
        ui.horizontal(|ui| {
            if l.key_icon {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                widgets::paint_icon(ui.painter(), rect.center(), Icon::Key, color::text());
                ui.add_space(2.0);
            }
            ui.heading(l.title.as_str());
        });
        ui.add_space(4.0);
        if let Some(line) = &l.line {
            ui.add(egui::Label::new(RichText::new(line).color(color::weak())).wrap());
        }
        if l.full_text {
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                ui.add(egui::Label::new(RichText::new(front.prompt.text.trim_end()).monospace()).wrap());
            });
        }
        ui.add_space(8.0);
        let enter = ui.input(|i| i.key_pressed(Key::Enter));
        let focus = std::mem::take(&mut p.focus_pending);
        let mut submit = false;
        if let Some(hint) = l.user {
            let r = ui.add(egui::TextEdit::singleline(&mut p.user).hint_text(hint).desired_width(f32::INFINITY));
            if focus {
                r.request_focus();
            }
            if r.lost_focus() && enter {
                // With the password still empty, Enter moves on to it.
                if l.secret.is_some() && p.secret.is_empty() {
                    ui.memory_mut(|m| m.request_focus(secret_id));
                } else {
                    submit = true;
                }
            }
        }
        if let Some(hint) = l.secret {
            ui.horizontal(|ui| {
                let width = ui.available_width() - 32.0;
                let field =
                    egui::TextEdit::singleline(&mut p.secret).id(secret_id).password(!p.reveal).hint_text(hint).desired_width(width);
                let r = ui.add(field);
                if focus && l.user.is_none() {
                    r.request_focus();
                }
                submit |= r.lost_focus() && enter;
                let (icon, tip) = if p.reveal { (Icon::EyeOff, "Hide") } else { (Icon::Eye, "Show") };
                if widgets::icon_button(ui, icon, tip).clicked() {
                    p.reveal = !p.reveal;
                }
            });
        }
        if let Some((text, col)) = notice(&front) {
            ui.add(egui::Label::new(RichText::new(text).color(col)).wrap());
        }
        if l.token_hint {
            ui.add_space(2.0);
            let hint = "Most hosting services expect a personal access token here, not your account password.";
            ui.add(egui::Label::new(RichText::new(hint).color(color::weak()).size(12.0)).wrap());
        }
        ui.add_space(10.0);
        let valid = p.can_submit();
        let mut cancel = ui.input(|i| i.key_pressed(Key::Escape));
        ui.horizontal(|ui| {
            submit |= ui.add_enabled(valid, egui::Button::new(RichText::new(l.primary).strong())).clicked();
            cancel |= ui.button(l.secondary).clicked();
        });
        if !l.full_text {
            ui.add_space(6.0);
            ui.label(RichText::new("Trellis doesn't save what you type here.").color(color::weak()).size(12.0));
        }
        if cancel {
            msgs.push(Msg::Credentials(CredentialMsg::CancelCredentials));
        } else if submit && valid {
            msgs.push(Msg::Credentials(CredentialMsg::AnswerCredentials));
        }
    });
}
