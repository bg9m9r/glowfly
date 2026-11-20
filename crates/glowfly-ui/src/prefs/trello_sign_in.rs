//! Integrations > Trello with Glowfly's own API key set: "Sign in with Trello" (or the wait for
//! Trello's answer, or why it ended), the manual fields under "Use your own API key", and, once
//! a Trello account connects, the offer to use it for the open repository.

use super::integrations::SignIn;
use super::integrations_form::{aligned, sign_in_form};
use super::{help, primary_button, trello};
use crate::theme::{color, type_scale};
use crate::widgets::buttons;
use crate::widgets::form_message::{self, Message};
use eframe::egui::collapsing_header::{CollapsingState, paint_default_icon};
use eframe::egui::{self, RichText, Sense, Ui};
use glowfly_core::forge::{self, Account};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::{Settings, TrackerChoice};
use glowfly_model::msg::{ForgeMsg, Msg};
use glowfly_model::state::PrefsState;
use glowfly_model::state::trello::SignInStep;
use std::path::Path;

/// The sign-in with Glowfly's `key`; with `cancel` (an account is already connected) the
/// manual form ends with Cancel.
pub(super) fn sign_in(ui: &mut Ui, t: Lang, key: &str, form: SignIn<'_>, cancel: bool, msgs: &mut Vec<Msg>) {
    let prefs = form.prefs;
    let step = &prefs.trello_sign_in.step;
    aligned(ui, |ui| match step {
        SignInStep::Waiting(_) => {
            ui.horizontal_wrapped(|ui| {
                ui.add(crate::widgets::Spinner::new().size(14.0));
                ui.label(RichText::new(t.tr("prefs.trello.sign_in.waiting")).size(type_scale::BODY).color(color::text()));
                ui.add_space(8.0);
                if ui.add(buttons::secondary(t.tr("common.cancel.button"))).clicked() {
                    msgs.push(Msg::Forge(ForgeMsg::CancelTrelloSignIn));
                }
            });
            help(ui, t.tr("prefs.trello.sign_in.waiting_help"));
        }
        SignInStep::Idle | SignInStep::Failed { .. } => {
            ui.horizontal_wrapped(|ui| {
                if primary_button(ui, t.tr("prefs.trello.sign_in.button"), !prefs.int_busy, None).clicked() {
                    msgs.push(Msg::Forge(ForgeMsg::StartTrelloSignIn));
                }
                if prefs.int_busy {
                    ui.add(crate::widgets::Spinner::new().size(14.0));
                }
            });
            help(ui, t.tr("prefs.trello.sign_in.help"));
            if let SignInStep::Failed { message, fallback } = step {
                form_message::line(ui, Message::error(message.as_str()));
                if *fallback {
                    token_page(ui, t, key, msgs);
                }
            }
        }
    });
    ui.add_space(14.0);
    let fallback = matches!(step, SignInStep::Failed { fallback: true, .. });
    own_key(ui, t, fallback, |ui| sign_in_form(ui, t, form, cancel, msgs));
}

/// Trello's page that shows a token for Glowfly's key, to paste into the manual form.
fn token_page(ui: &mut Ui, t: Lang, key: &str, msgs: &mut Vec<Msg>) {
    if let Some(url) = forge::trello_authorize_url(key, true)
        && ui.link(t.tr("prefs.trello.fallback.link")).clicked()
    {
        msgs.push(Msg::OpenUrl(url));
    }
    help(ui, t.tr("prefs.trello.fallback.help"));
}

/// "Use your own API key": the manual fields, collapsed unless the sign-in fell back to them.
fn own_key(ui: &mut Ui, t: Lang, open: bool, body: impl FnOnce(&mut Ui)) {
    let mut state = CollapsingState::load_with_default_open(ui.ctx(), ui.make_persistent_id("trello-own-key"), false);
    if open && !state.is_open() {
        state.set_open(true);
    }
    aligned(ui, |ui| {
        ui.horizontal(|ui| {
            state.show_toggle_button(ui, paint_default_icon);
            let title = RichText::new(t.tr("prefs.trello.own_key.title")).size(type_scale::BODY).color(color::text());
            if ui.add(egui::Label::new(title).sense(Sense::click())).clicked() {
                state.toggle(ui);
            }
        });
    });
    state.show_body_unindented(ui, |ui| {
        ui.add_space(8.0);
        body(ui);
    });
}

/// After a Trello account connects with a repository open: "Use Trello for this repository",
/// then, once it is the tracker, the Board row.
pub(super) fn offer_tracker(ui: &mut Ui, s: &Settings, prefs: &PrefsState, workdir: Option<&Path>, msgs: &mut Vec<Msg>) {
    let t = s.lang();
    let Some(workdir) = workdir.filter(|_| prefs.trello_sign_in.offer_tracker) else { return };
    let Some(account) = s.connections.iter().map(|c| &c.account).find(|a| a.provider == forge::Provider::Trello) else { return };
    match s.tracker_for(workdir) {
        TrackerChoice::Trello { account, board, board_name } => {
            trello::board_row(ui, &account, &board, &board_name, &prefs.trello_boards, msgs);
        }
        _ => aligned(ui, |ui| {
            if ui.add(buttons::secondary(t.tr("prefs.trello.use_here.button"))).clicked() {
                msgs.push(Msg::Forge(ForgeMsg::SetTracker(all_boards(account))));
            }
            help(ui, t.tr("prefs.trello.use_here.help"));
        }),
    }
}

fn all_boards(account: &Account) -> TrackerChoice {
    TrackerChoice::Trello { account: account.clone(), board: String::new(), board_name: String::new() }
}
