//! The first-run welcome: a dialog with four steps (who you are, a hosting service, git, where
//! to start). Drawn before the other dialogs, so a prompt or dialog opened over it stacks on top;
//! not drawn while Preferences is open (it waits behind, on the same step). The header band and
//! the progress bar are in `welcome_header.rs` (the band's lattice drawing in `welcome_art.rs`),
//! the step bodies in `welcome_steps.rs` and the Start step's choice cards in `welcome_cards.rs`.

use super::theme::color;
use super::welcome_header::{self, BAND_H};
use super::welcome_steps;
use crate::msg::Msg;
use crate::onboarding::OnboardingMsg;
use crate::state::AppState;
use crate::state::onboarding::{Welcome, WelcomeStep};
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use eframe::egui::{self, Align, Id, Layout, Margin, RichText, Stroke};
use trellis_core::i18n::Lang;

/// The dialog's width, kept `EDGE` clear of the window's sides, but never under `MIN_W`.
const WIDTH: f32 = 760.0;
const EDGE: f32 = 16.0;
const MIN_W: f32 = 360.0;
/// Everything under the band is padded this much at the sides, and the body's gaps.
const PAD_X: i8 = 24;
const GAP: f32 = 20.0;
const TITLE_GAP: f32 = 8.0;
/// The body's least height, so the dialog doesn't jump between steps.
const BODY_MIN_H: f32 = 200.0;
/// What a short window keeps around the dialog, and the footer row's height.
const SCREEN_MARGIN: f32 = 32.0;
const FOOTER_H: f32 = 32.0;

pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    if st.prefs.is_some() {
        return;
    }
    let prompting = st.askpass.showing();
    let has_repo = st.snap.is_some();
    let (settings, git) = (&st.settings, &st.git);
    let t = settings.lang();
    let Some(w) = st.onboarding.welcome.as_mut() else { return };
    let screen = ctx.screen_rect();
    let width = WIDTH.min(screen.width() - 2.0 * EDGE).max(MIN_W);
    let frame =
        egui::Frame::popup(&ctx.style()).stroke(Stroke::new(1.0_f32, color::dialog_border())).inner_margin(Margin::ZERO).corner_radius(8);
    let resp = super::theme::modal(ctx, Id::new("trellis-welcome")).frame(frame).show(ctx, |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = 0.0;
        welcome_header::band(ui, t, w, msgs);
        let margin = Margin { left: PAD_X, right: PAD_X, top: GAP as i8, bottom: GAP as i8 };
        egui::Frame::new().inner_margin(margin).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = super::theme::spacing::ITEM_SPACING.y;
            welcome_header::progress(ui, t, w.step);
            ui.add_space(GAP);
            let title_font = type_scale::strong(ui, type_scale::SUBHEAD);
            let title_h = ui.fonts(|f| f.row_height(&title_font));
            ui.label(RichText::new(w.step.title(t)).font(title_font).color(color::strong()));
            ui.add_space(TITLE_GAP);
            let chrome = BAND_H + 4.0 * GAP + welcome_header::progress_height(ui) + title_h + TITLE_GAP + FOOTER_H;
            let max_body = (screen.height() - SCREEN_MARGIN - chrome).max(48.0);
            egui::ScrollArea::vertical().id_salt("welcome-body").auto_shrink([false, true]).max_height(max_body).show(ui, |ui| {
                ui.set_min_height(BODY_MIN_H.min(max_body));
                match w.step {
                    WelcomeStep::Identity => welcome_steps::identity(ui, w, settings, !prompting),
                    WelcomeStep::Connect => welcome_steps::connect(ui, t, &settings.connections, msgs),
                    WelcomeStep::Git => welcome_steps::git(ui, t, git, msgs),
                    WelcomeStep::Start => welcome_steps::start(ui, t, w, has_repo, msgs),
                }
            });
            ui.add_space(GAP);
            footer(ui, t, w, settings.connections.is_empty(), msgs);
        });
    });
    // Esc or a click outside ends the welcome early, unless a credential prompt is on top.
    if resp.should_close() && !prompting {
        msgs.push(Msg::Onboarding(OnboardingMsg::CloseWelcome));
    }
}

/// Back on the left (not on the first step), the step's own buttons on the right.
fn footer(ui: &mut egui::Ui, t: Lang, w: &Welcome, no_connections: bool, msgs: &mut Vec<Msg>) {
    let send = |msgs: &mut Vec<Msg>, m: OnboardingMsg| msgs.push(Msg::Onboarding(m));
    ui.horizontal(|ui| {
        let busy = w.identity.saving;
        if w.step.prev().is_some() && ui.add(buttons::secondary(t.tr("welcome.back.button")).enabled(!busy)).clicked() {
            send(msgs, OnboardingMsg::Back);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| match w.step {
            WelcomeStep::Identity => {
                let f = &w.identity;
                let ready = !f.saving && !f.reading;
                if ui.add(buttons::primary(t.tr("welcome.continue.button")).enabled(ready)).clicked() {
                    send(msgs, OnboardingMsg::SaveIdentity);
                }
                if f.saving {
                    ui.add(egui::Spinner::new());
                }
                if ui.add(buttons::secondary(t.tr("welcome.skip_for_now.button")).enabled(!f.saving)).clicked() {
                    send(msgs, OnboardingMsg::Skip);
                }
            }
            WelcomeStep::Connect if no_connections => {
                if ui.add(buttons::secondary(t.tr("welcome.skip_step.button"))).clicked() {
                    send(msgs, OnboardingMsg::Skip);
                }
            }
            WelcomeStep::Connect | WelcomeStep::Git => {
                if ui.add(buttons::primary(t.tr("welcome.continue.button"))).clicked() {
                    send(msgs, OnboardingMsg::Next);
                }
            }
            WelcomeStep::Start => {
                // The cards are the step's main actions.
                if ui.add(buttons::secondary(t.tr("welcome.done.button"))).clicked() {
                    send(msgs, OnboardingMsg::Finish(None));
                }
            }
        });
    });
}
