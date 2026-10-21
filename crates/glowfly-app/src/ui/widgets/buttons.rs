//! The button family every dialog, preference page, panel and header uses, and its one disabled
//! look:
//!
//! - primary: the view's main action, white strong text on the `primary` fill with a
//!   `primary_border` edge;
//! - tonal: an important action that isn't the view's main one, accent ink on the accent tint;
//! - secondary: everything else, `text` on the `panel` fill with an `input_border` edge;
//! - quiet: transparent until hovered, for toolbars, headers and rows;
//! - danger: a step that loses work, white strong text on the `danger` fill;
//! - icon-only (`icon_only`): a square quiet button whose name is its tooltip and accessible
//!   name;
//! - split (`split`, in `button_split.rs`): an action and its menu caret joined into one pill.
//!
//! Each state's colours are `button_look.rs`'s. Disabled is the same for every filled style: a
//! neutral `raised` fill, `weak` text and no edge, never an outline, and never egui's fade (the
//! button senses hover only instead of going through `add_enabled`); quiet ones keep no fill and
//! show `faint` text. A disabled button explains itself on hover with `why`; a disabled primary
//! or danger in a dialog footer or a panel also shows that reason next to it (`reason`, through
//! `blocked`). A focused filled button gets the 2 px accent ring 2 px outside its edge
//! (`tokens::focus_ring`); a quiet one, packed among others, the inset ring.
//!
//! Sizes: 28 px high by default, 24 px `small` (both at body-size text), taller with
//! `min_height`, or an exact `height` inside a fixed-height row. Corners are `radius::MD`.

use super::button_look::{self, Press};
use super::icon::{self, Icon};
use crate::ui::theme::tokens::{self, radius};
use eframe::egui::{self, Color32, CornerRadius, Response, Sense, StrokeKind, TextStyle, Ui, Widget, WidgetText, pos2, vec2};
use std::borrow::Cow;

pub use super::button_split::split;

/// Padding beside the text.
const PADDING: egui::Vec2 = vec2(12.0, 5.0);
/// The least room above and below the text. Body text lays out about 20 px tall, so the
/// minimum heights decide a button's height, not `PADDING.y`.
const MIN_PAD_Y: f32 = 2.0;
/// The least height of a button, and of a small one (inline Browse…, Retry).
const HEIGHT: f32 = 28.0;
const SMALL_HEIGHT: f32 = 24.0;
/// Between a leading icon and the text.
const ICON_GAP: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Tonal,
    Secondary,
    Quiet,
    Danger,
}

/// A button in one of the styles; add it with `ui.add`.
#[must_use = "add it with ui.add"]
pub struct StyledButton<'a> {
    text: Cow<'a, str>,
    kind: Kind,
    enabled: bool,
    why: Option<&'a str>,
    tip: Option<&'a str>,
    min_width: f32,
    min_height: f32,
    small: bool,
    text_size: Option<f32>,
    height: Option<f32>,
    max_width: Option<f32>,
    /// A leading icon, or the whole content of an icon-only button.
    icon: Option<Icon>,
    icon_only: bool,
}

/// The view's main action.
pub fn primary<'a>(text: impl Into<Cow<'a, str>>) -> StyledButton<'a> {
    StyledButton::new(text.into(), Kind::Primary)
}

/// An important action that isn't the view's main one (Stage all).
pub fn tonal<'a>(text: impl Into<Cow<'a, str>>) -> StyledButton<'a> {
    StyledButton::new(text.into(), Kind::Tonal)
}

/// Any other action, and Cancel or Close.
pub fn secondary<'a>(text: impl Into<Cow<'a, str>>) -> StyledButton<'a> {
    StyledButton::new(text.into(), Kind::Secondary)
}

/// An action in a toolbar, header or row: no fill until hovered. Part of the family before a
/// view needs it (the ghost controls share its look through `button_look::paint_quiet`).
#[allow(dead_code)]
pub fn quiet<'a>(text: impl Into<Cow<'a, str>>) -> StyledButton<'a> {
    StyledButton::new(text.into(), Kind::Quiet)
}

/// A step that loses work.
pub fn danger<'a>(text: impl Into<Cow<'a, str>>) -> StyledButton<'a> {
    StyledButton::new(text.into(), Kind::Danger)
}

/// A square quiet button showing `icon` only; `name` is its accessible name and, unless `tip`
/// is set, its tooltip.
pub fn icon_only<'a>(icon: Icon, name: impl Into<Cow<'a, str>>) -> StyledButton<'a> {
    StyledButton { icon: Some(icon), icon_only: true, ..StyledButton::new(name.into(), Kind::Quiet) }
}

impl<'a> StyledButton<'a> {
    fn new(text: Cow<'a, str>, kind: Kind) -> Self {
        Self {
            text,
            kind,
            enabled: true,
            why: None,
            tip: None,
            min_width: 0.0,
            min_height: 0.0,
            small: false,
            text_size: None,
            height: None,
            max_width: None,
            icon: None,
            icon_only: false,
        }
    }

    /// Whether it can be clicked (true by default).
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Why it can't be clicked, shown on hover while disabled.
    pub fn why(mut self, why: Option<&'a str>) -> Self {
        self.why = why;
        self
    }

    /// What it does, shown on hover while enabled (for a footer button, whose response the
    /// dialog keeps).
    pub fn tip(mut self, tip: &'a str) -> Self {
        self.tip = Some(tip);
        self
    }

