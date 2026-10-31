//! A dialog's footer (`dialog_frame`): its buttons, built with `Footer`, and their layout 24 px under
//! the body, in the fixed order, left to right:
//!
//! extras | note | gap | dismiss, 12 px, alternatives, primary
//!
//! A disabled primary or danger button with a `why` says it on screen too: one weak line
//! (`widgets::reason`) right-aligned in the gap, 12 px before the dismiss button, cut with "…"
//! when long. When the gap has less than 140 px free (extras and a note take the room), the line
//! goes on its own row just above the buttons instead. Only the room decides, never the text's
//! length, so typing doesn't move it. Alternatives and extras keep hover-only reasons.

use super::Tone;
use crate::ui::theme::color;
use crate::ui::widgets::buttons::{self, StyledButton};
use crate::ui::widgets::reason;
use eframe::egui::{Color32, Response, RichText, TextStyle, Ui};
use glowfly_core::i18n::Lang;
use std::borrow::Cow;

/// The least width of a footer button.
const FOOTER_BUTTON: f32 = 76.0;
/// Between the dismiss button and the actions.
const DISMISS_GAP: f32 = 12.0;

/// What the footer's row did: the buttons' outcome, the responses the keys need, and how the
/// footer was set up.
pub(super) struct Laid {
    pub out: Outcome,
    pub dismiss: Option<Response>,
    pub primary: Option<Response>,
    pub primary_on: bool,
    pub tone: Tone,
    pub submit: bool,
}

/// Draws `footer`'s row (and its reason line), in `t`. `exact` (a dialog with a set width) leaves
/// out the empty space after a lone dismiss button, which would reach past the content's right
/// edge and widen the dialog by the item spacing.
pub(super) fn lay_out(ui: &mut Ui, footer: Footer<'_>, t: Lang, exact: bool) -> Laid {
    let Footer { primary, alts, extras, note, dismiss, tone, submit } = footer;
    let spacing = ui.spacing().item_spacing.x;
    let mut out = Outcome::default();
    // 24 px between the body and the buttons.
    ui.add_space(24.0);
    let dismiss = dismiss.map(|d| buttons::secondary(d.text(t)).min_width(FOOTER_BUTTON));
    let alts: Vec<_> = alts.into_iter().map(|b| b.min_width(FOOTER_BUTTON)).collect();
    let extras: Vec<_> = extras.into_iter().map(|b| b.min_width(FOOTER_BUTTON)).collect();
    let primary = primary.map(|b| b.min_width(FOOTER_BUTTON));
    let primary_on = primary.as_ref().is_some_and(StyledButton::is_enabled);
    let reason = primary.as_ref().and_then(StyledButton::blocked);
    // The width of the right-hand group, to push it against the edge.
    let mut right: Vec<f32> = Vec::new();
    let mut gap_after_dismiss = 0.0;
    if let Some(d) = &dismiss {
        right.push(d.desired_width(ui));
        if !alts.is_empty() || primary.is_some() {
            gap_after_dismiss = (DISMISS_GAP - spacing).max(0.0);
        }
    }
    right.extend(alts.iter().map(|b| b.desired_width(ui)));
    right.extend(primary.iter().map(|b| b.desired_width(ui)));
    let right_w = right.iter().sum::<f32>() + spacing * right.len().saturating_sub(1) as f32 + gap_after_dismiss;
    // The left-hand group's, each item followed by the spacing: what the row has free.
    let font = TextStyle::Body.resolve(ui.style());
    let note_w = note.as_ref().map(|n| ui.fonts(|f| f.layout_no_wrap(n.to_string(), font, Color32::PLACEHOLDER).size().x));
    let left: Vec<f32> = extras.iter().map(|b| b.desired_width(ui)).chain(note_w).collect();
    let room = ui.available_width() - right_w - left.iter().map(|w| w + spacing).sum::<f32>();
    let in_row = reason.filter(|_| reason::fits_in_row(room));
    if let (Some(why), None) = (reason, in_row) {
        reason::above(ui, why);
    }
    let (mut dismiss_r, mut primary_r): (Option<Response>, Option<Response>) = (None, None);
    ui.horizontal(|ui| {
        for (i, b) in extras.into_iter().enumerate() {
            if ui.add(b).clicked() {
                out.extra = Some(i);
            }
        }
        if let Some(n) = &note {
            ui.label(RichText::new(n.as_ref()).color(color::weak()));
        }
        let gap = ui.available_width() - right_w;
        if let Some(why) = in_row {
            reason::in_row(ui, why, gap);
        } else if gap > 0.0 {
            ui.add_space(gap);
        }
        if let Some(d) = dismiss {
            let r = ui.add(d);
            out.dismiss = r.clicked();
            dismiss_r = Some(r);
            if gap_after_dismiss > 0.0 || !exact {
                ui.add_space(gap_after_dismiss);
            }
        }
        for (i, b) in alts.into_iter().enumerate() {
            if ui.add(b).clicked() {
                out.alt = Some(i);
            }
        }
        if let Some(b) = primary {
            let r = ui.add(b);
            out.primary = r.clicked();
            primary_r = Some(r);
        }
    });
    Laid { out, dismiss: dismiss_r, primary: primary_r, primary_on, tone, submit }
}

