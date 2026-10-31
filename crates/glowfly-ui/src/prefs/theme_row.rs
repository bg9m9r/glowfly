//! Preferences > Appearance > Theme: the built-in themes' cards (`theme_picker.rs`), the
//! theme files' dropdown, the themes folder's buttons, and the selected file's problems.

use super::{help, row};
use crate::msg::Msg;
use crate::state::theme_names;
use crate::themes::{ThemeMsg, ThemeState};
use crate::ui::theme::type_scale;
use crate::ui::theme::{color, custom};
use crate::ui::widgets::buttons;
use eframe::egui::{self, RichText, Ui};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::{BuiltinTheme, Settings, ThemeCatalog, ThemeChoice};

pub fn theme_row(ui: &mut Ui, s: &mut Settings, themes: &ThemeState, msgs: &mut Vec<Msg>) {
    let t = s.lang();
    // The cards take a wider column than a form row's fields, up to four to a row.
    let wide = (ui.available_width() - super::form::field_left(ui)).clamp(0.0, PICKER_W);
    row(ui, t.tr("prefs.ui.theme.label"), |ui| {
        ui.set_max_width(wide.max(ui.available_width()));
        super::theme_picker::picker(ui, t, &mut s.theme);
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(t.tr("prefs.ui.theme.files.label")).color(color::text()));
            let shown = selected_text(t, &s.theme, &themes.catalog);
            egui::ComboBox::from_id_salt("theme").selected_text(shown).width(200.0).show_ui(ui, |ui| {
                if themes.catalog.files.is_empty() {
                    ui.label(RichText::new(t.tr("prefs.ui.theme.no_files")).color(color::weak()));
                }
                for f in &themes.catalog.files {
                    ui.selectable_value(&mut s.theme, ThemeChoice::Custom(f.id.clone()), theme_names::file_entry(t, f));
                }
            });
            if ui.add(buttons::secondary(t.tr("prefs.ui.theme.open_folder.button"))).clicked() {
                msgs.push(Msg::Theme(ThemeMsg::OpenFolder));
            }
            if ui.add(buttons::secondary(t.tr("prefs.ui.theme.reload.button"))).clicked() {
                msgs.push(Msg::Theme(ThemeMsg::Reload));
            }
            if ui.add(buttons::secondary(t.tr("prefs.ui.theme.create.button"))).clicked() {
                let p = color::current();
                msgs.push(Msg::Theme(ThemeMsg::CreateFromCurrent { colors: custom::starter(&p), dark: p.dark }));
            }
        });
        help(ui, t.tr("prefs.ui.theme.help"));
        problems(ui, t, &s.theme, &themes.catalog);
    });
}

/// The widest the theme cards' column gets: four cards and their gaps.
const PICKER_W: f32 = 660.0;

/// The theme file dropdown's text: the file's name, "<id> (missing)", or None for a built-in.
fn selected_text(t: Lang, choice: &ThemeChoice, cat: &ThemeCatalog) -> String {
    match choice {
        ThemeChoice::Custom(id) => cat.get(id).map_or_else(|| t.trf("prefs.ui.theme.missing", &[("id", id)]), |f| f.name.clone()),
        _ => t.tr("prefs.ui.theme.files.none").to_owned(),
    }
}

fn line(ui: &mut Ui, text: String, c: egui::Color32) {
    ui.add(egui::Label::new(RichText::new(text).size(type_scale::SMALL).color(c)).wrap());
}

/// The folder's error, and the selected file's problems (fatal ones red) or why it is missing.
/// The problems and the error come from glowfly-core as they are.
fn problems(ui: &mut Ui, t: Lang, choice: &ThemeChoice, cat: &ThemeCatalog) {
    if let Some(e) = &cat.error {
        line(ui, e.clone(), color::removed());
    }
    let ThemeChoice::Custom(id) = choice else { return };
    let Some(f) = cat.get(id) else {
        let dark = theme_names::builtin(t, BuiltinTheme::DEFAULT_DARK);
        line(ui, t.trf("prefs.ui.theme.not_in_folder", &[("file", &format!("{id}.json")), ("theme", &dark)]), color::warn());
        return;
    };
    for p in &f.problems {
        line(ui, format!("{}: {p}", f.file_name()), if p.is_fatal() { color::removed() } else { color::warn() });
    }
    line(ui, t.trf("prefs.ui.theme.base", &[("theme", &theme_names::builtin(t, f.base))]), color::weak());
}
