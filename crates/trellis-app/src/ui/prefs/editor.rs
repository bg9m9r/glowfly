//! Preferences > Editor: how the diff viewer draws text. Edits apply at once (views edit the
//! settings in place and the shell saves them); nothing reloads, since these only decide how
//! lines are painted.

use super::{help, row};
use eframe::egui::{self, Ui};
use trellis_core::settings::{DIFF_FONT_SIZES, Settings, TAB_SIZES};

pub fn editor_page(ui: &mut Ui, s: &mut Settings) {
    row(ui, "Font size", |ui| {
        ui.add(egui::DragValue::new(&mut s.diff_font_size).range(DIFF_FONT_SIZES).speed(0.25).max_decimals(1).suffix(" pt"));
        help(ui, "The size of the text in the diff viewer: hunks, the whole-file view and file history. The default is 12.5.");
    });
    row(ui, "Tab size", |ui| {
        ui.add(egui::DragValue::new(&mut s.tab_size).range(TAB_SIZES));
        help(ui, "Tabs in diff lines stop at every N columns. Only the display changes; files are never changed.");
    });
    row(ui, "Show line numbers", |ui| {
        ui.checkbox(&mut s.diff_line_numbers, "");
        help(ui, "The old and new line numbers beside each line in the inline, split and whole-file views.");
    });
    row(ui, "Syntax highlighting", |ui| {
        ui.checkbox(&mut s.syntax_highlighting, "");
        help(
            ui,
            "Colors code in the diff viewer by its language, worked out from the file name or, for scripts, the first line. Diffs with more than 2,000 lines or 100 KB of text, or with very long lines, are shown without colors.",
        );
    });
    row(ui, "Word-level highlights", |ui| {
        ui.checkbox(&mut s.diff_word_highlights, "");
        help(ui, "Marks the words that changed between a removed line and the added line paired with it.");
    });
    row(ui, "Wrap long lines", |ui| {
        ui.checkbox(&mut s.diff_word_wrap, "");
        help(
            ui,
            "Long lines in the diff viewer (hunks, the whole-file view and file history) continue on the next line instead of running past the right edge. The diff header has the same toggle.",
        );
    });
    row(ui, "Editor settings", |ui| {
        if ui.button("Reset to defaults").clicked() {
            reset(s);
        }
    });
}

/// Puts this page's settings back to their defaults, leaving every other setting alone.
fn reset(s: &mut Settings) {
    let d = Settings::default();
    s.diff_font_size = d.diff_font_size;
    s.tab_size = d.tab_size;
    s.diff_line_numbers = d.diff_line_numbers;
    s.syntax_highlighting = d.syntax_highlighting;
    s.diff_word_highlights = d.diff_word_highlights;
    s.diff_word_wrap = d.diff_word_wrap;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_restores_only_editor_fields() {
        let mut s = Settings {
            diff_font_size: 20.0,
            tab_size: 8,
            diff_line_numbers: false,
            syntax_highlighting: false,
            diff_word_highlights: false,
            diff_word_wrap: true,
            auto_fetch_minutes: 9,
            ..Settings::default()
        };
        reset(&mut s);
        let d = Settings::default();
        assert_eq!((s.diff_font_size, s.tab_size), (d.diff_font_size, d.tab_size));
        assert!(s.diff_line_numbers && s.syntax_highlighting && s.diff_word_highlights);
        assert!(!s.diff_word_wrap);
        assert_eq!(s.auto_fetch_minutes, 9);
    }
}
