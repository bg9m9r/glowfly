//! The first-run welcome: a dialog with four steps (who you are, a hosting service, git, where
//! to start), progress dots and a close button. Drawn before the other dialogs, so a prompt or
//! dialog opened over it stacks on top; not drawn while Preferences is open (it waits behind,
//! on the same step). Step bodies are in `welcome_steps.rs`.

use super::theme::color;
use super::welcome_steps;
use super::widgets::{self, Icon};
use crate::msg::Msg;
use crate::onboarding::OnboardingMsg;
use crate::state::AppState;
use crate::state::onboarding::{Welcome, WelcomeStep};
use eframe::egui::{self, Align, Id, Layout, RichText, vec2};
use trellis_core::i18n::Lang;

const WIDTH: f32 = 500.0;

pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    if st.prefs.is_some() {
        return;
    }
    let prompting = st.askpass.showing();
    let has_repo = st.snap.is_some();
    let (settings, git) = (&st.settings, &st.git);
    let t = settings.lang();
    let Some(w) = st.onboarding.welcome.as_mut() else { return };
    let resp = super::theme::modal(ctx, Id::new("trellis-welcome")).show(ctx, |ui| {
        ui.set_width(WIDTH);
        header(ui, t, w.step, msgs);
        ui.add_space(14.0);
        ui.label(RichText::new(w.step.title(t)).size(17.0).strong().color(color::strong()));
        ui.add_space(6.0);
        match w.step {
            WelcomeStep::Identity => welcome_steps::identity(ui, w, settings, !prompting),
            WelcomeStep::Connect => welcome_steps::connect(ui, t, &settings.connections, msgs),
            WelcomeStep::Git => welcome_steps::git(ui, t, git, msgs),
            WelcomeStep::Start => welcome_steps::start(ui, t, w, has_repo, msgs),
        }
        ui.add_space(16.0);
        footer(ui, t, w, settings.connections.is_empty(), msgs);
    });
    // Esc or a click outside ends the welcome early, unless a credential prompt is on top.
    if resp.should_close() && !prompting {
        msgs.push(Msg::Onboarding(OnboardingMsg::CloseWelcome));
    }
}

/// The title, the close button and a row of progress dots with their captions.
fn header(ui: &mut egui::Ui, t: Lang, step: WelcomeStep, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(t.tr("welcome.title")).size(22.0).strong().color(color::text()));
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            if widgets::small_icon(ui, Icon::Close, t.tr("welcome.close.tip"), color::weak()).clicked() {
                msgs.push(Msg::Onboarding(OnboardingMsg::CloseWelcome));
            }
        });
    });
    ui.add_space(10.0);
    let slot = WIDTH / WelcomeStep::ALL.len() as f32;
    let (rect, _) = ui.allocate_exact_size(vec2(WIDTH, 34.0), egui::Sense::hover());
    let p = ui.painter();
    for s in WelcomeStep::ALL {
        let x = rect.left() + slot * (s.index() as f32 + 0.5);
        let centre = egui::pos2(x, rect.top() + 7.0);
        if s.index() + 1 < WelcomeStep::ALL.len() {
            let next = rect.left() + slot * (s.index() as f32 + 1.5);
            let line = if s.index() < step.index() { color::weak() } else { color::border() };
            p.line_segment([egui::pos2(x + 9.0, centre.y), egui::pos2(next - 9.0, centre.y)], egui::Stroke::new(1.5_f32, line));
        }
        let (fill, text) = match s.index().cmp(&step.index()) {
            std::cmp::Ordering::Less => (color::weak(), color::weak()),
            std::cmp::Ordering::Equal => (color::accent(), color::text()),
            std::cmp::Ordering::Greater => (color::border(), color::faint()),
        };
        p.circle_filled(centre, 5.5, fill);
        let font = egui::FontId::proportional(12.0);
        p.text(egui::pos2(x, rect.top() + 18.0), egui::Align2::CENTER_TOP, s.caption(t), font, text);
    }
}

/// Back on the left (not on the first step), the step's own buttons on the right.
fn footer(ui: &mut egui::Ui, t: Lang, w: &Welcome, no_connections: bool, msgs: &mut Vec<Msg>) {
    let send = |msgs: &mut Vec<Msg>, m: OnboardingMsg| msgs.push(Msg::Onboarding(m));
    ui.horizontal(|ui| {
        let busy = w.identity.saving;
        if w.step.prev().is_some() && ui.add_enabled(!busy, egui::Button::new(t.tr("welcome.back.button"))).clicked() {
            send(msgs, OnboardingMsg::Back);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| match w.step {
            WelcomeStep::Identity => {
                let f = &w.identity;
                let ready = !f.saving && !f.reading;
                if ui.add_enabled(ready, egui::Button::new(t.tr("welcome.continue.button"))).clicked() {
                    send(msgs, OnboardingMsg::SaveIdentity);
                }
                if f.saving {
                    ui.add(egui::Spinner::new());
                }
                if ui.add_enabled(!f.saving, egui::Button::new(t.tr("welcome.skip_for_now.button"))).clicked() {
                    send(msgs, OnboardingMsg::Skip);
                }
            }
            WelcomeStep::Connect if no_connections => {
                if ui.button(t.tr("welcome.skip_step.button")).clicked() {
                    send(msgs, OnboardingMsg::Skip);
                }
            }
            WelcomeStep::Connect | WelcomeStep::Git => {
                if ui.button(t.tr("welcome.continue.button")).clicked() {
                    send(msgs, OnboardingMsg::Next);
                }
            }
            WelcomeStep::Start => {
                if ui.button(t.tr("welcome.done.button")).clicked() {
                    send(msgs, OnboardingMsg::Finish(None));
                }
            }
        });
    });
}
