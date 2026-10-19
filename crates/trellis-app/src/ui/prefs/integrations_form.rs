//! Preferences > Integrations: one card per connected account, and the sign-in form (Git
//! Credential Manager where the service supports it, or a host, user and token), both aligned
//! with the field column.

use super::{field_left, field_width, help, primary_button, row, text_field, trello};
use crate::msg::{ForgeMsg, Msg, PrefsMsg};
use crate::state::PrefsState;
use crate::ui::theme::{color, controls, type_scale};
use crate::ui::widgets::form_message::{self, Message};
use crate::ui::widgets::{self, Icon, buttons};
use eframe::egui::{self, Align, Layout, RichText, Sense, Stroke, Ui, vec2};
use trellis_core::forge::{self, Account, Connection, Provider, Target};
use trellis_core::i18n::Lang;

/// Why Connect waits: the host isn't known yet.
const NO_HOST: &str = "Enter the host domain first";

/// Draws `add` in the field column: indented past the labels and as wide as the fields.
pub fn aligned(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let (left, width) = (field_left(ui), field_width(ui));
    ui.horizontal(|ui| {
        ui.add_space(left);
        ui.vertical(|ui| {
            ui.set_max_width(width);
            add(ui);
        });
    });
}

/// One connected account: a user icon, "Connected", the account, who signed in and how, and
/// Disconnect at the right. As wide as the column it is drawn in.
pub fn account_card(ui: &mut Ui, c: &Connection, msgs: &mut Vec<Msg>) {
    let w = ui.available_width();
    egui::Frame::new().fill(color::raised()).corner_radius(4).inner_margin(egui::Margin::symmetric(16, 12)).show(ui, |ui| {
        ui.set_width((w - 32.0).max(0.0));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add(buttons::secondary("Disconnect").small()).clicked() {
                msgs.push(Msg::Forge(ForgeMsg::Disconnect(c.account.clone())));
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                let (r, _) = ui.allocate_exact_size(vec2(26.0, 40.0), Sense::hover());
                ui.painter().circle_stroke(r.center(), 11.0, Stroke::new(1.5_f32, color::weak()));
                widgets::paint_icon(ui.painter(), r.center(), Icon::User, color::weak());
                ui.vertical(|ui| {
                    ui.label(RichText::new("Connected").color(color::added()));
                    ui.add(egui::Label::new(RichText::new(c.account.display()).color(color::text())).wrap());
                    let how = format!("{} via {}", c.user, c.method);
                    ui.add(egui::Label::new(RichText::new(how).size(type_scale::SMALL).color(color::weak())).wrap());
                });
            });
        });
    });
}

/// The sign-in form; with `cancel` (an account is already connected) it ends with Cancel.
pub fn sign_in_form(ui: &mut Ui, t: Lang, prefs: &mut PrefsState, cancel: bool, msgs: &mut Vec<Msg>) {
    let provider = prefs.int_provider;
    let account = forge::parse_host(provider, &prefs.int_host).map(|(host, org)| Account { provider, host, org });
    if provider.supports_gcm() {
        aligned(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let enabled = !prefs.int_busy && account.is_some();
                let label = format!("Connect to {}", provider.name());
                let tip = if account.is_some() { "Sign in with Git Credential Manager (the same sign-in git push uses)" } else { NO_HOST };
                // While busy, the spinner says why.
                let why = (!prefs.int_busy && account.is_none()).then_some(NO_HOST);
                if primary_button(ui, &label, enabled, why).on_hover_text(tip).clicked()
                    && let Some(a) = &account
                {
                    msgs.push(Msg::Forge(ForgeMsg::Connect { account: a.clone(), user: None, token: None }));
                }
                if prefs.int_busy {
                    ui.add(egui::Spinner::new().size(14.0));
                }
            });
        });
        ui.add_space(12.0);
        aligned(ui, |ui| {
            ui.label(RichText::new("or").size(type_scale::BODY).color(color::text()));
        });
        ui.add_space(12.0);
    }
    if provider.fixed_host().is_none() {
        row(ui, "Host domain", |ui| {
            text_field(ui, &mut prefs.int_host, provider.host_hint());
        });
    }
    let is_trello = provider == Provider::Trello;
    if let Some(label) = provider.username_label() {
        row(ui, label, |ui| {
            let hint = match label {
                "Email" => "you@example.com",
                _ if is_trello => "32-character key",
                _ => "username",
            };
            text_field(ui, &mut prefs.int_user, hint);
            if is_trello {
                trello::key_link(ui, msgs);
            }
        });
    }
    let token_label = match provider {
        Provider::Bitbucket => "App password",
        Provider::JiraCloud => "API token",
        Provider::Trello => "Token",
        _ => "Personal access token",
    };
    row(ui, token_label, |ui| {
        let w = super::fit(ui, 320.0);
        controls::field(ui, egui::TextEdit::singleline(&mut prefs.int_token).password(true).desired_width(w).margin(vec2(8.0, 5.0)));
        help(ui, &format!("{} Stored in the Windows Credential Manager, never in Trellis's settings file.", provider.token_help()));
        if is_trello {
            trello::token_link(ui, &prefs.int_user, msgs);
        } else if let Some(url) = account.as_ref().and_then(Target::token_url)
            && ui.link("Create a token").clicked()
        {
            msgs.push(Msg::OpenUrl(url));
        }
    });
    aligned(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            let needs_user = matches!(provider, Provider::JiraCloud | Provider::Trello) && prefs.int_user.trim().is_empty();
            let valid = !prefs.int_token.trim().is_empty() && account.is_some() && !prefs.int_busy && !needs_user;
            if primary_button(ui, "Connect", valid, None).clicked()
                && let Some(a) = &account
            {
                let user = Some(prefs.int_user.trim().to_owned()).filter(|u| !u.is_empty());
                msgs.push(Msg::Forge(ForgeMsg::Connect { account: a.clone(), user, token: Some(prefs.int_token.trim().to_owned()) }));
            }
            if cancel && ui.add(buttons::secondary(t.tr("common.cancel.button"))).clicked() {
                msgs.push(Msg::Prefs(PrefsMsg::ShowAccountForm(false)));
            }
            if prefs.int_busy {
                ui.add(egui::Spinner::new().size(14.0));
            }
        });
    });
    if let Some(e) = &prefs.int_error {
        aligned(ui, |ui| form_message::line(ui, Message::error(e.as_str())));
    }
}
