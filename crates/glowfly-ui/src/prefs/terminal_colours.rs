//! Preferences > Terminal > Colours: Match app theme, a built-in theme's terminal colours or an
//! imported scheme, with Import scheme… (ThemeMsg::PickScheme), and quiet notes under it: a
//! scheme that is gone, a scheme file's problems, and the colours that are hard to read on its
//! background (the text rules of `theme/ansi.rs`). A hard-to-read scheme can still be used.

use super::{fit, help, row};
use crate::msg::Msg;
use crate::state::theme_names;
use crate::themes::{ThemeMsg, ThemeState};
use crate::ui::theme::{ansi, color, terminal_scheme, type_scale};
use crate::ui::widgets::buttons;
use eframe::egui::{self, RichText, Ui};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::terminal_schemes::SchemeCatalog;
use glowfly_core::settings::{BuiltinTheme, TerminalColours};

pub(super) fn colours_row(ui: &mut Ui, t: Lang, choice: &mut TerminalColours, themes: &ThemeState, msgs: &mut Vec<Msg>) {
    let schemes = &themes.catalog.terminal;
    row(ui, t.tr("prefs.terminal.colours.label"), |ui| {
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("terminal-colours")
                .selected_text(selected_text(t, choice, schemes))
                .width(fit(ui, 240.0))
                .show_ui(ui, |ui| {
                    ui.selectable_value(choice, TerminalColours::MatchTheme, t.tr("prefs.terminal.colours.match"));
                    ui.separator();
                    for b in BuiltinTheme::ALL {
                        ui.selectable_value(choice, TerminalColours::Theme(b.id().to_owned()), theme_names::builtin(t, b));
                    }
                    ui.separator();
                    if schemes.files.is_empty() {
                        ui.label(RichText::new(t.tr("prefs.terminal.colours.no_schemes")).color(color::weak()));
                    }
                    for f in &schemes.files {
                        ui.selectable_value(choice, TerminalColours::Scheme(f.id.clone()), f.name.as_str());
                    }
                });
            if ui.add(buttons::secondary(t.tr("prefs.terminal.colours.import.button"))).clicked() {
                msgs.push(Msg::Theme(ThemeMsg::PickScheme));
            }
        });
        help(ui, t.tr("prefs.terminal.colours.help"));
        notes(ui, t, choice, schemes);
    });
}

/// The dropdown's text: Match app theme, a built-in's name, a scheme's name or "<id> (missing)".
fn selected_text(t: Lang, choice: &TerminalColours, schemes: &SchemeCatalog) -> String {
    match choice {
        TerminalColours::MatchTheme => t.tr("prefs.terminal.colours.match").to_owned(),
        TerminalColours::Theme(id) => BuiltinTheme::from_id(id)
            .map_or_else(|| t.tr("prefs.terminal.colours.match").to_owned(), |b| theme_names::builtin(t, b).to_owned()),
        TerminalColours::Scheme(id) => {
            schemes.get(id).map_or_else(|| t.trf("prefs.terminal.colours.missing", &[("id", id)]), |f| f.name.clone())
        }
    }
}

fn line(ui: &mut Ui, text: String, c: egui::Color32) {
    ui.add(egui::Label::new(RichText::new(text).size(type_scale::SMALL).color(c)).wrap());
}

/// The chosen scheme's problems, or that it is gone, and what is hard to read in the colours the
/// terminal will draw in.
fn notes(ui: &mut Ui, t: Lang, choice: &TerminalColours, schemes: &SchemeCatalog) {
    if let Some(e) = &schemes.error {
        line(ui, e.clone(), color::removed());
    }
    if let TerminalColours::Scheme(id) = choice {
        match schemes.get(id) {
            None => line(ui, t.trf("prefs.terminal.colours.not_in_folder", &[("file", &format!("{id}.json"))]), color::warn()),
            Some(f) => {
                for p in &f.problems {
                    line(ui, format!("{}: {p}", f.file_name()), color::warn());
                }
            }
        }
    }
    let shown = terminal_scheme::resolve(choice, &color::current(), schemes);
    let hard = ansi::hard_to_read(&shown);
    if !hard.is_empty() {
        let names: Vec<&str> = hard.iter().map(|i| colour_name(t, *i)).collect();
        line(ui, t.trf("prefs.terminal.colours.hard_to_read", &[("colours", &names.join(", "))]), color::warn());
    }
}

/// The name of the foreground (None) or basic colour `i` (0-15), in the language.
pub(crate) fn colour_name(t: Lang, i: Option<usize>) -> &'static str {
    match i {
        None => t.tr("prefs.terminal.colours.name.foreground"),
        Some(0) => t.tr("prefs.terminal.colours.name.black"),
        Some(1) => t.tr("prefs.terminal.colours.name.red"),
        Some(2) => t.tr("prefs.terminal.colours.name.green"),
        Some(3) => t.tr("prefs.terminal.colours.name.yellow"),
        Some(4) => t.tr("prefs.terminal.colours.name.blue"),
        Some(5) => t.tr("prefs.terminal.colours.name.magenta"),
        Some(6) => t.tr("prefs.terminal.colours.name.cyan"),
        Some(7) => t.tr("prefs.terminal.colours.name.white"),
        Some(8) => t.tr("prefs.terminal.colours.name.bright_black"),
        Some(9) => t.tr("prefs.terminal.colours.name.bright_red"),
        Some(10) => t.tr("prefs.terminal.colours.name.bright_green"),
        Some(11) => t.tr("prefs.terminal.colours.name.bright_yellow"),
        Some(12) => t.tr("prefs.terminal.colours.name.bright_blue"),
        Some(13) => t.tr("prefs.terminal.colours.name.bright_magenta"),
        Some(14) => t.tr("prefs.terminal.colours.name.bright_cyan"),
        Some(_) => t.tr("prefs.terminal.colours.name.bright_white"),
    }
}
