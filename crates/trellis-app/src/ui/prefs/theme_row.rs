//! Preferences > UI Customization > Theme: the built-in themes and the theme files, the
//! themes folder's buttons, and the selected file's problems.

use super::{help, row};
use crate::msg::Msg;
use crate::themes::{ThemeMsg, ThemeState};
use crate::ui::theme::{color, custom};
use eframe::egui::{self, RichText, Ui};
use trellis_core::settings::{BuiltinTheme, Settings, ThemeCatalog, ThemeChoice};

const HELP: &str = "Theme files are JSON files in the themes folder. Each one names a base theme (dark, light, \
high-contrast-dark, high-contrast-light or sand) and sets the colors it changes, either as #rrggbb or as the name of \
another color; every color it leaves out comes from the base. Create from current writes a file with every color of \
the theme in use. Press Reload after editing a file.";

pub fn theme_row(ui: &mut Ui, s: &mut Settings, themes: &ThemeState, msgs: &mut Vec<Msg>) {
    row(ui, "Theme", |ui| {
        ui.horizontal(|ui| {
            let shown = selected_text(&s.theme, &themes.catalog);
            egui::ComboBox::from_id_salt("theme").selected_text(shown).width(230.0).show_ui(ui, |ui| {
                for choice in ThemeChoice::BUILT_IN {
                    let label = choice.label();
                    ui.selectable_value(&mut s.theme, choice, label);
                }
                ui.separator();
                if themes.catalog.files.is_empty() {
                    ui.label(RichText::new("No theme files yet").color(color::weak()));
                }
                for f in &themes.catalog.files {
                    let label = if f.problems.is_empty() { f.name.clone() } else { format!("{} (has problems)", f.name) };
                    ui.selectable_value(&mut s.theme, ThemeChoice::Custom(f.id.clone()), label);
                }
            });
            if ui.button("Open themes folder").clicked() {
                msgs.push(Msg::Theme(ThemeMsg::OpenFolder));
            }
            if ui.button("Reload").clicked() {
                msgs.push(Msg::Theme(ThemeMsg::Reload));
            }
            if ui.button("Create from current").clicked() {
                let p = color::current();
                msgs.push(Msg::Theme(ThemeMsg::CreateFromCurrent { colors: custom::starter(&p), dark: p.dark }));
            }
        });
        help(ui, HELP);
        problems(ui, &s.theme, &themes.catalog);
    });
}

/// The dropdown's text: a built-in's label, a file's name, or "<id> (missing)".
fn selected_text(choice: &ThemeChoice, cat: &ThemeCatalog) -> String {
    match choice {
        ThemeChoice::Custom(id) => cat.get(id).map_or_else(|| format!("{id} (missing)"), |f| f.name.clone()),
        other => other.label(),
    }
}

fn line(ui: &mut Ui, text: String, c: egui::Color32) {
    ui.add(egui::Label::new(RichText::new(text).size(12.0).color(c)).wrap());
}

/// The folder's error, and the selected file's problems (fatal ones red) or why it is missing.
fn problems(ui: &mut Ui, choice: &ThemeChoice, cat: &ThemeCatalog) {
    if let Some(e) = &cat.error {
        line(ui, e.clone(), color::red());
    }
    let ThemeChoice::Custom(id) = choice else { return };
    let Some(f) = cat.get(id) else {
        line(ui, format!("{id}.json is not in the themes folder; showing {}.", BuiltinTheme::Dark.label()), color::amber());
        return;
    };
    for p in &f.problems {
        line(ui, format!("{}: {p}", f.file_name()), if p.is_fatal() { color::red() } else { color::amber() });
    }
    line(ui, format!("Colors not set come from {}.", f.base.label()), color::weak());
}
