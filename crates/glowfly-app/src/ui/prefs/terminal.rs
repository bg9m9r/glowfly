//! Preferences > Terminal. Built-in terminal: the shell new terminals start, the text size, its
//! colours (`terminal_colours.rs`), the cursor, dimming without the keyboard, copy on select, middle-click paste (shown only on
//! Linux, where it acts), and whether Open in terminal opens a tab there. External terminal: the
//! program Open in terminal starts otherwise, with its arguments. Edits apply at once (the view
//! edits the settings in place and the shell saves them); a new shell choice applies to
//! terminals started afterwards, and Browse… asks update to show a file dialog.

use super::terminal_colours::colours_row;
use super::tools::{program_field, warnings};
use super::{check_row, help, row, section, text_field, warn};
use crate::msg::Msg;
use crate::themes::ThemeState;
use crate::tools::ToolKind;
use crate::ui::theme::controls;
use crate::ui::widgets::buttons;
use eframe::egui::{self, Ui};
use glowfly_core::external_tools::{self, EDITOR_PLACEHOLDERS};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::{CursorStyle, Settings, ShellChoice, TERMINAL_FONT_SIZES};

pub fn terminal_page(ui: &mut Ui, s: &mut Settings, themes: &ThemeState, msgs: &mut Vec<Msg>) {
    page_for(ui, s, themes, cfg!(target_os = "linux"), msgs);
}

/// The page as drawn on Linux (`linux`, with the middle-click row) or elsewhere.
pub(crate) fn page_for(ui: &mut Ui, s: &mut Settings, themes: &ThemeState, linux: bool, msgs: &mut Vec<Msg>) {
    let t = s.lang();
    section(ui, t.tr("prefs.terminal.builtin.section"));
    built_in(ui, t, s, themes, linux, msgs);
    section(ui, t.tr("prefs.terminal.external.section"));
    external(ui, t, s, msgs);
}

