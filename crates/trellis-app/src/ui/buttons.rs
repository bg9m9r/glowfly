//! Filled and outlined buttons: the primary action of a view, and the danger buttons of steps
//! that lose work.

use super::super::theme::color;
use eframe::egui::{self, Color32, RichText, Stroke};

/// The view's main action: white strong text on the primary fill, or faint text on the dimmed
/// fill when `enabled` is false.
pub fn primary(text: &str, enabled: bool) -> egui::Button<'static> {
    let (fg, fill) = if enabled { (Color32::WHITE, color::primary()) } else { (color::faint(), color::primary_off()) };
    egui::Button::new(RichText::new(text.to_owned()).strong().color(fg)).fill(fill).stroke(Stroke::new(1.0_f32, color::primary_border()))
}

/// A confirmation's button for a step that loses work: white text on the danger fill.
pub fn danger(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_owned()).strong().color(Color32::WHITE)).fill(color::danger())
}

/// A button that starts a step losing work (after asking): red text and border on the normal fill.
pub fn danger_outline(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_owned()).color(color::red())).stroke(Stroke::new(1.0_f32, color::red()))
}