/// The dismiss button's label.
enum Dismiss<'a> {
    Cancel,
    Close,
    Text(Cow<'a, str>),
}

impl<'a> Dismiss<'a> {
    fn text(self, t: Lang) -> Cow<'a, str> {
        match self {
            Dismiss::Cancel => t.tr("common.cancel.button").into(),
            Dismiss::Close => t.tr("common.close.button").into(),
            Dismiss::Text(s) => s,
        }
    }
}

/// A dialog's buttons; `Open::finish` lays them out in the fixed order.
#[derive(Default)]
pub struct Footer<'a> {
    primary: Option<StyledButton<'a>>,
    alts: Vec<StyledButton<'a>>,
    extras: Vec<StyledButton<'a>>,
    note: Option<Cow<'a, str>>,
    dismiss: Option<Dismiss<'a>>,
    tone: Tone,
    submit: bool,
}

impl<'a> Footer<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// The main action, at the right edge; the default button.
    pub fn primary(mut self, b: StyledButton<'a>) -> Self {
        self.primary = Some(b);
        self
    }

    /// A step that loses work, at the right edge. The dialog's tone becomes Danger: Cancel is
    /// the default, so no key presses it.
    pub fn danger(mut self, b: StyledButton<'a>) -> Self {
        self.primary = Some(b);
        self.tone = Tone::Danger;
        self
    }

    /// Another action, between the dismiss button and the primary.
    pub fn alt(mut self, b: StyledButton<'a>) -> Self {
        self.alts.push(b);
        self
    }

    /// An action at the left edge.
    pub fn extra(mut self, b: StyledButton<'a>) -> Self {
        self.extras.push(b);
        self
    }

    /// A weak note at the left edge, after the extras.
    pub fn note(mut self, text: impl Into<Cow<'a, str>>) -> Self {
        self.note = Some(text.into());
        self
    }

    pub fn cancel(mut self) -> Self {
        self.dismiss = Some(Dismiss::Cancel);
        self
    }

    pub fn close(mut self) -> Self {
        self.dismiss = Some(Dismiss::Close);
        self
    }

    /// A dismiss button with its own label.
    pub fn close_label(mut self, text: impl Into<Cow<'a, str>>) -> Self {
        self.dismiss = Some(Dismiss::Text(text.into()));
        self
    }

    /// Whether a single-line field reported Enter this frame (runs the default button).
    pub fn submit(mut self, submit: bool) -> Self {
        self.submit = submit;
        self
    }
}

/// What the footer's buttons did this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub primary: bool,
    pub dismiss: bool,
    pub alt: Option<usize>,
    pub extra: Option<usize>,
    /// Enter asked for the primary while it was disabled: the form shows every field's problem
    /// (`state::touched::Touched::attempt`).
    pub attempted: bool,
}
