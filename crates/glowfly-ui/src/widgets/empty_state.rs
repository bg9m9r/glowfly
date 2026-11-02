//! The block under an expanded empty left-panel section: one sentence saying what the feature
//! is for (`weak`, small, wrapping to the panel) and its action as a Regular button below it
//! ("Add worktree…"), or two side by side that wrap in a narrow panel (Publish… and a quiet
//! Add remote…). A blocked action keeps the disabled look with its reason on hover. A button is
//! never wider than the panel's room.

use super::buttons;
use super::{PAD_X, ROW_INDENT};
use crate::msg::{Msg, PrefsMsg};
use crate::state::PrefPage;
use crate::state::section_actions::SectionAction;
use crate::ui::theme::{color, type_scale};
use eframe::egui::{self, RichText};
use glowfly_core::i18n::Lang;

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

/// Draws `text` and its `actions` (see the module doc).
pub fn show(ui: &mut egui::Ui, text: &str, actions: Vec<Action>, msgs: &mut Vec<Msg>) {
    let bottom = if actions.is_empty() { TOP } else { BOTTOM };
    let margin = egui::Margin { left: ROW_INDENT as i8, right: PAD_X as i8, top: TOP, bottom };
    egui::Frame::new().inner_margin(margin).show(ui, |ui| {
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
