//! The one frame every dialog is drawn in (`modals.rs`, the "Discard your changes?" question and
//! the credential prompt): a title with an optional weak subtitle, the body, and a footer whose
//! buttons always come in the same order, left to right:
//!
//! extras (Use folder name, Copy info) | gap | dismiss (Cancel or Close), 12 px, alternatives,
//! primary
//!
//! A dialog with no primary puts Close at the right edge. A disabled primary or danger button
//! says why in a weak line before the dismiss button, or just above the row when it has no room
//! (`dialog_footer.rs`, which holds the footer and its layout). The header (`dialog_header.rs`)
//! holds the title and subtitle and, at its right end, a painted close button that does what Esc
//! does (`take_closed`). The width comes from the size class (`dialog_size.rs`: Small, Medium or
//! Wide), following the title row or subtitle between the class's base and most, clamped to the
//! window; the body scrolls instead of pushing the footer off screen, and a resizable dialog's
//! body sits in an `egui::Resize` with egui's grip at its bottom right. A dialog for a less
//! familiar operation has a tinted side pane that explains it in steps (`dialog_guide.rs`, the
//! texts in `dialog_guides.rs`), beside the form or, in a narrow window, above it. A dialog with
//! a look of its own (the first-run welcome) draws its header itself (`Dialog::header`, placing
//! the shared close box with `close_button`), sets its content width (`Dialog::width`) and keeps
//! its body a least height (`Dialog::min_body`).
//!
//! Keys: Enter with no modifiers runs the default button when a footer button has focus (egui
//! presses it), when nothing has focus, or when a single-line field gave up the focus on it or
//! reported it (the form's `submit`); Ctrl/Cmd+Enter runs it from anywhere, a multiline field
//! included. A field's Enter runs only a primary, never Cancel or Close. The default is the
//! primary, or the dismiss button when there is none or the dialog's tone is Danger, so Enter
//! never presses a danger button. On the first frame a dialog shows, its default button gets the
//! focus unless the body gave it to a field. Esc stays with `dialog_guard::dismissal`.

use super::theme::{DIALOG_PAD, color, controls};
use super::widgets::Icon;
use eframe::egui::{self, Id, Key, Modifiers, Rect, Response, RichText, Ui, pos2, vec2};
use std::borrow::Cow;
use trellis_core::i18n::Lang;

#[path = "dialog_footer.rs"]
mod footer;
#[path = "dialog_guide.rs"]
mod guide;
#[path = "dialog_header.rs"]
mod header;
#[path = "dialog_size.rs"]
mod size;
pub use footer::{Footer, Outcome};
pub use guide::Guide;
pub use header::{close_button, take_closed};
use size::MIN_WIDTH;
pub use size::Size;

/// Space kept free between a dialog and the window's edges, and `theme::dialog`'s padding.
const MARGIN: f32 = 48.0;
const PAD: f32 = DIALOG_PAD as f32;
/// Between the header and the body.
const HEADER_GAP: f32 = 16.0;
/// The space under the body taken by the footer before it was first measured.
const FOOTER_GUESS: f32 = 24.0 + 32.0;

/// What every dialog is drawn with: the language, whether it may take the keyboard (false under
/// a credential prompt or the discard question) and its key in egui's memory.
#[derive(Clone, Copy, Debug)]
pub struct Cx {
    pub t: Lang,
    pub keyboard: bool,
    pub key: Id,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Normal,
    Danger,
}

/// A header the caller draws instead of the title row (`Dialog::header`).
type DrawHeader<'a> = Box<dyn FnOnce(&mut Ui) + 'a>;

/// A dialog about to be drawn: its title, size class, side pane and whether its body resizes.
pub struct Dialog<'a> {
    custom: Option<DrawHeader<'a>>,
    fixed_w: Option<f32>,
    min_body: Option<f32>,
    cx: Cx,
    title: Cow<'a, str>,
    subtitle: Option<Cow<'a, str>>,
    icon: Option<Icon>,
    size: Size,
    guide: Option<Guide>,
    resize: Option<&'static str>,
}

