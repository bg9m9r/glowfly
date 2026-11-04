//! The block under an expanded empty left-panel section: one sentence saying what the feature
//! is for (`weak`, small, wrapping to the panel) and its action as a Regular button below it
//! ("Add worktree…"), or two side by side that wrap in a narrow panel (Publish… and a quiet
//! Add remote…). A blocked action keeps the disabled look with its reason on hover. A button is
//! never wider than the panel's room. A hosting service that failed (`failure`) is laid out the
//! same way: the needs-you badge and the cause, then Sign in.

use super::buttons;
use super::{PAD_X, ROW_INDENT};
use crate::theme::{color, type_scale};
use eframe::egui::{self, RichText};
use glowfly_core::i18n::Lang;
use glowfly_model::msg::{ForgeMsg, Msg, PrefsMsg};
use glowfly_model::state::PrefPage;
use glowfly_model::state::section_actions::SectionAction;

/// Above the sentence.
const TOP: i8 = 6;
/// Between the sentence and the buttons.
const ABOVE_BUTTONS: f32 = 8.0;
/// Below the buttons, before the next section's divider.
const BOTTOM: i8 = 12;
/// Between two buttons.
const GAP: f32 = 8.0;

/// One of the block's buttons.
pub struct Action<'a> {
    pub label: String,
    /// Why it can't run, shown on hover over its disabled look.
    pub blocked: Option<&'a str>,
    pub msg: Msg,
    /// A quiet button (the second, lesser action) rather than a secondary one.
    pub quiet: bool,
}

impl<'a> Action<'a> {
    /// A section's create action as a secondary button ("Add worktree…").
    pub fn of(a: &'a SectionAction) -> Self {
        Self { label: a.label(), blocked: a.blocked.as_deref(), msg: a.msg(), quiet: false }
    }

    /// Connect…, opening Preferences > Integrations (a hosting service not connected yet).
    pub fn connect(t: Lang) -> Self {
        let msg = Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Integrations));
        Self { label: t.tr("status.connect").to_owned(), blocked: None, msg, quiet: false }
    }

    /// The same as a quiet button.
    pub fn quiet(self) -> Self {
        Self { quiet: true, ..self }
    }
}

/// The block's frame, ending `BOTTOM` below its last button (`TOP` without one).
fn frame(buttons: bool) -> egui::Frame {
    let bottom = if buttons { BOTTOM } else { TOP };
    egui::Frame::new().inner_margin(egui::Margin { left: ROW_INDENT as i8, right: PAD_X as i8, top: TOP, bottom })
}

/// A service that failed with `cause` although credentials were found: the needs-you badge and
/// the cause in `text`, then, with `sign_in`, a Sign in that asks again interactively (left out
/// where the section above already offers one for the same account).
pub fn failure(ui: &mut egui::Ui, t: Lang, cause: &str, sign_in: bool, msgs: &mut Vec<Msg>) {
    frame(sign_in).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
            crate::needs_you::badge(ui.painter(), rect);
            ui.add(egui::Label::new(RichText::new(cause).color(color::text()).size(type_scale::SMALL)).wrap());
        });
        if sign_in {
            ui.add_space(ABOVE_BUTTONS);
            if ui.add(buttons::secondary(t.tr("status.sign_in"))).clicked() {
                msgs.push(Msg::Forge(ForgeMsg::ForgeRefresh { interactive: true }));
            }
        }
    });
}

/// Draws `text` and its `actions` (see the module doc).
pub fn show(ui: &mut egui::Ui, text: &str, actions: Vec<Action>, msgs: &mut Vec<Msg>) {
    frame(!actions.is_empty()).show(ui, |ui| {
        ui.add(egui::Label::new(RichText::new(text).size(type_scale::SMALL).color(color::weak())).wrap().selectable(false));
        if actions.is_empty() {
            return;
        }
        ui.add_space(ABOVE_BUTTONS);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(GAP, GAP);
            // Never wider than the panel: in the narrowest panel a long label is cut rather than
            // pushing the section headers' counts out of view.
            let room = ui.available_width();
            for a in actions {
                let b = if a.quiet { buttons::quiet(a.label.as_str()) } else { buttons::secondary(a.label.as_str()) };
                if ui.add(b.enabled(a.blocked.is_none()).why(a.blocked).max_width(room)).clicked() && a.blocked.is_none() {
                    msgs.push(a.msg);
                }
            }
        });
    });
}
