//! Preferences > External Tools: the editor that opens files and the repository, and a custom
//! terminal for Open in terminal. Edits apply at once (the shell saves the settings when they
//! change); Browse… asks update to show a file dialog.

use super::appearance::section;
use super::{help, row, text_field, warn};
use crate::msg::Msg;
use crate::state::tools::{bundle_warning, template_warnings};
use crate::tools::{ToolKind, ToolsMsg};
use eframe::egui::Ui;
use trellis_core::external_tools;
use trellis_core::settings::Settings;

const PLACEHOLDER_HELP: &str = "{path} is the file's full path, {line} its line number (1 when no line is chosen) and {repo} the repository folder. Double quotes keep text with spaces together; there is no escape character, so backslashes in Windows paths are fine. The arguments go straight to the program, without a shell.";

pub fn tools_page(ui: &mut Ui, s: &mut Settings, msgs: &mut Vec<Msg>) {
    let t = &mut s.external_tools;
    section(ui, "Editor");
    row(ui, "Editor executable", |ui| {
        program_field(ui, &mut t.editor, "Program name on your PATH, or a full path", ToolKind::Editor, msgs);
        help(ui, "The program Trellis starts to open files and repositories. Leave it empty for none.");
        if bundle_warning(&t.editor) {
            warn(
                ui,
                "An application bundle opens files, but extra options may be dropped. Choosing the editor's command-line tool works best.",
            );
        }
    });
    row(ui, "Open file arguments", |ui| {
        text_field(ui, &mut t.editor_file_args, "{path}");
        help(ui, PLACEHOLDER_HELP);
        warnings(ui, &t.editor_file_args);
    });
    row(ui, "Open repository arguments", |ui| {
        text_field(ui, &mut t.editor_repo_args, "{repo}");
        help(ui, "{repo} (and {path}) is the repository folder.");
        warnings(ui, &t.editor_repo_args);
    });
    row(ui, "Arguments", |ui| {
        if ui.button("Restore default arguments").clicked() {
            reset(s);
        }
    });

    let t = &mut s.external_tools;
    section(ui, "Terminal");
    row(ui, "Terminal executable", |ui| {
        program_field(ui, &mut t.terminal, "Detect automatically", ToolKind::Terminal, msgs);
        help(
            ui,
            "Leave it empty to use the system's usual terminal. When set, Open in terminal starts this program in the repository folder instead.",
        );
    });
    let custom = external_tools::program(&t.terminal).is_some();
    row(ui, "Terminal arguments", |ui| {
        ui.add_enabled_ui(custom, |ui| {
            text_field(ui, &mut t.terminal_args, "");
        });
        help(ui, "{repo} is the repository folder; the program also starts in that folder.");
        warnings(ui, &t.terminal_args);
    });
}

/// A program's text field and its Browse… button.
fn program_field(ui: &mut Ui, value: &mut String, hint: &str, kind: ToolKind, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        text_field(ui, value, hint);
        if ui.button("Browse…").clicked() {
            msgs.push(Msg::Tools(ToolsMsg::Browse(kind)));
        }
    });
}

fn warnings(ui: &mut Ui, template: &str) {
    for w in template_warnings(template) {
        warn(ui, &w);
    }
}

/// Restore default arguments: both editor templates, nothing else.
fn reset(s: &mut Settings) {
    s.external_tools.reset_editor_args();
}

#[cfg(test)]
mod tests {
    use super::*;
    use trellis_core::settings::ExternalTools;

    #[test]
    fn reset_restores_only_the_editor_arguments() {
        let tools = ExternalTools {
            editor: "code".into(),
            editor_file_args: "--goto {path}:{line}".into(),
            editor_repo_args: "-n".into(),
            terminal: "wt".into(),
            terminal_args: "-d {repo}".into(),
        };
        let mut s = Settings { external_tools: tools, tab_size: 8, ..Settings::default() };
        reset(&mut s);
        let want =
            ExternalTools { editor: "code".into(), terminal: "wt".into(), terminal_args: "-d {repo}".into(), ..ExternalTools::default() };
        assert_eq!(s.external_tools, want);
        assert_eq!(s.tab_size, 8);
    }
}
