//! Help > Getting started, filling the central area: a header with how many features are in use
//! and a close button, then the feature cards under their group headings (state/getting_started.rs),
//! each with one sentence and a Show me button, and at the foot the switch that hides the page
//! from the Help menu. Cards already in use are left out unless asked for.

use super::theme::color;
use super::widgets::{self, Icon};
use crate::theme::controls;
use crate::theme::sizes::ControlSize;
use crate::theme::tokens::radius;
use crate::theme::type_scale;
use crate::widgets::buttons;
use eframe::egui::{self, Align, Layout, Margin, RichText, Stroke, Ui};
use glowfly_core::i18n::Lang;
use glowfly_model::msg::Msg;
use glowfly_model::onboarding::OnboardingMsg;
use glowfly_model::state::AppState;
use glowfly_model::state::onboarding::getting_started::{Card, groups, hidden, progress};

/// A card's padding, the gap between cards, and the least width that fits a card in a column.
const CARD_MARGIN: i8 = 14;
const GAP: f32 = 12.0;
const CARD_MIN_W: f32 = 260.0;
/// The most cards side by side.
const MAX_COLUMNS: usize = 3;

fn send(msgs: &mut Vec<Msg>, m: OnboardingMsg) {
    msgs.push(Msg::Onboarding(m));
}

pub(super) fn show(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let show_finished = st.onboarding.getting_started.as_ref().is_some_and(|p| p.show_finished);
    egui::Frame::new().inner_margin(Margin { left: 16, right: 16, top: 12, bottom: 4 }).show(ui, |ui| {
        let (pad, w) = super::dashboard::column(ui.available_width());
        let st: &AppState = st;
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            let w = w.min((ui.available_width() - pad).max(0.0));
            ui.horizontal_top(|ui| {
                ui.add_space(pad);
                ui.vertical(|ui| {
                    ui.set_width(w);
                    header(ui, st, t, show_finished, msgs);
                    for (group, cards) in groups(st, show_finished) {
                        ui.add_space(18.0);
                        ui.label(type_scale::section_text(ui, group.title(t)));
                        ui.add_space(6.0);
                        grid(ui, st, group as usize, &cards, w, msgs);
                    }
                    ui.add_space(20.0);
                    footer(ui, st, t, msgs);
                    ui.add_space(12.0);
                });
            });
        });
    });
}

/// The title and what the page is for, the close button, and the count of features in use with
/// the switch that lists their cards too.
fn header(ui: &mut Ui, st: &AppState, t: Lang, show_finished: bool, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(t.tr("getting_started.title")).font(type_scale::strong(ui, type_scale::TITLE)).color(color::strong()));
            ui.label(RichText::new(t.tr("getting_started.intro")).color(color::weak()));
        });
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            if widgets::icon_button(ui, Icon::Close, t.tr("common.close.button")).clicked() {
                send(msgs, OnboardingMsg::CloseGettingStarted);
            }
        });
    });
    ui.add_space(10.0);
    let (done, total) = progress(st);
    ui.horizontal(|ui| {
        ui.label(RichText::new(t.trf("getting_started.progress", &[("done", &done), ("total", &total)])).color(color::text()));
        if done > 0 {
            ui.add_space(12.0);
            let mut on = show_finished;
            if controls::checkbox(ui, &mut on, t.trf("getting_started.show_finished.check", &[("n", &done)])).changed() {
                send(msgs, OnboardingMsg::ShowFinished(on));
            }
        }
    });
}

/// The group's cards, as many side by side as `w` fits (at most `MAX_COLUMNS`). The cards of a
/// row share one height: each pads its texts to the tallest texts of the row (measured the
/// frame before), so their buttons line up.
fn grid(ui: &mut Ui, st: &AppState, group: usize, cards: &[(Card, bool)], w: f32, msgs: &mut Vec<Msg>) {
    let columns = (((w + GAP) / (CARD_MIN_W + GAP)).floor() as usize).clamp(1, MAX_COLUMNS);
    let card_w = (w - GAP * (columns - 1) as f32) / columns as f32;
    for (i, row) in cards.chunks(columns).enumerate() {
        let id = ui.make_persistent_id(("getting-started-row", group, i, columns));
        let texts_h: f32 = ui.data(|d| d.get_temp(id)).unwrap_or(0.0);
        let mut tallest = 0.0_f32;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for &(card, done) in row {
                tallest = tallest.max(card_view(ui, st, (card, done), card_w, texts_h, msgs));
            }
        });
        if (tallest - texts_h).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(id, tallest));
            ui.ctx().request_repaint();
        }
        ui.add_space(GAP - ui.spacing().item_spacing.y);
    }
}

/// One card `w` wide: its title, its sentence (padded to `texts_h`), then Show me (disabled
/// with the reason when it can't show itself now) and, for a feature in use, a check and "In
/// use". Returns the height of its texts.
fn card_view(ui: &mut Ui, st: &AppState, (card, done): (Card, bool), w: f32, texts_h: f32, msgs: &mut Vec<Msg>) -> f32 {
    let t = st.settings.lang();
    let (title, body) = card.text(t);
    let inner = w - 2.0 * f32::from(CARD_MARGIN) - 2.0;
    let frame = egui::Frame::new()
        .fill(color::raised())
        .stroke(Stroke::new(1.0_f32, color::border()))
        .corner_radius(radius::LG)
        .inner_margin(Margin::same(CARD_MARGIN));
    frame
        .show(ui, |ui| {
            ui.set_width(inner);
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                let top = ui.cursor().top();
                ui.label(RichText::new(title).font(type_scale::strong(ui, type_scale::BODY)).color(color::strong()));
                ui.add(egui::Label::new(RichText::new(body).size(type_scale::SMALL).color(color::weak())).wrap());
                let own = ui.cursor().top() - top;
                ui.add_space(8.0 + (texts_h - own).max(0.0));
                ui.horizontal(|ui| {
                    let action = card.show_me(st);
                    let button = buttons::secondary(t.tr("getting_started.show_me.button"))
                        .size(ControlSize::Small)
                        .enabled(action.is_ok())
                        .why(action.err());
                    if ui.add(button).clicked() {
                        send(msgs, OnboardingMsg::ShowMe(card));
                    }
                    if done {
                        ui.add_space(8.0);
                        in_use(ui, t);
                    }
                });
                own
            })
            .inner
        })
        .inner
}

/// The check and "In use" beside a finished card's button.
fn in_use(ui: &mut Ui, t: Lang) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
    widgets::paint_icon(ui.painter(), rect.center(), Icon::Check, color::added());
    ui.label(RichText::new(t.tr("getting_started.done")).size(type_scale::SMALL).color(color::added()));
}

/// Hide the page from the Help menu, or show it there again.
fn footer(ui: &mut Ui, st: &AppState, t: Lang, msgs: &mut Vec<Msg>) {
    let is_hidden = hidden(&st.settings);
    let label = if is_hidden { t.tr("getting_started.unhide.button") } else { t.tr("getting_started.hide.button") };
    if ui.add(buttons::quiet(label).size(ControlSize::Small)).clicked() {
        send(msgs, OnboardingMsg::HideGettingStarted(!is_hidden));
    }
}
