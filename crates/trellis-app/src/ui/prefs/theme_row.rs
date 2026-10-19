//! Preferences > UI Customization > Theme: the built-in themes and the theme files, the
//! themes folder's buttons, and the selected file's problems.

use super::{help, row};
use crate::msg::Msg;
use crate::themes::{ThemeMsg, ThemeState};
use crate::ui::theme::type_scale;
use crate::ui::theme::{color, custom};
use crate::ui::theme_names;
use crate::ui::widgets::buttons;
use eframe::egui::{self, RichText, Ui};
use trellis_core::i18n::Lang;
use trellis_core::settings::{BuiltinTheme, Settings, ThemeCatalog, ThemeChoice};

pub fn theme_row(ui: &mut Ui, s: &mut Settings, themes: &ThemeState, msgs: &mut Vec<Msg>) {
    let t = s.lang();
    row(ui, t.tr("prefs.ui.theme.label"), |ui| {
        ui.horizontal(|ui| {
            let shown = selected_text(t, &s.theme, &themes.catalog);
            egui::ComboBox::from_id_salt("theme").selected_text(shown).width(230.0).show_ui(ui, |ui| {
                for choice in ThemeChoice::BUILT_IN {
                    let label = theme_names::choice(t, &choice);
                    ui.selectable_value(&mut s.theme, choice, label);
                }
                ui.separator();
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

/// The dropdown's text: a built-in's name, a file's name, or "<id> (missing)".
fn selected_text(t: Lang, choice: &ThemeChoice, cat: &ThemeCatalog) -> String {
    match choice {
        ThemeChoice::Custom(id) => cat.get(id).map_or_else(|| t.trf("prefs.ui.theme.missing", &[("id", id)]), |f| f.name.clone()),
        other => theme_names::choice(t, other),
    }
}

fn line(ui: &mut Ui, text: String, c: egui::Color32) {
    ui.add(egui::Label::new(RichText::new(text).size(type_scale::SMALL).color(c)).wrap());
}

/// The folder's error, and the selected file's problems (fatal ones red) or why it is missing.
/// The problems and the error come from trellis-core as they are.
fn problems(ui: &mut Ui, t: Lang, choice: &ThemeChoice, cat: &ThemeCatalog) {
    if let Some(e) = &cat.error {
        line(ui, e.clone(), color::removed());
    }
    let ThemeChoice::Custom(id) = choice else { return };
    let Some(f) = cat.get(id) else {
        let dark = theme_names::builtin(t, BuiltinTheme::Dark);
        line(ui, t.trf("prefs.ui.theme.not_in_folder", &[("file", &format!("{id}.json")), ("theme", &dark)]), color::warn());
        return;
    };
    for p in &f.problems {
        line(ui, format!("{}: {p}", f.file_name()), if p.is_fatal() { color::removed() } else { color::warn() });
    }
    line(ui, t.trf("prefs.ui.theme.base", &[("theme", &theme_names::builtin(t, f.base))]), color::weak());
}
