//! Dates drawn as labels with their full date on hover.

use super::theme::color;
use super::widgets::truncate;
use eframe::egui::{Response, RichText, Ui};
use glowfly_core::settings::Settings;

/// A weak label with `settings.format_time(secs…)` and the date tooltip, which also covers the
/// label when it is cut short. In a wrapping row it moves onto the next row rather than being
/// cut, and is cut only when no whole row has room for it.
pub fn time_label(ui: &mut Ui, settings: &Settings, secs: i64, now: i64, offset: i32) -> Response {
    let text = RichText::new(settings.format_time(secs, now, offset)).color(color::weak());
    let (r, _) = truncate::cut_label(ui, text, ui.available_width());
    r.on_hover_text(settings.date_tooltip(secs, now, offset))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widgets::truncate::tests::in_panel;
    use eframe::egui;

    #[test]
    fn a_date_after_a_long_author_wraps_whole_instead_of_being_cut() {
        let settings = Settings::default();
        let now = 1_760_000_000;
        let shown = settings.format_time(now - 3 * 86_400, now, 0);
        in_panel(300.0, |ui| {
            ui.horizontal_wrapped(|ui| {
                let row_top = ui.cursor().top();
                ui.add_space(ui.available_width() - 30.0);
                let r = time_label(ui, &settings, now - 3 * 86_400, now, 0);
                let natural = ui.fonts(|f| f.layout_no_wrap(shown.clone(), egui::TextStyle::Body.resolve(ui.style()), color::weak()));
                assert!(r.rect.top() > row_top, "the date moved onto the next row: {:?}", r.rect);
                assert!((r.rect.width() - natural.size().x).abs() < 0.5, "{shown:?} is drawn whole: {:?}", r.rect);
            });
        });
    }
}
