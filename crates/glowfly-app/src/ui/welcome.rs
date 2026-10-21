//! The first-run welcome: a dialog with four steps (who you are, a hosting service, git, where
//! to start), drawn in the shared dialog frame (`dialog_frame`) with its own header: the band,
//! the progress bar and the step title (`welcome_header.rs`, the band's lattice drawing in
//! `welcome_art.rs`). The step bodies are in `welcome_steps.rs`, the Start step's choice cards in
//! `welcome_cards.rs`, and the buttons in the shared footer, with its keys. Drawn before the
//! other dialogs, so a prompt or dialog opened over it stacks on top; not drawn while
//! Preferences is open (it waits behind, on the same step).

use super::dialog_frame::{self, Cx, Dialog, Footer, Outcome};
use super::theme;
use super::welcome_header;
use super::welcome_steps;
use crate::msg::Msg;
use crate::onboarding::OnboardingMsg;
use crate::state::AppState;
use crate::state::onboarding::{Welcome, WelcomeStep};
use crate::ui::widgets::buttons;
use eframe::egui::{self, Id};
use glowfly_core::i18n::Lang;

/// The whole dialog's width, padding included (clamped to the window like any dialog).
const WIDTH: f32 = 760.0;
/// The body's least height, so the dialog doesn't jump between steps.
const BODY_MIN_H: f32 = 200.0;

pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    if st.prefs.is_some() {
        return;
    }
    let prompting = st.askpass.showing();
    // A dialog stacked on top (drawn after this one) owns Enter and Ctrl/Cmd+Enter.
    let covered = st.modal.is_some();
    let has_repo = st.snap.is_some();
    let (settings, git) = (&st.settings, &st.git);
    let t = settings.lang();
    let Some(w) = st.onboarding.welcome.as_mut() else { return };
    // Under a credential prompt or another dialog the welcome reads no keys.
    let cx = Cx { t, keyboard: !prompting && !covered, key: Id::new("trellis-welcome-dialog") };
    let no_connections = settings.connections.is_empty();
    let resp = theme::dialog(ctx, Id::new("trellis-welcome")).show(ctx, |ui| {
        let step = w.step;
        let greeting = w.greeting_name().map(str::to_owned);
        let content_w = WIDTH - 2.0 * f32::from(theme::DIALOG_PAD);
        let mut d = Dialog::new(cx, t.tr("welcome.title"))
            .width(content_w)
            .min_body(BODY_MIN_H)
            .header(move |ui| welcome_header::draw(ui, t, step, greeting.as_deref()))
            .begin(ui);
        d.body(ui, |ui| match step {
            WelcomeStep::Identity => welcome_steps::identity(ui, w, settings, !prompting),
            WelcomeStep::Connect => welcome_steps::connect(ui, t, &settings.connections, msgs),
            WelcomeStep::Git => welcome_steps::git(ui, t, git, msgs),
            WelcomeStep::Start => welcome_steps::start(ui, t, w, has_repo, msgs),
        });
        let out = d.finish(ui, footer(t, w, no_connections));
        answer(out, step, msgs);
    });
    // Taken every frame, so a stale click never carries over. The close button, Esc or a click
    // outside ends the welcome early, unless a credential prompt is on top.
    let closed = dialog_frame::take_closed(&resp);
    if closed || (resp.should_close() && !prompting) {
        msgs.push(Msg::Onboarding(OnboardingMsg::CloseWelcome));
    }
}

/// Back at the left edge (not on the first step), then the step's own buttons.
fn footer<'a>(t: Lang, w: &Welcome, no_connections: bool) -> Footer<'a> {
    let mut footer = Footer::new();
    if w.step.prev().is_some() {
        footer = footer.extra(buttons::secondary(t.tr("welcome.back.button")).enabled(!w.identity.saving));
    }
    match w.step {
        WelcomeStep::Identity => {
            let f = &w.identity;
            let reason = if f.reading { Some(t.tr("welcome.identity.reading")) } else { f.saving.then(|| t.tr("welcome.identity.saving")) };
            footer
                .alt(buttons::secondary(t.tr("welcome.skip_for_now.button")).enabled(!f.saving))
                .primary(buttons::primary(t.tr("welcome.continue.button")).enabled(!f.saving && !f.reading).why(reason))
        }
        WelcomeStep::Connect if no_connections => footer.alt(buttons::secondary(t.tr("welcome.skip_step.button"))),
        WelcomeStep::Connect | WelcomeStep::Git => footer.primary(buttons::primary(t.tr("welcome.continue.button"))),
        // The cards are the step's main actions; Done just ends the welcome.
        WelcomeStep::Start => footer.close_label(t.tr("welcome.done.button")),
    }
}

/// What the footer's buttons (or keys) mean on `step`. `out.attempted` needs nothing: the
/// primary is disabled only while git's settings are read or written, when the fields can't be
/// edited, and SaveIdentity itself shows every problem of a bad form.
fn answer(out: Outcome, step: WelcomeStep, msgs: &mut Vec<Msg>) {
    let mut send = |m: OnboardingMsg| msgs.push(Msg::Onboarding(m));
    if out.extra == Some(0) {
        send(OnboardingMsg::Back);
    }
    match step {
        WelcomeStep::Identity => {
            if out.primary {
                send(OnboardingMsg::SaveIdentity);
            }
            if out.alt == Some(0) {
                send(OnboardingMsg::Skip);
            }
        }
        WelcomeStep::Connect => {
            if out.alt == Some(0) {
                send(OnboardingMsg::Skip);
            }
            if out.primary {
                send(OnboardingMsg::Next);
            }
        }
        WelcomeStep::Git => {
            if out.primary {
                send(OnboardingMsg::Next);
            }
        }
        WelcomeStep::Start => {
            if out.dismiss {
                send(OnboardingMsg::Finish(None));
            }
        }
    }
}
