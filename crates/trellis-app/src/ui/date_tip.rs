//! Dates drawn as labels with their full date on hover.

use super::theme::color;
use eframe::egui::{Response, RichText, Ui};
use trellis_core::settings::Settings;

/// A weak label with `settings.format_time(secs…)` and the date tooltip.
pub fn time_label(ui: &mut Ui, settings: &Settings, secs: i64, now: i64, offset: i32) -> Response {
    ui.label(RichText::new(settings.format_time(secs, now, offset)).color(color::weak()))
        .on_hover_text(settings.date_tooltip(secs, now, offset))
}
