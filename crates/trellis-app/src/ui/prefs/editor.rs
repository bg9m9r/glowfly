//! Preferences > Editor: how the diff viewer draws text. Edits apply at once (views edit the
//! settings in place and the shell saves them); nothing reloads, since these only decide how
//! lines are painted.

use super::{help, row};
use crate::ui::theme::controls;
use crate::ui::widgets::buttons;
use eframe::egui::{self, Ui};
use trellis_core::settings::{DIFF_FONT_SIZES, HistoryPane, Settings, TAB_SIZES};

pub fn editor_page(ui: &mut Ui, s: &mut Settings) {
    let t = s.lang();
    row(ui, t.tr("prefs.editor.font_size.label"), |ui| {
        let size = egui::DragValue::new(&mut s.diff_font_size).range(DIFF_FONT_SIZES).speed(0.25).max_decimals(1);
        ui.add(size.suffix(t.tr("prefs.editor.font_size.suffix")));
        help(ui, t.tr("prefs.editor.font_size.help"));
    });
    row(ui, t.tr("prefs.editor.tab_size.label"), |ui| {
        ui.add(egui::DragValue::new(&mut s.tab_size).range(TAB_SIZES));
        help(ui, t.tr("prefs.editor.tab_size.help"));
    });
    row(ui, t.tr("prefs.editor.line_numbers.label"), |ui| {
        controls::checkbox(ui, &mut s.diff_line_numbers, "");
        help(ui, t.tr("prefs.editor.line_numbers.help"));
    });
    row(ui, t.tr("prefs.editor.syntax.label"), |ui| {
        controls::checkbox(ui, &mut s.syntax_highlighting, "");
        help(ui, t.tr("prefs.editor.syntax.help"));
    });
    row(ui, t.tr("prefs.editor.word_highlights.label"), |ui| {
        controls::checkbox(ui, &mut s.diff_word_highlights, "");
        help(ui, t.tr("prefs.editor.word_highlights.help"));
    });
    row(ui, t.tr("prefs.editor.wrap.label"), |ui| {
        controls::checkbox(ui, &mut s.diff_word_wrap, "");
        help(ui, t.tr("prefs.editor.wrap.help"));
    });
    row(ui, t.tr("prefs.editor.history_pane.label"), |ui| {
        ui.horizontal(|ui| {
            controls::radio_value(ui, &mut s.history.pane, HistoryPane::Diff, t.tr("prefs.editor.history_pane.diff"));
            controls::radio_value(ui, &mut s.history.pane, HistoryPane::File, t.tr("prefs.editor.history_pane.file"));
        });
        help(ui, t.tr("prefs.editor.history_pane.help"));
    });
    row(ui, t.tr("prefs.editor.reset.label"), |ui| {
        if ui.add(buttons::secondary(t.tr("prefs.editor.reset.button"))).clicked() {
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
    s.history = d.history;
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
            history: trellis_core::settings::HistoryPrefs { pane: HistoryPane::File },
            auto_fetch_minutes: 9,
            ..Settings::default()
        };
        reset(&mut s);
        let d = Settings::default();
        assert_eq!((s.diff_font_size, s.tab_size), (d.diff_font_size, d.tab_size));
        assert!(s.diff_line_numbers && s.syntax_highlighting && s.diff_word_highlights);
        assert!(!s.diff_word_wrap);
        assert_eq!(s.history.pane, HistoryPane::Diff);
        assert_eq!(s.auto_fetch_minutes, 9);
    }
}
