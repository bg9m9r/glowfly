//! A text field that underlines misspellings and offers corrections on right-click.

use super::theme::color;
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use eframe::egui::{self, Stroke, TextFormat, text::LayoutJob};
use trellis_core::spell::{Miss, Speller};

/// Like `TextEdit`, with spell checking when `enabled`. `rows == 1` makes a single-line field.
pub fn spell_text_edit(
    ui: &mut egui::Ui,
    text: &mut String,
    hint: &str,
    rows: usize,
    speller: &mut Speller,
    enabled: bool,
) -> egui::Response {
    let enabled = enabled && speller.available();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let text_color = ui.visuals().text_color();
    let mut layouter = |ui: &egui::Ui, s: &str, wrap_width: f32| {
        let misses = if enabled { speller.check(s) } else { Vec::new() };
        let mut job = layout(s, &misses, font.clone(), text_color);
        job.wrap.max_width = wrap_width;
        ui.fonts(|f| f.layout_job(job))
    };
    let edit = if rows <= 1 { egui::TextEdit::singleline(text) } else { egui::TextEdit::multiline(text).desired_rows(rows) };
    let resp = controls::field(ui, edit.hint_text(hint).desired_width(f32::INFINITY).layouter(&mut layouter));
    if enabled {
        let misses = speller.check(text);
        if !misses.is_empty() {
            resp.context_menu(|ui| corrections(ui, text, &misses, speller));
        }
    }
    resp
}

/// Text with misspelled ranges underlined.
fn layout(s: &str, misses: &[Miss], font: egui::FontId, text_color: egui::Color32) -> LayoutJob {
    let plain = TextFormat { font_id: font, color: text_color, ..Default::default() };
    let wrong = TextFormat { underline: Stroke::new(1.5_f32, color::removed()), ..plain.clone() };
    let mut job = LayoutJob::default();
    let mut last = 0;
    for m in misses {
        let end = m.start + m.len;
        if m.start < last || end > s.len() || !s.is_char_boundary(m.start) || !s.is_char_boundary(end) {
            continue;
        }
        job.append(&s[last..m.start], 0.0, plain.clone());
        job.append(&s[m.start..end], 0.0, wrong.clone());
        last = end;
    }
    job.append(&s[last..], 0.0, plain);
    job
}

/// One submenu per misspelled word (up to five): suggestions and "Add to dictionary".
fn corrections(ui: &mut egui::Ui, text: &mut String, misses: &[Miss], speller: &mut Speller) {
    ui.label(egui::RichText::new("Spelling").size(type_scale::SMALL).color(color::weak()));
    let mut seen = Vec::new();
    for m in misses.iter().take(5) {
        let Some(word) = text.get(m.start..m.start + m.len).map(str::to_owned) else { continue };
        if seen.contains(&word) {
            continue;
        }
        seen.push(word.clone());
        ui.menu_button(&word, |ui| {
            let suggestions = speller.suggest(&word);
            if suggestions.is_empty() {
                ui.label(egui::RichText::new("No suggestions").color(color::weak()));
            }
            for s in suggestions {
                if ui.button(&s).clicked() {
                    text.replace_range(m.start..m.start + m.len, &s);
                    ui.close_menu();
                }
            }
            ui.separator();
            if ui.button("Add to dictionary").clicked() {
                speller.add_to_dictionary(&word);
                ui.close_menu();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn underlines_only_valid_ranges() {
        let font = egui::FontId::proportional(14.0);
        let misses = [Miss { start: 4, len: 5 }, Miss { start: 2, len: 3 }, Miss { start: 40, len: 2 }];
        let job = layout("the wrold is", &misses, font, egui::Color32::WHITE);
        let underlined: Vec<&str> =
            job.sections.iter().filter(|s| s.format.underline.width > 0.0).map(|s| &job.text[s.byte_range.clone()]).collect();
        assert_eq!(underlined, ["wrold"], "overlapping and out-of-range misses are skipped");
    }
}
