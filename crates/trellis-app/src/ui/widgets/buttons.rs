//! The three button styles every dialog, preference page and panel uses, and their one disabled
//! look:
//!
//! - primary: the view's main action, white strong text on the `primary` fill with a
//!   `primary_border` edge;
//! - secondary: everything else, `text` on the `panel` fill with an `input_border` edge;
//! - danger: a step that loses work, white strong text on the `danger` fill.
//!
//! Hover shades the fill 10% toward white on dark palettes (black on light ones) and a press
//! 18%. Disabled is the same for every style: a neutral `toolbar` fill, `weak` text and no edge,
//! never an outline, and never egui's fade (the button senses hover only instead of going through
//! `add_enabled`). A disabled button explains itself on hover with `why`. A focused button gets a
//! 2 px `accent` ring outside its edge.

use super::super::theme::{color, contrast};
use eframe::egui::{self, Color32, CornerRadius, Response, Sense, Stroke, StrokeKind, TextStyle, Ui, Widget, WidgetText, vec2};
use std::borrow::Cow;

/// Padding around the text.
const PADDING: egui::Vec2 = vec2(12.0, 5.0);
/// The least height of a button, and of a small one (inline Browse…, Retry).
const HEIGHT: f32 = 28.0;
const SMALL_HEIGHT: f32 = 24.0;
const RADIUS: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Secondary,
    Danger,
}

/// A button in one of the three styles; add it with `ui.add`.
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
    radius: u8,
    text_size: Option<f32>,
}

/// The view's main action.
pub fn primary<'a>(text: impl Into<Cow<'a, str>>) -> StyledButton<'a> {
    StyledButton::new(text.into(), Kind::Primary)
}

/// Any other action, and Cancel or Close.
pub fn secondary<'a>(text: impl Into<Cow<'a, str>>) -> StyledButton<'a> {
    StyledButton::new(text.into(), Kind::Secondary)
}

/// A step that loses work.
pub fn danger<'a>(text: impl Into<Cow<'a, str>>) -> StyledButton<'a> {
    StyledButton::new(text.into(), Kind::Danger)
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
            radius: RADIUS,
            text_size: None,
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

    /// Rounder corners than the 4 px standard (the diff header's 6 px controls).
    pub fn radius(mut self, r: u8) -> Self {
        self.radius = r;
        self
    }

    /// Text of `px` instead of the button text style's size.
    pub fn text_size(mut self, px: f32) -> Self {
        self.text_size = Some(px);
        self
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// The text laid out in `fg` (egui's `strong` would pick its own colour over it).
    fn galley(&self, ui: &Ui, fg: Color32) -> std::sync::Arc<egui::Galley> {
        let mut text = egui::RichText::new(self.text.as_ref()).color(fg);
        if let Some(px) = self.text_size {
            text = text.size(px);
        }
        WidgetText::from(text).into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, TextStyle::Button)
    }

    fn size(&self, galley: &egui::Galley) -> egui::Vec2 {
        let h = if self.small { SMALL_HEIGHT } else { HEIGHT.max(self.min_height) };
        let s = galley.size() + 2.0 * PADDING;
        vec2(s.x.max(self.min_width), s.y.max(h))
    }

    /// The width it takes in `ui` (for laying out a right-aligned row).
    pub fn desired_width(&self, ui: &Ui) -> f32 {
        self.size(&self.galley(ui, Color32::PLACEHOLDER)).x
    }
}

/// (fill, text, edge) of an enabled button of `kind`, not hovered.
fn colors(kind: Kind) -> (Color32, Color32, Stroke) {
    match kind {
        Kind::Primary => (color::primary(), Color32::WHITE, Stroke::new(1.0_f32, color::primary_border())),
        Kind::Secondary => (color::panel(), color::text(), Stroke::new(1.0_f32, color::input_border())),
        Kind::Danger => (color::danger(), Color32::WHITE, Stroke::NONE),
    }
}

/// `fill` shaded `t` of the way toward white on dark palettes, or black on light ones.
fn shade(fill: Color32, t: f32) -> Color32 {
    contrast::mix(fill, if color::is_dark() { Color32::WHITE } else { Color32::BLACK }, t)
}

impl Widget for StyledButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let enabled = self.enabled && ui.is_enabled();
        let (fill, fg, stroke) = if enabled { colors(self.kind) } else { (color::toolbar(), color::weak(), Stroke::NONE) };
        let galley = self.galley(ui, fg);
        let size = self.size(&galley);
        let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
        let text = self.text.as_ref();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, text));
        if ui.is_rect_visible(rect) {
            let fill = if !enabled {
                fill
            } else if resp.is_pointer_button_down_on() {
                shade(fill, 0.18)
            } else if resp.hovered() {
                shade(fill, 0.10)
            } else {
                fill
            };
            let p = ui.painter();
            p.rect(rect, CornerRadius::same(self.radius), fill, stroke, StrokeKind::Inside);
            let pos = rect.center() - galley.size() / 2.0;
            p.galley(pos, galley, fg);
            if resp.has_focus() {
                p.rect_stroke(rect, CornerRadius::same(self.radius), Stroke::new(2.0_f32, color::accent()), StrokeKind::Outside);
            }
        }
        if enabled {
            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            match self.tip {
                Some(tip) => resp.on_hover_text(tip),
                None => resp,
            }
        } else if let Some(why) = self.why {
            resp.on_hover_text(why)
        } else {
            resp
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    #[test]
    fn disabled_buttons_are_named_and_inert() {
        let mut clicks = 0;
        let mut h = Harness::new_ui(|ui| {
            if ui.add(primary("Save").enabled(false).why(Some("Nothing changed"))).clicked() {
                clicks += 1;
            }
            ui.add(secondary("Cancel"));
        });
        h.run();
        assert!(h.get_by_label("Save").is_disabled());
        assert!(!h.get_by_label("Cancel").is_disabled());
        h.get_by_label("Save").click();
        h.run();
        drop(h);
        assert_eq!(clicks, 0);
    }

    #[test]
    fn sizes_meet_the_minimums() {
        let mut h = Harness::new_ui(|ui| {
            assert_eq!(secondary("OK").min_width(76.0).desired_width(ui), 76.0);
            let r = ui.add(secondary("OK"));
            assert!(r.rect.height() >= HEIGHT);
            let r = ui.add(secondary("OK").small());
            assert!(r.rect.height() >= SMALL_HEIGHT && r.rect.height() < HEIGHT);
        });
        h.run();
    }

    #[test]
    fn hover_shades_toward_the_far_end() {
        let f = Color32::from_rgb(100, 100, 100);
        color::set_dark(true);
        assert!(shade(f, 0.1).r() > 100);
        color::set_dark(false);
        assert!(shade(f, 0.1).r() < 100);
        color::set_dark(true);
    }
}
