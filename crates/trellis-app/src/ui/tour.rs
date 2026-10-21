//! The tour of the main window: a translucent scrim over everything but the step's area, an
//! accent outline round that area and a small card beside it. Painting only, so clicks reach
//! the window underneath; hidden while Preferences, the palette, a dialog, a credential prompt
//! or the welcome is open (the step is kept and the tour reappears afterwards).

use super::theme::color;
use super::widgets::{self, Icon};
use crate::keymap::KeyAction;
use crate::msg::{LayoutMsg, Msg, PaletteMsg};
use crate::onboarding::OnboardingMsg;
use crate::state::AppState;
use crate::state::layout::{self, Panel};
use crate::state::onboarding_tour::{NO_REPOSITORY, PANEL_HIDDEN, TourStep};
use crate::ui::theme::tokens::radius;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, Id, LayerId, Layout, Order, Rect, RichText, Stroke, StrokeKind, pos2, vec2,
};

const CARD_WIDTH: f32 = 320.0;
const GAP: f32 = 12.0;

/// Why the card can't point at its area.
#[derive(Clone, Copy, PartialEq)]
enum Missing {
    /// The active tab has no repository.
    Repository,
    /// The step's panel is turned off.
    Panel(Panel),
}

pub fn show(ctx: &egui::Context, st: &AppState, central: Rect, msgs: &mut Vec<Msg>) {
    let Some(step) = st.onboarding.tour else { return };
    if st.prefs.is_some() || st.palette.is_some() || st.modal.is_some() || st.askpass.showing() || st.onboarding.welcome.is_some() {
        return;
    }
    let missing = match step.panel() {
        Some(p) if !layout::shown(&st.settings, p) => Some(Missing::Panel(p)),
        _ if st.snap.is_none() && step != TourStep::Palette => Some(Missing::Repository),
        _ => None,
    };
    let target = match (missing, step) {
        (Some(_), _) | (None, TourStep::Palette) => None,
        (None, TourStep::Graph) => Some(central),
        (None, TourStep::Toolbar) => panel_rect(ctx, "toolbar"),
        (None, TourStep::LeftPanel) => panel_rect(ctx, "refs"),
        (None, TourStep::Details) => panel_rect(ctx, "detail"),
    };
    let screen = ctx.screen_rect();
    scrim(ctx, screen, target);
    let area = egui::Area::new(Id::new("tour-card")).order(Order::Foreground).constrain_to(screen.shrink(8.0));
    let area = match target {
        None => area.anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)),
        Some(r) => {
            let (pos, pivot) = place(step, r);
            area.fixed_pos(pos).pivot(pivot)
        }
    };
    area.show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
            ui.set_width(CARD_WIDTH);
            card(ui, st, step, missing, msgs);
        });
    });
}

/// Where a panel was drawn last, by the id it is shown with.
fn panel_rect(ctx: &egui::Context, id: &str) -> Option<Rect> {
    egui::containers::panel::PanelState::load(ctx, Id::new(id)).map(|s| s.rect)
}

/// The card's corner and the corner of it that sits there: below the toolbar, right of the left
/// panel, left of the details panel, near the top of the graph.
fn place(step: TourStep, r: Rect) -> (egui::Pos2, Align2) {
    match step {
        TourStep::Toolbar => (pos2(r.center().x, r.bottom() + GAP), Align2::CENTER_TOP),
        TourStep::LeftPanel => (pos2(r.right() + GAP, r.top() + 48.0), Align2::LEFT_TOP),
        TourStep::Details => (pos2(r.left() - GAP, r.top() + 48.0), Align2::RIGHT_TOP),
        TourStep::Graph | TourStep::Palette => (pos2(r.center().x, r.top() + 72.0), Align2::CENTER_TOP),
    }
}

/// Dims everything but `target` and outlines it. Its own layer above the panels and below the
/// card; nothing on it senses input.
fn scrim(ctx: &egui::Context, screen: Rect, target: Option<Rect>) {
    let p = ctx.layer_painter(LayerId::new(Order::Middle, Id::new("tour-scrim")));
    let dim = Color32::from_black_alpha(120);
    let Some(t) = target.map(|t| t.intersect(screen)) else {
        p.rect_filled(screen, CornerRadius::ZERO, dim);
        return;
    };
    p.rect_filled(Rect::from_min_max(screen.min, pos2(screen.max.x, t.top())), CornerRadius::ZERO, dim);
    p.rect_filled(Rect::from_min_max(pos2(screen.min.x, t.bottom()), screen.max), CornerRadius::ZERO, dim);
    p.rect_filled(Rect::from_min_max(pos2(screen.min.x, t.top()), pos2(t.left(), t.bottom())), CornerRadius::ZERO, dim);
    p.rect_filled(Rect::from_min_max(pos2(t.right(), t.top()), pos2(screen.max.x, t.bottom())), CornerRadius::ZERO, dim);
    p.rect_stroke(t.shrink(1.0), CornerRadius::same(radius::MD), Stroke::new(2.0_f32, color::accent()), StrokeKind::Inside);
}

/// The card: title and close, the text, the step's own buttons, then where the tour is and
/// Back / Next.
fn card(ui: &mut egui::Ui, st: &AppState, step: TourStep, missing: Option<Missing>, msgs: &mut Vec<Msg>) {
    let send = |msgs: &mut Vec<Msg>, m: OnboardingMsg| msgs.push(Msg::Onboarding(m));
    ui.horizontal(|ui| {
        ui.label(RichText::new(step.title()).font(type_scale::strong(ui, type_scale::SUBHEAD)).color(color::strong()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::small_icon(ui, Icon::Close, "Close the tour", color::weak()).clicked() {
                send(msgs, OnboardingMsg::CloseTour);
            }
        });
    });
    ui.add_space(4.0);
    match missing {
        Some(Missing::Repository) => {
            ui.add(egui::Label::new(RichText::new(NO_REPOSITORY).color(color::text())).wrap());
        }
        Some(Missing::Panel(panel)) => {
            ui.add(egui::Label::new(RichText::new(step.body(&st.keymap)).color(color::text())).wrap());
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(PANEL_HIDDEN).color(color::weak()));
                if ui.add(buttons::secondary("Show it")).clicked() {
                    msgs.push(Msg::Layout(LayoutMsg::SetPanel { panel, shown: true }));
                }
            });
        }
        None => {
            ui.add(egui::Label::new(RichText::new(step.body(&st.keymap)).color(color::text())).wrap());
        }
    }
    if step == TourStep::Palette {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.add(buttons::secondary("Open the palette")).clicked() {
                msgs.push(Msg::Palette(PaletteMsg::TogglePalette));
            }
            if ui.add(buttons::secondary("Show shortcuts")).clicked() {
                msgs.push(Msg::Shortcut(KeyAction::ToggleSheet));
            }
        });
    }
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{} of {}", step.number(), TourStep::ALL.len())).color(color::weak()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let next = if step.next().is_some() { "Next" } else { "Done" };
            if ui.add(buttons::primary(next)).clicked() {
                send(msgs, OnboardingMsg::TourNext);
            }
            if step.prev().is_some() && ui.add(buttons::secondary("Back")).clicked() {
                send(msgs, OnboardingMsg::TourBack);
            }
        });
    });
}
