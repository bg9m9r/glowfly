//! Dates drawn as labels with their full date on hover.

use super::theme::color;
use super::widgets::truncate;
use eframe::egui::{Response, RichText, Ui};
use trellis_core::settings::Settings;

/// A weak label with `settings.format_time(secs…)` and the date tooltip, which also covers the
/// label when it is cut short.
pub fn time_label(ui: &mut Ui, settings: &Settings, secs: i64, now: i64, offset: i32) -> Response {
    let text = RichText::new(settings.format_time(secs, now, offset)).color(color::weak());
    let (r, _) = truncate::cut_label(ui, text, ui.available_width());
    r.on_hover_text(settings.date_tooltip(secs, now, offset))
}
