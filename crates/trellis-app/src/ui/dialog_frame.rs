//! The one frame every dialog is drawn in (`modals.rs`, the "Discard your changes?" question and
//! the credential prompt): a title with an optional weak subtitle, the body, and a footer whose
//! buttons always come in the same order, left to right:
//!
//! extras (Use folder name, Copy info) | gap | dismiss (Cancel or Close), 12 px, alternatives,
//! primary
//!
//! A dialog with no primary puts Close at the right edge. The width is Narrow (460 px) or Wide
//! (600 px), clamped to the window; the body scrolls instead of pushing the footer off screen,
//! and a resizable dialog's body sits in an `egui::Resize` with egui's grip at its bottom right.
//!
//! Keys: Enter with no modifiers runs the default button when a footer button has focus (egui
//! presses it), when nothing has focus, or when a single-line field gave up the focus on it or
//! reported it (the form's `submit`); Ctrl/Cmd+Enter runs it from anywhere, a multiline field
//! included. A field's Enter runs only a primary, never Cancel or Close. The default is the
//! primary, or the dismiss button when there is none or the dialog's tone is Danger, so Enter
//! never presses a danger button. On the first frame a dialog shows, its default button gets the
//! focus unless the body gave it to a field. Esc stays with `dialog_guard::dismissal`.

use super::theme::{color, controls};
use super::widgets::buttons::{self, StyledButton};
use super::widgets::{Icon, paint_icon};
use eframe::egui::{self, Id, Key, Modifiers, Response, RichText, Ui};
use std::borrow::Cow;
use trellis_core::i18n::Lang;

/// The standard widths of the content.
pub const NARROW: f32 = 460.0;
pub const WIDE: f32 = 600.0;
/// Space kept free between a dialog and the window's edges, and `theme::dialog`'s padding.
const MARGIN: f32 = 48.0;
const PAD: f32 = 18.0;
/// The least content width, however small the window.
const MIN_WIDTH: f32 = 320.0;
/// The least width of a footer button.
const FOOTER_BUTTON: f32 = 76.0;
/// Between the dismiss button and the actions.
const DISMISS_GAP: f32 = 12.0;
/// The space under the body taken by the footer before it was first measured.
const FOOTER_GUESS: f32 = 16.0 + 28.0;

/// What every dialog is drawn with: the language, whether it may take the keyboard (false under
/// a credential prompt or the discard question) and its key in egui's memory.
#[derive(Clone, Copy, Debug)]
pub struct Cx {
    pub t: Lang,
    pub keyboard: bool,
    pub key: Id,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    Narrow,
    Wide,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Normal,
    Danger,
}

/// A dialog about to be drawn: its title, width and whether its body resizes.
pub struct Dialog<'a> {
    cx: Cx,
    title: Cow<'a, str>,
    subtitle: Option<Cow<'a, str>>,
    icon: Option<Icon>,
    width: Width,
    resize: Option<&'static str>,
}

impl<'a> Dialog<'a> {
    pub fn new(cx: Cx, title: impl Into<Cow<'a, str>>) -> Self {
        Self { cx, title: title.into(), subtitle: None, icon: None, width: Width::Narrow, resize: None }
    }

    /// A painted icon before the title.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// 600 px wide instead of 460.
    pub fn wide(mut self) -> Self {
        self.width = Width::Wide;
        self
    }

    /// A weak line under the title.
    pub fn subtitle(mut self, s: impl Into<Cow<'a, str>>) -> Self {
        self.subtitle = Some(s.into());
        self
    }

    /// The body goes in a box the user can resize (long content), remembered under `salt`.
    pub fn resizable(mut self, salt: &'static str) -> Self {
        self.resize = Some(salt);
        self
    }

    /// Sets the width, reads this pass's keys and draws the title.
    pub fn begin(self, ui: &mut Ui) -> Open {
        let key = self.cx.key;
        let screen = ui.ctx().screen_rect().size();
        let room = (screen.x - MARGIN - 2.0 * PAD).max(MIN_WIDTH);
        let standard = match self.width {
            Width::Narrow => NARROW,
            Width::Wide => WIDE,
        };
        // A resizable dialog is as wide as its box was last frame.
        let width = self.resize.and_then(|_| ui.data(|d| d.get_temp::<f32>(key.with("dialog-width")))).unwrap_or(standard).min(room);
        ui.set_width(width);

        let keyboard = self.cx.keyboard;
        let focus_start = ui.memory(|m| m.focused());
        let popup = ui.memory(|m| m.any_popup_open());
        let enter = keyboard && !popup && ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers.is_none());
        // Taken before any field sees it, so a multiline field doesn't get a new line.
        let cmd_enter = keyboard && !popup && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));

        let now = ui.ctx().cumulative_pass_nr();
        let pass = key.with("dialog-pass");
        let opened = ui.data(|d| d.get_temp::<u64>(pass)).is_none_or(|last| last + 1 != now);
        ui.data_mut(|d| d.insert_temp(pass, now));
        if opened
            && keyboard
            && let Some(id) = focus_start
        {
            // The window beneath doesn't keep the keyboard.
            ui.memory_mut(|m| m.surrender_focus(id));
        }

        let top = ui.cursor().top();
        let title = RichText::new(self.title.as_ref()).size(17.0).strong().color(color::strong());
        match self.icon {
            None => {
                ui.label(title);
            }
            Some(icon) => {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                    paint_icon(ui.painter(), rect.center(), icon, color::text());
                    ui.label(title);
                });
            }
        }
        if let Some(s) = &self.subtitle {
            ui.label(RichText::new(s.as_ref()).color(color::weak()));
        }
        ui.add_space(10.0);
        let title_h = ui.cursor().top() - top;
        Open { cx: self.cx, standard, resize: self.resize, opened, focus_start, enter, cmd_enter, title_h, body_end: None }
    }
}

