//! Preferences > General: the Language row. It edits `settings.language` in place, so the next
//! frame is drawn in the new language and the shell saves the change.

use super::{help, row};
use eframe::egui::{self, Ui};
use trellis_core::i18n::{self, Lang, SYSTEM};
use trellis_core::settings::Settings;

pub fn language_row(ui: &mut Ui, s: &mut Settings) {
    let t = s.lang();
    row(ui, t.tr("prefs.general.language.label"), |ui| {
        let system_label = t.trf("prefs.general.language.system", &[("language", &i18n::system().native_name())]);
        let selected = match Lang::from_code(&s.language) {
            Some(lang) => lang.native_name().to_owned(),
            None => system_label.clone(),
        };
        // A fixed id: the label changes with the language, the widget's state must not.
        egui::ComboBox::from_id_salt("language").width(320.0).selected_text(selected).show_ui(ui, |ui| {
            ui.selectable_value(&mut s.language, SYSTEM.to_owned(), system_label);
            for lang in Lang::ALL {
                ui.selectable_value(&mut s.language, lang.code().to_owned(), lang.native_name());
            }
        });
        help(ui, t.tr("prefs.general.language.help"));
    });
}
