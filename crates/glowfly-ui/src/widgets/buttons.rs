//! The button family every dialog, preference page, panel and header uses, and its one disabled
//! look:
//!
//! - primary: the view's main action, white strong text on the `primary` fill with a
//!   `primary_border` edge;
//! - tonal: an important action that isn't the view's main one, accent ink on the accent tint;
//! - secondary: everything else, `text` on the `panel` fill with an `input_border` edge;
//! - quiet: transparent until hovered, for toolbars, headers and rows;
//! - danger: a step that loses work, white strong text on the `danger` fill;
//! - quiet danger (`danger_quiet`): a step that loses work among quiet ones, danger text and
//!   no fill until hovered;
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
//! Sizes come from `theme::sizes` (`size`, Regular by default): the height, the text step, the
//! room beside the text (`pad_x` framed, `quiet_pad_x` quiet), the icon and its gap, and the
//! corners. Compact is for quiet buttons only. The text keeps at least 4 px above and below
//! inside a framed button's edge and 3 px inside a quiet one's. An exact `height` beside a
//! field (`text_field::with_button`), until fields take a size.

use super::button_look::{self, Press, Target};
use super::icon::{self, Icon};
use crate::theme::sizes::{ControlSize, Metrics};
use eframe::egui::{self, Color32, CornerRadius, Rect, Response, Sense, TextStyle, Ui, Widget, WidgetText, pos2, vec2};
use std::borrow::Cow;

pub use super::button_split::split;

/// The least room above and below the text. The sizes' heights keep more than this, so they
/// decide a button's height; this only matters for text taller than its size expects.
const MIN_PAD_Y: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Primary,
    #[allow(dead_code)]
    Tonal,
    Secondary,
    Quiet,
    Danger,
    /// Danger text with no fill until hovered.
    DangerQuiet,
}

impl Kind {
    /// Whether it has a border or a fill at rest: every kind but the quiet ones.
    pub fn framed(self) -> bool {
        !matches!(self, Self::Quiet | Self::DangerQuiet)
    }
}

/// A button in one of the styles; add it with `ui.add`.
#[must_use = "add it with ui.add"]
pub struct StyledButton<'a> {
    text: Cow<'a, str>,
    kind: Kind,
    enabled: bool,
    why: Option<&'a str>,
    tip: Option<&'a str>,
    /// The tooltip's wrap width, when not the app's (`tooltip::style`).
    tip_width: Option<f32>,
    /// Its accessible name, when not its text (a label whose names were cut).
    access_name: Option<&'a str>,
    min_width: f32,
    height: Option<f32>,
    size: ControlSize,
    max_width: Option<f32>,
    /// A leading icon, or the whole content of an icon-only button.
    icon: Option<Icon>,
    icon_only: bool,
    /// The size an icon is drawn at, when not the size's (a glyph drawn small on its grid).
    icon_px: Option<f32>,
}

/// The view's main action.
pub fn primary<'a>(text: impl Into<Cow<'a, str>>) -> StyledButton<'a> {
    StyledButton::new(text.into(), Kind::Primary)
}

/// An important action that isn't the view's main one. Part of the family before a view needs
/// it again (the WIP lists' header actions became quiet).
#[allow(dead_code)]
pub(crate) fn tonal<'a>(text: impl Into<Cow<'a, str>>) -> StyledButton<'a> {
    StyledButton::new(text.into(), Kind::Tonal)
}

/// Any other action, and Cancel or Close.
pub fn secondary<'a>(text: impl Into<Cow<'a, str>>) -> StyledButton<'a> {
    StyledButton::new(text.into(), Kind::Secondary)
}

/// An action in a toolbar, header or row: no fill until hovered (the ghost controls share its
/// look through `button_look::paint_target`).
pub fn quiet<'a>(text: impl Into<Cow<'a, str>>) -> StyledButton<'a> {
    StyledButton::new(text.into(), Kind::Quiet)
}