/// A dialog whose title is drawn; draw the body (optionally through `body`), then `finish`.
pub struct Open {
    cx: Cx,
    standard: f32,
    resize: Option<&'static str>,
    opened: bool,
    focus_start: Option<Id>,
    enter: bool,
    cmd_enter: bool,
    title_h: f32,
    body_end: Option<f32>,
}

impl Open {
    /// Draws `f` in the body's scroll area: as tall as its content up to what the window has
    /// room for, or, for a resizable dialog, filling the resizable box.
    pub fn body<R>(&mut self, ui: &mut Ui, f: impl FnOnce(&mut Ui) -> R) -> R {
        let key = self.cx.key;
        let screen = ui.ctx().screen_rect().size();
        let below = ui.data(|d| d.get_temp::<f32>(key.with("dialog-below"))).unwrap_or(FOOTER_GUESS);
        let cap = (screen.y - MARGIN - 2.0 * PAD - self.title_h - below).max(80.0);
        let scroll = egui::ScrollArea::vertical().id_salt(key.with("dialog-body"));
        let r = match self.resize {
            None => scroll.auto_shrink([false, true]).max_height(cap).show(ui, f).inner,
            Some(salt) => {
                let max = egui::vec2((screen.x - MARGIN - 2.0 * PAD).max(MIN_WIDTH), cap);
                let min = egui::vec2(NARROW.min(max.x), 200.0_f32.min(cap));
                let shown = ui.scope(|ui| {
                    egui::Resize::default()
                        .id_salt(key.with(salt))
                        .default_size([self.standard.min(max.x), 480.0])
                        .min_size(min)
                        .max_size(max)
                        .show(ui, |ui| scroll.auto_shrink([false, false]).show(ui, f).inner)
                });
                let w = shown.response.rect.width();
                ui.data_mut(|d| d.insert_temp(key.with("dialog-width"), w));
                shown.inner
            }
        };
        self.body_end = Some(ui.cursor().top());
        r
    }

    /// Draws the footer and works out what was pressed, by click or by key.
    pub fn finish(self, ui: &mut Ui, footer: Footer<'_>) -> Outcome {
        let start = self.body_end.unwrap_or_else(|| ui.cursor().top());
        let Footer { primary, alts, extras, note, dismiss, tone, submit } = footer;
        let t = self.cx.t;
        let spacing = ui.spacing().item_spacing.x;
        let mut out = Outcome::default();
        ui.add_space(16.0);
        let dismiss = dismiss.map(|d| buttons::secondary(d.text(t)).min_width(FOOTER_BUTTON));
        let alts: Vec<_> = alts.into_iter().map(|b| b.min_width(FOOTER_BUTTON)).collect();
        let primary = primary.map(|b| b.min_width(FOOTER_BUTTON));
        let primary_on = primary.as_ref().is_some_and(StyledButton::is_enabled);
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
        let (mut dismiss_r, mut primary_r): (Option<Response>, Option<Response>) = (None, None);
        ui.horizontal(|ui| {
            for (i, b) in extras.into_iter().enumerate() {
                if ui.add(b.min_width(FOOTER_BUTTON)).clicked() {
                    out.extra = Some(i);
                }
            }
            if let Some(n) = &note {
                ui.label(RichText::new(n.as_ref()).color(color::weak()));
            }
            let gap = ui.available_width() - right_w;
            if gap > 0.0 {
                ui.add_space(gap);
            }
            if let Some(d) = dismiss {
                let r = ui.add(d);
                out.dismiss = r.clicked();
                dismiss_r = Some(r);
                ui.add_space(gap_after_dismiss);
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
        let default_is_primary = tone == Tone::Normal && primary_r.is_some();
        let default_r = if default_is_primary { primary_r.as_ref() } else { dismiss_r.as_ref() };
        // A single-line field gives up the focus on Enter, and nothing else in a dialog does:
        // that Enter runs the primary as if the form reported it.
        let field_enter = self.enter && !self.opened && self.focus_start.is_some() && ui.memory(|m| m.focused()).is_none();
        if self.opened
            && self.cx.keyboard
            && ui.memory(|m| m.focused()).is_none()
            && let Some(r) = default_r
        {
            r.request_focus();
        }
        // A focused footer button was already pressed by egui's own Enter.
        let run_default =
            submit || self.cmd_enter || (self.enter && self.focus_start.is_none()) || (field_enter && default_is_primary);
        if run_default {
            if default_is_primary {
                out.primary |= primary_on;
            } else if dismiss_r.is_some() {
                out.dismiss = true;
            }
        }
        let below = ui.min_rect().bottom() - start;
        ui.data_mut(|d| d.insert_temp(self.cx.key.with("dialog-below"), below));
        out
    }
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
}

/// The acknowledgement above the footer of a step that loses work for good: a checkbox with
/// `label`, or `common.ack.check`. The danger button stays disabled until it is ticked.
pub fn ack(ui: &mut Ui, t: Lang, acked: &mut bool, label: Option<&str>) -> Response {
    ui.add_space(10.0);
    let text = label.unwrap_or(t.tr("common.ack.check"));
    controls::checkbox(ui, acked, RichText::new(text).color(color::red()))
}
