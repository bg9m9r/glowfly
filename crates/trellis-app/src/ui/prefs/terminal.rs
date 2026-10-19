//! Preferences > In-App Terminal: the shell new terminals start, the text size, the cursor,
//! dimming without the keyboard, copy on select, middle-click paste (shown only on Linux, where
//! it acts), and whether Open in terminal uses the panel. Edits apply at
//! once (the view edits the settings in place and the shell saves them); a new shell choice
//! applies to terminals started afterwards.

use super::{help, row, text_field, warn};
use crate::ui::theme::controls;
use crate::ui::widgets::buttons;
use eframe::egui::{self, Ui};
use trellis_core::external_tools;
use trellis_core::i18n::Lang;
use trellis_core::settings::{CursorStyle, Settings, ShellChoice, TERMINAL_FONT_SIZES};

pub fn terminal_page(ui: &mut Ui, s: &mut Settings) {
    page_for(ui, s, cfg!(target_os = "linux"));
}

/// The page as drawn on Linux (`linux`, with the middle-click row) or elsewhere.
pub(crate) fn page_for(ui: &mut Ui, s: &mut Settings, linux: bool) {
    let t = s.lang();
    let windows = cfg!(windows);
    let p = &mut s.terminal;
    row(ui, t.tr("prefs.terminal.shell.label"), |ui| {
        // A Windows-only choice in a settings file from elsewhere acts as, and shows as, Automatic.
        let shown = if ShellChoice::offered(windows).contains(&p.shell.choice) { p.shell.choice } else { ShellChoice::Auto };
        egui::ComboBox::from_id_salt("terminal-shell").selected_text(choice_label(t, shown, windows)).width(240.0).show_ui(ui, |ui| {
            for c in ShellChoice::offered(windows) {
                ui.selectable_value(&mut p.shell.choice, *c, choice_label(t, *c, windows));
            }
        });
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
    row(ui, t.tr("prefs.terminal.cursor.label"), |ui| {
        ui.horizontal(|ui| {
            controls::radio_value(ui, &mut p.cursor, CursorStyle::Block, t.tr("prefs.terminal.cursor.block"));
            controls::radio_value(ui, &mut p.cursor, CursorStyle::Underline, t.tr("prefs.terminal.cursor.underline"));
            controls::radio_value(ui, &mut p.cursor, CursorStyle::Bar, t.tr("prefs.terminal.cursor.bar"));
        });
        help(ui, t.tr("prefs.terminal.cursor.help"));
    });
    row(ui, t.tr("prefs.terminal.dim.label"), |ui| {
        controls::checkbox(ui, &mut p.dim_unfocused, "");
        help(ui, t.tr("prefs.terminal.dim.help"));
    });
    row(ui, t.tr("prefs.terminal.copy_on_select.label"), |ui| {
        controls::checkbox(ui, &mut p.copy_on_select, "");
        help(ui, t.tr("prefs.terminal.copy_on_select.help"));
    });
    if linux {
        row(ui, t.tr("prefs.terminal.middle_click.label"), |ui| {
            controls::checkbox(ui, &mut p.middle_click_paste, "");
            help(ui, t.tr("prefs.terminal.middle_click.help"));
        });
    }
    row(ui, t.tr("prefs.terminal.open_in_panel.label"), |ui| {
        controls::checkbox(ui, &mut p.open_in_panel, t.tr("prefs.terminal.open_in_panel.check"));
        help(ui, t.tr("prefs.terminal.open_in_panel.help"));
    });
    row(ui, t.tr("prefs.terminal.reset.label"), |ui| {
        if ui.add(buttons::secondary(t.tr("prefs.terminal.reset.button"))).clicked() {
            p.reset_page();
        }
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