/// A step that loses work.
pub fn danger<'a>(text: impl Into<Cow<'a, str>>) -> StyledButton<'a> {
    StyledButton::new(text.into(), Kind::Danger)
}

/// A step that loses work among quiet actions (a hunk's Discard): danger text, no fill until
/// hovered.
pub(crate) fn danger_quiet<'a>(text: impl Into<Cow<'a, str>>) -> StyledButton<'a> {
    StyledButton::new(text.into(), Kind::DangerQuiet)
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
            tip_width: None,
            access_name: None,
            min_width: 0.0,
            height: None,
            size: ControlSize::Regular,
            max_width: None,
            icon: None,
            icon_only: false,
            icon_px: None,
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

    /// Wraps the tooltip at `w` instead of the app's tooltip width (a git command on one line).
    pub fn tip_width(mut self, w: f32) -> Self {
        self.tip_width = Some(w);
        self
    }

    /// Names it `name` for screen readers instead of its text: the whole label when the text
    /// cuts a name, as the branch menus' rows do.
    pub fn access_name(mut self, name: &'a str) -> Self {
        self.access_name = Some(name);
        self
    }

    /// The least width (dialog footers use 76 px).
    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = w;
        self
    }

    /// Its size (Regular by default): Small for buttons inline with a field, a row or a line of
    /// chips (Browse…, Retry, Compare with working directory). Compact is for quiet buttons
    /// only: asking for a framed Compact button panics in debug builds.
    pub fn size(mut self, size: ControlSize) -> Self {
        debug_assert!(size.allows_frame() || !self.kind.framed(), "a {:?} button can't be Compact", self.kind);
        self.size = size;
        self
    }

    /// An icon before the text, at the size's icon size.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Its icon drawn at `px` instead of the size's (for a glyph that fills less or more of its
    /// grid than the others), keeping the button's size.
    pub(crate) fn icon_px(mut self, px: f32) -> Self {
        self.icon_px = Some(px);
        self
    }

    /// An exact height: as tall as the field it sits beside (`text_field::with_button`).
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

    /// Its size.
    pub(super) fn control_size(&self) -> ControlSize {
        self.size
    }

    /// Its size's corners.
    pub(super) fn radius(&self) -> u8 {
        self.metrics().radius
    }

    fn metrics(&self) -> Metrics {
        self.size.metrics()
    }

    /// The room beside the text: the size's framed or quiet padding.
    fn pad_x(&self) -> f32 {
        let m = self.metrics();
        if self.kind.framed() { m.pad_x } else { m.quiet_pad_x }
    }

    /// A leading icon's size, or an icon-only button's: the size's icon.
    fn icon_size(&self) -> f32 {
        self.icon_px.unwrap_or(self.metrics().icon)
    }

    /// The room a leading icon takes before the text.
    fn icon_w(&self) -> f32 {
        if self.icon.is_some() && !self.icon_only { self.icon_size() + self.metrics().icon_gap } else { 0.0 }
    }

    /// The text laid out in the placeholder colour, so painting gives it the state's ink.
    fn galley(&self, ui: &Ui) -> std::sync::Arc<egui::Galley> {
        let shown = if self.icon_only { "" } else { self.text.as_ref() };
        let mut text = egui::RichText::new(shown).color(Color32::PLACEHOLDER);
        text = text.size(self.metrics().text);
        match self.max_width {
            Some(max) => {
                let room = (max - 2.0 * self.pad_x() - self.icon_w()).max(0.0);
                WidgetText::from(text).into_galley(ui, Some(egui::TextWrapMode::Truncate), room, TextStyle::Button)
            }
            None => WidgetText::from(text).into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, TextStyle::Button),
        }
    }

    /// Its size around `galley`. Where the size sets the height (no `height`), debug builds
    /// check the inset against the real text: at least `galley_h + 8` for a framed button and
    /// `+ 6` for a quiet one.
    fn measure(&self, galley: &egui::Galley) -> egui::Vec2 {
        let h_min = self.metrics().h;
        if self.icon_only {
            let side = self.height.unwrap_or(h_min);
            return vec2(side.max(self.min_width), side);
        }
        let w = (galley.size().x + 2.0 * self.pad_x() + self.icon_w()).max(self.min_width);
        if let Some(h) = self.height {
            return vec2(w, h);
        }
        debug_assert!(
            h_min + 0.5 >= galley.size().y + ControlSize::inset(self.kind.framed()),
            "{:?} text {} px tall doesn't keep its inset in a {h_min} px {:?} button",
            self.size,
            galley.size().y,
            self.kind
        );
        vec2(w, h_min.max(galley.size().y + 2.0 * MIN_PAD_Y))
    }

    /// The width it takes in `ui` (for laying out a right-aligned row).
    pub fn desired_width(&self, ui: &Ui) -> f32 {
        self.measure(&self.galley(ui)).x
    }

    /// Its text and its size around it.
    pub(super) fn layout(&self, ui: &Ui) -> (std::sync::Arc<egui::Galley>, egui::Vec2) {
        let galley = self.galley(ui);
        let size = self.measure(&galley);
        (galley, size)
    }

    /// Whether it can be clicked in `ui`.
    pub(super) fn live(&self, ui: &Ui) -> bool {
        self.enabled && ui.is_enabled()
    }

    /// Names `resp` for screen readers.
    pub(super) fn name(&self, resp: &Response, enabled: bool) {
        let text = self.access_name.unwrap_or(self.text.as_ref());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, text));
    }

    /// Paints its icon and text in `ink`, centred in `rect`.
    pub(super) fn paint_content(&self, p: &egui::Painter, rect: Rect, galley: std::sync::Arc<egui::Galley>, ink: Color32) {
        let left = rect.center().x - (galley.size().x + self.icon_w()) / 2.0;
        match self.icon {
            Some(i) if self.icon_only => icon::paint_icon_sized(p, rect.center(), i, ink, self.icon_size()),
            Some(i) => {
                let side = self.icon_size();
                icon::paint_icon_sized(p, pos2(left + side / 2.0, rect.center().y), i, ink, side);
            }
            None => {}
        }
        if !self.icon_only {
            p.galley(pos2(left + self.icon_w(), rect.center().y - galley.size().y / 2.0), galley, ink);
        }
    }

    /// `resp` with the pointing hand and its tooltip while `enabled`, or its reason on hover
    /// while disabled.
    pub(super) fn explain(&self, resp: Response, enabled: bool) -> Response {
        let tip = if self.icon_only { self.tip.or(Some(self.text.as_ref())) } else { self.tip };
        if enabled {
            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            match (tip, self.tip_width) {
                (Some(tip), Some(w)) => resp.on_hover_ui(|ui| {
                    ui.set_max_width(w);
                    ui.add(egui::Label::new(tip));
                }),
                (Some(tip), None) => resp.on_hover_text(tip),
                (None, _) => resp,
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
    /// Adds it standing alone: its look over its whole shape through `button_look::paint_button`,
    /// a framed kind's edge, and the focus ring (outside a framed kind's edge, inset in a quiet
    /// one).
    fn ui(self, ui: &mut Ui) -> Response {
        let enabled = self.live(ui);
        let (galley, size) = self.layout(ui);
        let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
        self.name(&resp, enabled);
        if ui.is_rect_visible(rect) {
            let press = if enabled { Press::of(resp.hovered(), resp.is_pointer_button_down_on(), false) } else { Press::Rest };
            let look = button_look::look(self.kind, enabled, press);
            let corners = CornerRadius::same(self.radius());
            let target = Target { rect, corners, edge: self.kind.framed().then_some(look.edge) };
            button_look::paint_button(ui.painter(), target, self.size, look, resp.has_focus());
            self.paint_content(ui.painter(), rect, galley, look.ink);
        }
        self.explain(resp, enabled)
    }
}

#[cfg(test)]
#[path = "button_tests.rs"]
mod tests;
