//! Preferences > External Tools: the editor that opens files and the repository, a custom
//! terminal for Open in terminal, the diff tool that compares a file's versions, and the merge
//! tool that resolves a conflict. Each tool's arguments warn about an unmatched quote, names
//! that are not its placeholders and (diff and merge tools) the versions they leave out. Edits
//! apply at once (the shell saves the settings when they change); Browse… asks update to show
//! a file dialog.

use super::appearance::section;
use super::{help, row, text_field, warn};
use crate::msg::Msg;
use crate::state::tools::{bundle_warning, missing_warnings, template_warnings};
use crate::tools::{ToolKind, ToolsMsg};
use eframe::egui::Ui;
use trellis_core::external_tools::{self, DIFF_PLACEHOLDERS, EDITOR_PLACEHOLDERS, MERGE_PLACEHOLDERS};
use trellis_core::settings::{ExternalTools, Settings};

const DIFF_TOOL_ARGS: &str = ExternalTools::DIFF_TOOL_ARGS;
const MERGE_TOOL_ARGS: &str = ExternalTools::MERGE_TOOL_ARGS;

const DIFF_HELP: &str = "{local} is the older version and {remote} the newer one. For unstaged changes {remote} is the working file itself, so edits saved there are kept. Every other version is a read-only copy in a temporary folder. {path} is the file's full path in the working tree (it may not exist there) and {repo} is the repository folder. Double quotes keep text with spaces together, and the arguments go straight to the program, without a shell.";
const MERGE_HELP: &str = "{base} is the common ancestor (an empty file when both sides added the file), {ours} is your side, {theirs} is the side being merged in, {output} is the working file the result is saved to, and {repo} is the repository folder. Trellis waits until the program exits, so a program that hands off to a window that is already open needs its wait option. Console programs aren't supported.";
const BUNDLE_WARNING: &str =
    "An application bundle opens files, but extra options may be dropped. Choosing the program's command-line tool works best.";

const PLACEHOLDER_HELP: &str = "{path} is the file's full path, {line} its line number (1 when no line is chosen) and {repo} the repository folder. Double quotes keep text with spaces together; there is no escape character, so backslashes in Windows paths are fine. The arguments go straight to the program, without a shell.";

pub fn tools_page(ui: &mut Ui, s: &mut Settings, msgs: &mut Vec<Msg>) {
    let t = &mut s.external_tools;
    section(ui, "Editor");
    row(ui, "Editor executable", |ui| {
        program_field(ui, &mut t.editor, "Program name on your PATH, or a full path", ToolKind::Editor, msgs);
        help(ui, "The program Trellis starts to open files and repositories. Leave it empty for none.");
        if bundle_warning(&t.editor) {
            warn(ui, BUNDLE_WARNING);
        }
    });
    row(ui, "Open file arguments", |ui| {
        text_field(ui, &mut t.editor_file_args, "{path}");
        help(ui, PLACEHOLDER_HELP);
        warnings(ui, &t.editor_file_args, &EDITOR_PLACEHOLDERS, &[]);
    });
    row(ui, "Open repository arguments", |ui| {
        text_field(ui, &mut t.editor_repo_args, "{repo}");
        help(ui, "{repo} (and {path}) is the repository folder.");
        warnings(ui, &t.editor_repo_args, &EDITOR_PLACEHOLDERS, &[]);
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
        warnings(ui, &t.terminal_args, &EDITOR_PLACEHOLDERS, &[]);
    });
    diff_section(ui, s, msgs);
    merge_section(ui, s, msgs);
}