impl<'a> Dialog<'a> {
    pub fn new(cx: Cx, title: impl Into<Cow<'a, str>>) -> Self {
        Self {
            custom: None,
            fixed_w: None,
            min_body: None,
            cx,
            title: title.into(),
            subtitle: None,
            icon: None,
            size: Size::Medium,
            guide: None,
            resize: None,
        }
    }

    /// Draws the header in place of the title row and close button (the welcome's band,
    /// progress bar and step title). `draw` runs at the content's top left once the width is
    /// set. It may paint into the frame's padding (`DIALOG_PAD`) for a band across the dialog,
    /// and puts its own close button with `close_button`, so `take_closed` works as for any
    /// dialog.
    pub fn header(mut self, draw: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.custom = Some(Box::new(draw));
        self
    }

    /// A set content width, clamped to the window, instead of the size class's range.
    pub fn width(mut self, w: f32) -> Self {
        self.fixed_w = Some(w);
        self
    }

    /// The body's least height, clamped to the room, so a dialog whose steps differ in length
    /// doesn't jump between them.
    pub fn min_body(mut self, h: f32) -> Self {
        self.min_body = Some(h);
        self
    }

    /// A painted icon before the title.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The size class (Medium unless set).
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// A side pane that explains a less familiar operation in steps. A resizable dialog
    /// ignores it.
    pub fn guide(mut self, guide: Guide) -> Self {
        self.guide = Some(guide);
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

    /// Sets the width, reads this pass's keys and draws the header (and a stacked side pane).
    pub fn begin(self, ui: &mut Ui) -> Open {
        let key = self.cx.key;
        let t = self.cx.t;
        let screen = ui.ctx().screen_rect().size();
        let room = (screen.x - MARGIN - 2.0 * PAD).max(MIN_WIDTH);
        debug_assert!(!(self.resize.is_some() && self.guide.is_some()), "a resizable dialog has no side pane");
        let guide = self.guide.filter(|_| self.resize.is_none()).map(|g| (g, guide::place(self.size.base(), room)));
        let beside = matches!(guide, Some((_, guide::Placement::Beside)));
        let room_for_form = if beside { room - guide::GAP - guide::PANE_W } else { room };
        let need = if self.custom.is_some() { 0.0 } else { header::need(ui, &self.title, self.subtitle.as_deref(), self.icon.is_some()) };
        // A resizable dialog is as wide as its box was last frame.
        let width = match (self.resize, self.fixed_w) {
            (Some(_), _) => ui.data(|d| d.get_temp::<f32>(key.with("dialog-width"))).unwrap_or(self.size.base()).min(room),
            (None, Some(w)) => w.min(room_for_form),
            (None, None) => size::width(self.size, need, room_for_form),
        };
        ui.set_width(width);
        let total_w = if beside { width + guide::GAP + guide::PANE_W } else { width };

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

        let (left, top) = (ui.cursor().left(), ui.cursor().top());
        match self.custom {
            Some(draw) => draw(ui),
            None => {
                header::draw(ui, t, &self.title, self.subtitle.as_deref(), self.icon, left + total_w);
            }
        }
        ui.add_space(HEADER_GAP);
        let header_bottom = ui.cursor().top();
        if let Some((g, guide::Placement::Stacked)) = &guide {
            let h = guide::height(ui, t, g, width);
            let (rect, _) = ui.allocate_exact_size(vec2(width, h), egui::Sense::hover());
            guide::paint(ui, t, g, rect);
            ui.add_space(HEADER_GAP);
        }
        let title_h = ui.cursor().top() - top;
        let pane = guide.and_then(|(g, at)| (at == guide::Placement::Beside).then_some(g));
        Open {
            cx: self.cx,
            standard: self.size.base(),
            resize: self.resize,
            min_body: self.min_body,
            exact: self.fixed_w.is_some() && self.resize.is_none(),
            opened,
            focus_start,
            enter,
            cmd_enter,
            title_h,
            body_end: None,
            pane,
            left,
            top,
            header_bottom,
            width,
            total_w,
        }
    }
}

/// A dialog whose title is drawn; draw the body (optionally through `body`), then `finish`.
pub struct Open {
    cx: Cx,
    standard: f32,
    resize: Option<&'static str>,
    min_body: Option<f32>,
    /// A set width: the footer keeps to it exactly.
    exact: bool,
    opened: bool,
    focus_start: Option<Id>,
    enter: bool,
    cmd_enter: bool,
    title_h: f32,
    body_end: Option<f32>,
    /// The side pane beside the form, and where the dialog's content is.
    pane: Option<Guide>,
    left: f32,
    top: f32,
    header_bottom: f32,
    width: f32,
    total_w: f32,
}

impl Open {
    /// Draws `f` in the body's scroll area: as tall as its content (at least `Dialog::min_body`)
    /// up to what the window has room for, or, for a resizable dialog, filling the resizable box.
    pub fn body<R>(&mut self, ui: &mut Ui, f: impl FnOnce(&mut Ui) -> R) -> R {
        let key = self.cx.key;
        let screen = ui.ctx().screen_rect().size();
        let below = ui.data(|d| d.get_temp::<f32>(key.with("dialog-below"))).unwrap_or(FOOTER_GUESS);
        let cap = (screen.y - MARGIN - 2.0 * PAD - self.title_h - below).max(80.0);
        let scroll = egui::ScrollArea::vertical().id_salt(key.with("dialog-body"));
        let r = match self.resize {
            None => {
                let min = self.min_body;
                let f = move |ui: &mut Ui| {
                    if let Some(h) = min {
                        ui.set_min_height(h.min(cap));
                    }
                    f(ui)
                };
                scroll.auto_shrink([false, true]).max_height(cap).show(ui, f).inner
            }
            Some(salt) => {
                let max = egui::vec2((screen.x - MARGIN - 2.0 * PAD).max(MIN_WIDTH), cap);
                let min = egui::vec2(Size::Small.base().min(max.x), 200.0_f32.min(cap));
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
        let below_key = self.cx.key.with("dialog-below");
        let start = self.body_end.unwrap_or_else(|| ui.cursor().top());
        // A pane taller than the form: space above the footer lines the buttons up with its bottom.
        let pane_h = self.pane.as_ref().map(|g| guide::height(ui, self.cx.t, g, guide::PANE_W));
        let mut extra = 0.0;
        if let Some(h) = pane_h {
            let below_last = ui.data(|d| d.get_temp::<f32>(below_key)).unwrap_or(FOOTER_GUESS);
            let used = ui.cursor().top() - self.header_bottom;
            if h > used + below_last {
                extra = h - used - below_last;
                ui.add_space(extra);
            }
        }
        let footer::Laid { mut out, dismiss: dismiss_r, primary: primary_r, primary_on, tone, submit } =
            footer::lay_out(ui, footer, self.cx.t, self.exact);
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
        let run_default = submit || self.cmd_enter || (self.enter && self.focus_start.is_none()) || (field_enter && default_is_primary);
        if run_default {
            if default_is_primary {
                out.primary |= primary_on;
                out.attempted = !primary_on;
            } else if dismiss_r.is_some() {
                out.dismiss = true;
            }
        }
        // Measured before the pane is painted, which would grow the content's rect.
        let below = ui.min_rect().bottom() - start - extra;
        ui.data_mut(|d| d.insert_temp(below_key, below));
        if let (Some(g), Some(h)) = (&self.pane, pane_h) {
            let x = self.left + self.width + guide::GAP;
            let bottom = ui.min_rect().bottom().max(self.header_bottom + h);
            let rect = Rect::from_min_max(pos2(x, self.header_bottom), pos2(self.left + self.total_w, bottom));
            guide::paint(ui, self.cx.t, g, rect);
            ui.expand_to_include_rect(rect);
        }
        // The close button, at the content's right edge.
        ui.expand_to_include_rect(Rect::from_min_size(pos2(self.left, self.top), vec2(self.total_w, 1.0)));
        out
    }
}

/// The acknowledgement above the footer of a step that loses work for good: a checkbox with
/// `label`, or `common.ack.check`. The danger button stays disabled until it is ticked.
pub fn ack(ui: &mut Ui, t: Lang, acked: &mut bool, label: Option<&str>) -> Response {
    ui.add_space(10.0);
    let text = label.unwrap_or(t.tr("common.ack.check"));
    controls::checkbox(ui, acked, RichText::new(text).color(color::removed()))
}
