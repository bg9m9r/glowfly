//! Preferences > Appearance > Theme: the built-in themes' cards (`theme_picker.rs`), then the
//! "Custom theme files" disclosure (its header in `theme_files.rs`): the theme files' dropdown,
//! the themes folder's buttons, a short help, and the selected file's problems.

use super::theme_picker::gap;
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
        super::theme_picker::picker(ui, t, s);
        gap(ui, 12.0);
        // A theme file in use keeps the disclosure open.
        let in_use = matches!(s.theme, ThemeChoice::Custom(_));
        if super::theme_files::header(ui, t.tr("prefs.ui.theme.files.section"), in_use, t.tr("prefs.ui.theme.files.pinned")) {
            gap(ui, 8.0);
            files(ui, t, s, themes, msgs);
        }
    });
}

/// The widest the theme cards' column gets: four cards and their gaps.
const PICKER_W: f32 = 660.0;
/// The theme file dropdown's width: the form's combo width.
const COMBO_W: f32 = 230.0;

/// The open disclosure: the dropdown, the folder's buttons, the help and the problems.
fn files(ui: &mut Ui, t: Lang, s: &mut Settings, themes: &ThemeState, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.label(RichText::new(t.tr("prefs.ui.theme.files.label")).size(type_scale::BODY).color(color::text()));
        let shown = selected_text(t, &s.theme, &themes.catalog);
        let combo = egui::ComboBox::from_id_salt("theme").selected_text(shown).width(COMBO_W).show_ui(ui, |ui| {
            if themes.catalog.files.is_empty() {
                ui.label(RichText::new(t.tr("prefs.ui.theme.no_files")).color(color::weak()));
            }
            for f in &themes.catalog.files {
                if ui.selectable_value(&mut s.theme, ThemeChoice::Custom(f.id.clone()), theme_names::file_entry(t, f)).changed() {
                    s.settle_contrast();
                }
            }
        });
        crate::ui::access::from_label(&combo.response);
    });
    gap(ui, 8.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
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
    gap(ui, 6.0);
    help(ui, t.tr("prefs.ui.theme.help"));
    gap(ui, 4.0);
    problems(ui, t, &s.theme, &themes.catalog);
}

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