    /// The least width (dialog footers use 76 px).
    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = w;
        self
    }

    /// A taller button than the 28 px standard (the commit box's Commit, the welcome choices).
    pub fn min_height(mut self, h: f32) -> Self {
        self.min_height = h;
        self
    }

    /// 24 px high, for buttons inline with a field or a row (Browse…, Retry).
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    /// A 16 px icon before the text. Part of the family before a view needs it.
    #[allow(dead_code)]
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Text of `px` instead of the button text style's size.
    pub fn text_size(mut self, px: f32) -> Self {
        self.text_size = Some(px);
        self
    }

    /// An exact height, for a button inside a fixed-height row (the diff's hunk headers).
    pub fn height(mut self, h: f32) -> Self {
        self.height = Some(h);
        self
    }

    /// The widest it gets: a longer label is cut with "…" (the commit box's button, which
    /// shares its pill with the more-actions caret).
    pub fn max_width(mut self, w: f32) -> Self {
        self.max_width = Some(w);
        self
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Why it is disabled, for showing next to it; None while it is enabled.
    pub fn blocked(&self) -> Option<&'a str> {
        if self.enabled { None } else { self.why }
    }

    pub(super) fn kind(&self) -> Kind {
        self.kind
    }

    /// The room a leading icon takes before the text.
    fn icon_w(&self) -> f32 {
        if self.icon.is_some() && !self.icon_only { icon::SIZE_MD + ICON_GAP } else { 0.0 }
    }

    /// The text laid out in the placeholder colour, so painting gives it the state's ink.
    fn galley(&self, ui: &Ui) -> std::sync::Arc<egui::Galley> {
        let shown = if self.icon_only { "" } else { self.text.as_ref() };
        let mut text = egui::RichText::new(shown).color(Color32::PLACEHOLDER);
        if let Some(px) = self.text_size {
            text = text.size(px);
        }
        match self.max_width {
            Some(max) => {
                let room = (max - 2.0 * PADDING.x - self.icon_w()).max(0.0);
                WidgetText::from(text).into_galley(ui, Some(egui::TextWrapMode::Truncate), room, TextStyle::Button)
            }
            None => WidgetText::from(text).into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, TextStyle::Button),
        }
    }

    fn size(&self, galley: &egui::Galley) -> egui::Vec2 {
        let h_min = if self.small { SMALL_HEIGHT } else { HEIGHT.max(self.min_height) };
        if self.icon_only {
            let side = self.height.unwrap_or(h_min);
            return vec2(side.max(self.min_width), side);
        }
        let w = (galley.size().x + 2.0 * PADDING.x + self.icon_w()).max(self.min_width);
        if let Some(h) = self.height {
            return vec2(w, h);
        }
        vec2(w, h_min.max(galley.size().y + 2.0 * MIN_PAD_Y))
    }

    /// The width it takes in `ui` (for laying out a right-aligned row).
    pub fn desired_width(&self, ui: &Ui) -> f32 {
        self.size(&self.galley(ui)).x
    }

    /// Adds it with `corners` (a split button's half takes its outer ones), `latched` while a
    /// menu it opens is open.
    pub(super) fn show_shaped(self, ui: &mut Ui, corners: CornerRadius, latched: bool) -> Response {
        let enabled = self.enabled && ui.is_enabled();
        let galley = self.galley(ui);
        let size = self.size(&galley);
        let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
        let text = self.text.as_ref();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, text));
        if ui.is_rect_visible(rect) {
            let press = if enabled { Press::of(resp.hovered(), resp.is_pointer_button_down_on(), latched) } else { Press::Rest };
            let look = button_look::look(self.kind, enabled, press);
            let p = ui.painter();
            p.rect(rect, corners, look.fill, look.edge, StrokeKind::Inside);
            let left = rect.center().x - (galley.size().x + self.icon_w()) / 2.0;
            match self.icon {
                Some(i) if self.icon_only => icon::paint_icon_sized(p, rect.center(), i, look.ink, icon::SIZE_MD),
                Some(i) => icon::paint_icon_sized(p, pos2(left + icon::SIZE_MD / 2.0, rect.center().y), i, look.ink, icon::SIZE_MD),
                None => {}
            }
            if !self.icon_only {
                p.galley(pos2(left + self.icon_w(), rect.center().y - galley.size().y / 2.0), galley, look.ink);
            }
            if resp.has_focus() {
                if self.kind == Kind::Quiet {
                    tokens::focus_ring_inset(p, rect, radius::MD);
                } else {
                    tokens::focus_ring(p, rect, radius::MD);
                }
            }
        }
        let tip = if self.icon_only { self.tip.or(Some(text)) } else { self.tip };
        if enabled {
            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            match tip {
                Some(tip) => resp.on_hover_text(tip),
                None => resp,
            }
        } else if let Some(why) = self.why {
            // In a disabled `Ui` (e.g. `add_enabled_ui(false, ..)`) the response is disabled too,
            // and egui shows only disabled-hover tooltips for it.
            if resp.enabled() { resp.on_hover_text(why) } else { resp.on_disabled_hover_text(why) }
        } else {
            resp
        }
    }
}

impl Widget for StyledButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.show_shaped(ui, CornerRadius::same(radius::MD), false)
    }
}

#[cfg(test)]
#[path = "button_tests.rs"]
mod tests;