/// The built-in terminal's rows, ending with its Reset (which resets only these).
fn built_in(ui: &mut Ui, t: Lang, s: &mut Settings, themes: &ThemeState, linux: bool, msgs: &mut Vec<Msg>) {
    let windows = cfg!(windows);
    let p = &mut s.terminal;
    row(ui, t.tr("prefs.terminal.shell.label"), |ui| {
        // A Windows-only choice in a settings file from elsewhere acts as, and shows as, Automatic.
        let shown = if ShellChoice::offered(windows).contains(&p.shell.choice) { p.shell.choice } else { ShellChoice::Auto };
        egui::ComboBox::from_id_salt("terminal-shell").selected_text(choice_label(t, shown, windows)).width(super::fit(ui, 240.0)).show_ui(
            ui,
            |ui| {
                for c in ShellChoice::offered(windows) {
                    ui.selectable_value(&mut p.shell.choice, *c, choice_label(t, *c, windows));
                }
            },
        );
        help(ui, if windows { t.tr("prefs.terminal.shell.help_windows") } else { t.tr("prefs.terminal.shell.help_unix") });
    });
    if p.shell.choice == ShellChoice::Custom {
        row(ui, t.tr("prefs.terminal.program.label"), |ui| {
            text_field(ui, &mut p.shell.program, t.tr("prefs.tools.program_hint"));
        });
        row(ui, t.tr("prefs.terminal.args.label"), |ui| {
            text_field(ui, &mut p.shell.args, "");
            help(ui, t.tr("prefs.terminal.args.help"));
            if external_tools::split(&p.shell.args).is_err() {
                warn(ui, t.tr("prefs.tools.warn.unmatched_quote"));
            }
        });
    }
    row(ui, t.tr("prefs.terminal.font_size.label"), |ui| {
        let size = egui::DragValue::new(&mut p.font_size).range(TERMINAL_FONT_SIZES).speed(0.25).max_decimals(1);
        ui.add(size.suffix(t.tr("prefs.terminal.font_size.suffix")));
        help(ui, t.tr("prefs.terminal.font_size.help"));
    });
    colours_row(ui, t, &mut p.colours, themes, msgs);
    row(ui, t.tr("prefs.terminal.cursor.label"), |ui| {
        ui.horizontal(|ui| {
            controls::radio_value(ui, &mut p.cursor, CursorStyle::Block, t.tr("prefs.terminal.cursor.block"));
            controls::radio_value(ui, &mut p.cursor, CursorStyle::Underline, t.tr("prefs.terminal.cursor.underline"));
            controls::radio_value(ui, &mut p.cursor, CursorStyle::Bar, t.tr("prefs.terminal.cursor.bar"));
        });
        help(ui, t.tr("prefs.terminal.cursor.help"));
    });
    let (label, check, text) = (t.tr("prefs.terminal.dim.label"), t.tr("prefs.terminal.dim.check"), t.tr("prefs.terminal.dim.help"));
    check_row(ui, label, &mut p.dim_unfocused, check, Some(text));
    let (label, check, text) = (
        t.tr("prefs.terminal.copy_on_select.label"),
        t.tr("prefs.terminal.copy_on_select.check"),
        t.tr("prefs.terminal.copy_on_select.help"),
    );
    check_row(ui, label, &mut p.copy_on_select, check, Some(text));
    if linux {
        let (label, check, text) = (
            t.tr("prefs.terminal.middle_click.label"),
            t.tr("prefs.terminal.middle_click.check"),
            t.tr("prefs.terminal.middle_click.help"),
        );
        check_row(ui, label, &mut p.middle_click_paste, check, Some(text));
    }
    let (label, check, text) =
        (t.tr("prefs.terminal.open_in_panel.label"), t.tr("prefs.terminal.open_in_panel.check"), t.tr("prefs.terminal.open_in_panel.help"));
    check_row(ui, label, &mut p.open_in_panel, check, Some(text));
    row(ui, t.tr("prefs.terminal.reset.label"), |ui| {
        if ui.add(buttons::secondary(t.tr("prefs.terminal.reset.button"))).clicked() {
            p.reset_page();
        }
    });
}

/// The external terminal: its program (with Browse…) and arguments, which wait for a program.
fn external(ui: &mut Ui, t: Lang, s: &mut Settings, msgs: &mut Vec<Msg>) {
    let et = &mut s.external_tools;
    row(ui, t.tr("prefs.tools.terminal.label"), |ui| {
        program_field(ui, t, &mut et.terminal, t.tr("prefs.tools.terminal.hint"), ToolKind::Terminal, msgs);
        help(ui, t.tr("prefs.tools.terminal.help"));
    });
    let custom = external_tools::program(&et.terminal).is_some();
    row(ui, t.tr("prefs.tools.terminal_args.label"), |ui| {
        ui.add_enabled_ui(custom, |ui| {
            text_field(ui, &mut et.terminal_args, "");
        });
        help(ui, t.tr("prefs.tools.terminal_args.help"));
        warnings(ui, t, &et.terminal_args, &EDITOR_PLACEHOLDERS, &[]);
    });
}

/// A shell choice's name in the menu; bash on Windows is Git Bash.
pub(crate) fn choice_label(t: Lang, c: ShellChoice, windows: bool) -> &'static str {
    match c {
        ShellChoice::Auto => t.tr("prefs.terminal.shell.auto"),
        ShellChoice::Pwsh => t.tr("prefs.terminal.shell.pwsh"),
        ShellChoice::WindowsPowerShell => t.tr("prefs.terminal.shell.windows_powershell"),
        ShellChoice::Cmd => t.tr("prefs.terminal.shell.cmd"),
        ShellChoice::Bash if windows => t.tr("prefs.terminal.shell.git_bash"),
        ShellChoice::Bash => t.tr("prefs.terminal.shell.bash"),
        ShellChoice::Custom => t.tr("prefs.terminal.shell.custom"),
    }
}
