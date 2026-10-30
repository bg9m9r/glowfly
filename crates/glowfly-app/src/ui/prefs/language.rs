//! Preferences > Language & region: the interface language, and how dates and times are written
//! (the locale, four patterns that follow the locale while blank, each with a live preview of
//! the current time, and relative dates). It
//! edits the settings in place, so the next frame is drawn in the new language and the shell
//! saves the change.

use super::{check_row, help, row, section};
use crate::ui::theme::{color, controls, type_scale};
use eframe::egui::{self, RichText, Ui, vec2};
use glowfly_core::i18n::{self, Lang, SYSTEM};
use glowfly_core::settings::{DateKind, Settings};

/// `now` is the current time and the local UTC offset, for the previews.
pub fn language_page(ui: &mut Ui, s: &mut Settings, now: (i64, i32)) {
    let t = s.lang();
    section(ui, t.tr("prefs.language.section"));
    language_row(ui, s);
    date_time_section(ui, t, s, now);
}

fn language_row(ui: &mut Ui, s: &mut Settings) {
    let t = s.lang();
    row(ui, t.tr("prefs.general.language.label"), |ui| {
        let system_label = t.trf("prefs.general.language.system", &[("language", &i18n::system().native_name())]);
        let selected = match Lang::from_code(&s.language) {
            Some(lang) => lang.native_name().to_owned(),
            None => system_label.clone(),
        };
        // A fixed id: the label changes with the language, the widget's state must not.
        egui::ComboBox::from_id_salt("language").width(super::fit(ui, 320.0)).selected_text(selected).show_ui(ui, |ui| {
            ui.selectable_value(&mut s.language, SYSTEM.to_owned(), system_label);
            for lang in Lang::ALL {
                ui.selectable_value(&mut s.language, lang.code().to_owned(), lang.native_name());
            }
        });
        help(ui, t.tr("prefs.general.language.help"));
    });
}

/// Dates and times: the tokens patterns take (which stay literal in every language), the
/// locale, the four patterns and relative dates.
fn date_time_section(ui: &mut Ui, t: Lang, s: &mut Settings, now: (i64, i32)) {
    section(ui, t.tr("prefs.ui.date_time.section"));
    ui.add(egui::Label::new(RichText::new(t.tr("prefs.ui.date_time.patterns_help")).size(type_scale::SMALL).color(color::weak())).wrap());
    ui.add_space(10.0);
    row(ui, t.tr("prefs.ui.date_locale.label"), |ui| {
        let current = glowfly_core::dates::LOCALES
            .iter()
            .find(|(k, _)| *k == s.date_locale)
            .map(|(_, l)| *l)
            .unwrap_or(s.date_locale.as_str())
            .to_owned();
        let shown = if s.date_locale == "system" {
            t.trf("prefs.ui.date_locale.system", &[("locale", &glowfly_core::dates::system_date_locale().unwrap_or_default())])
        } else {
            current
        };
        egui::ComboBox::from_id_salt("locale").selected_text(shown).width(super::fit(ui, 230.0)).show_ui(ui, |ui| {
            for (k, label) in glowfly_core::dates::LOCALES {
                ui.selectable_value(&mut s.date_locale, k.to_owned(), label);
            }
        });
    });
    // After the locale picker, so a new locale's hints and previews show on the same frame.
    let [date, long, date_time, full] = [DateKind::Date, DateKind::LongDate, DateKind::DateTime, DateKind::Full].map(|k| s.date_default(k));
    let loc = s.date_locale.clone();
    date_pattern(ui, t.tr("prefs.ui.date_format.label"), &mut s.date_format, date, &loc, now);
    date_pattern(ui, t.tr("prefs.ui.long_date_format.label"), &mut s.date_word_format, long, &loc, now);
    date_pattern(ui, t.tr("prefs.ui.date_time_format.label"), &mut s.date_time_format, date_time, &loc, now);
    date_pattern(ui, t.tr("prefs.ui.full_date_format.label"), &mut s.date_verbose_format, full, &loc, now);
    let (label, check) = (t.tr("prefs.ui.relative_dates.label"), t.tr("prefs.ui.relative_dates.check"));
    check_row(ui, label, &mut s.relative_dates, check, None);
}

/// A date pattern field with a live preview of the current time; blank shows and uses `default`.
fn date_pattern(ui: &mut Ui, label: &str, value: &mut String, default: &str, locale: &str, now: (i64, i32)) {
    row(ui, label, |ui| {
        ui.horizontal_wrapped(|ui| {
            let w = super::fit(ui, 230.0);
            controls::field(ui, egui::TextEdit::singleline(value).hint_text(default).desired_width(w).margin(vec2(8.0, 5.0)));
            let pattern = if value.trim().is_empty() { default } else { value.as_str() };
            let preview = glowfly_core::dates::format(now.0, pattern, locale, now.1);
            ui.label(RichText::new(preview).size(type_scale::SMALL).color(color::weak()));
        });
    });
}