/// The diff tool: its program, its arguments and their own Restore default arguments.
fn diff_section(ui: &mut Ui, s: &mut Settings, msgs: &mut Vec<Msg>) {
    let t = &mut s.external_tools;
    section(ui, "Diff tool");
    row(ui, "Diff tool executable", |ui| {
        program_field(ui, &mut t.diff_tool, "Program name on your PATH, or a full path", ToolKind::DiffTool, msgs);
        help(ui, "The program that compares two versions of a file. Leave it empty for none.");
        if bundle_warning(&t.diff_tool) {
            warn(ui, BUNDLE_WARNING);
        }
    });
    row(ui, "Diff tool arguments", |ui| {
        text_field(ui, &mut t.diff_tool_args, DIFF_TOOL_ARGS);
        help(ui, DIFF_HELP);
        warnings(ui, &t.diff_tool_args, &DIFF_PLACEHOLDERS, &["{local}", "{remote}"]);
    });
    row(ui, "Arguments", |ui| {
        if ui.button("Restore default arguments").clicked() {
            reset_diff(s);
        }
    });
}

/// The merge tool, laid out as the diff tool.
fn merge_section(ui: &mut Ui, s: &mut Settings, msgs: &mut Vec<Msg>) {
    let t = &mut s.external_tools;
    section(ui, "Merge tool");
    row(ui, "Merge tool executable", |ui| {
        program_field(ui, &mut t.merge_tool, "Program name on your PATH, or a full path", ToolKind::MergeTool, msgs);
        help(ui, "The program that resolves a conflicted file. Leave it empty for none.");
        if bundle_warning(&t.merge_tool) {
            warn(ui, BUNDLE_WARNING);
        }
    });
    row(ui, "Merge tool arguments", |ui| {
        text_field(ui, &mut t.merge_tool_args, MERGE_TOOL_ARGS);
        help(ui, MERGE_HELP);
        warnings(ui, &t.merge_tool_args, &MERGE_PLACEHOLDERS, &["{ours}", "{theirs}", "{output}"]);
    });
    row(ui, "Arguments", |ui| {
        if ui.button("Restore default arguments").clicked() {
            reset_merge(s);
        }
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

/// The warnings under an arguments field whose placeholders are `known`, and which should name
/// each of `wanted`.
fn warnings(ui: &mut Ui, template: &str, known: &[&str], wanted: &[&str]) {
    for w in template_warnings(template, known).into_iter().chain(missing_warnings(template, wanted)) {
        warn(ui, &w);
    }
}

/// Restore default arguments: both editor templates, nothing else.
fn reset(s: &mut Settings) {
    s.external_tools.reset_editor_args();
}

/// The diff tool's Restore default arguments: its template only.
fn reset_diff(s: &mut Settings) {
    s.external_tools.reset_diff_tool_args();
}

/// The merge tool's Restore default arguments: its template only.
fn reset_merge(s: &mut Settings) {
    s.external_tools.reset_merge_tool_args();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_restores_only_the_editor_arguments() {
        let tools = ExternalTools {
            editor: "code".into(),
            editor_file_args: "--goto {path}:{line}".into(),
            editor_repo_args: "-n".into(),
            terminal: "wt".into(),
            terminal_args: "-d {repo}".into(),
            diff_tool_args: "{remote}".into(),
            merge_tool_args: "{output}".into(),
            ..ExternalTools::default()
        };
        let mut s = Settings { external_tools: tools, tab_size: 8, ..Settings::default() };
        reset(&mut s);
        let want = ExternalTools {
            editor: "code".into(),
            terminal: "wt".into(),
            terminal_args: "-d {repo}".into(),
            diff_tool_args: "{remote}".into(),
            merge_tool_args: "{output}".into(),
            ..ExternalTools::default()
        };
        assert_eq!(s.external_tools, want);
        assert_eq!(s.tab_size, 8);
        // Each tool's button resets only its own template.
        reset_diff(&mut s);
        assert_eq!(s.external_tools, ExternalTools { diff_tool_args: DIFF_TOOL_ARGS.into(), ..want.clone() });
        reset_merge(&mut s);
        assert_eq!(
            s.external_tools,
            ExternalTools { diff_tool_args: DIFF_TOOL_ARGS.into(), merge_tool_args: MERGE_TOOL_ARGS.into(), ..want }
        );
    }
}
