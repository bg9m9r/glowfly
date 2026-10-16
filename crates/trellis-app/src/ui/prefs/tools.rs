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
use trellis_core::i18n::Lang;
use trellis_core::settings::{ExternalTools, Settings};

const DIFF_TOOL_ARGS: &str = ExternalTools::DIFF_TOOL_ARGS;
const MERGE_TOOL_ARGS: &str = ExternalTools::MERGE_TOOL_ARGS;

pub fn tools_page(ui: &mut Ui, s: &mut Settings, msgs: &mut Vec<Msg>) {
    let t = s.lang();
    let et = &mut s.external_tools;
    section(ui, t.tr("prefs.tools.editor.section"));
    row(ui, t.tr("prefs.tools.editor.label"), |ui| {
        program_field(ui, t, &mut et.editor, t.tr("prefs.tools.program_hint"), ToolKind::Editor, msgs);
        help(ui, t.tr("prefs.tools.editor.help"));
        if bundle_warning(&et.editor) {
            warn(ui, t.tr("prefs.tools.bundle_warn"));
        }
    });
    row(ui, t.tr("prefs.tools.editor_file_args.label"), |ui| {
        text_field(ui, &mut et.editor_file_args, "{path}");
        help(ui, t.tr("prefs.tools.placeholder_help"));
        warnings(ui, t, &et.editor_file_args, &EDITOR_PLACEHOLDERS, &[]);
    });
    row(ui, t.tr("prefs.tools.editor_repo_args.label"), |ui| {
        text_field(ui, &mut et.editor_repo_args, "{repo}");
        help(ui, t.tr("prefs.tools.editor_repo_args.help"));
        warnings(ui, t, &et.editor_repo_args, &EDITOR_PLACEHOLDERS, &[]);
    });
    row(ui, t.tr("prefs.tools.arguments.label"), |ui| {
        if ui.button(t.tr("prefs.tools.restore.button")).clicked() {
            reset(s);
        }
    });

    let et = &mut s.external_tools;
    section(ui, t.tr("prefs.tools.terminal.section"));
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
    diff_section(ui, t, s, msgs);
    merge_section(ui, t, s, msgs);
}

/// The diff tool: its program, its arguments and their own Restore default arguments.
fn diff_section(ui: &mut Ui, t: Lang, s: &mut Settings, msgs: &mut Vec<Msg>) {
    let et = &mut s.external_tools;
    section(ui, t.tr("prefs.tools.diff.section"));
    row(ui, t.tr("prefs.tools.diff.label"), |ui| {
        program_field(ui, t, &mut et.diff_tool, t.tr("prefs.tools.program_hint"), ToolKind::DiffTool, msgs);
        help(ui, t.tr("prefs.tools.diff.help"));
        if bundle_warning(&et.diff_tool) {
            warn(ui, t.tr("prefs.tools.bundle_warn"));
        }
    });
    row(ui, t.tr("prefs.tools.diff_args.label"), |ui| {
        text_field(ui, &mut et.diff_tool_args, DIFF_TOOL_ARGS);
        help(ui, t.tr("prefs.tools.diff_help"));
        warnings(ui, t, &et.diff_tool_args, &DIFF_PLACEHOLDERS, &["{local}", "{remote}"]);
    });
    row(ui, t.tr("prefs.tools.arguments.label"), |ui| {
        if ui.button(t.tr("prefs.tools.restore.button")).clicked() {
            reset_diff(s);
        }
    });
}

/// The merge tool, laid out as the diff tool.
fn merge_section(ui: &mut Ui, t: Lang, s: &mut Settings, msgs: &mut Vec<Msg>) {
    let et = &mut s.external_tools;
    section(ui, t.tr("prefs.tools.merge.section"));
    row(ui, t.tr("prefs.tools.merge.label"), |ui| {
        program_field(ui, t, &mut et.merge_tool, t.tr("prefs.tools.program_hint"), ToolKind::MergeTool, msgs);
        help(ui, t.tr("prefs.tools.merge.help"));
        if bundle_warning(&et.merge_tool) {
            warn(ui, t.tr("prefs.tools.bundle_warn"));
        }
    });
    row(ui, t.tr("prefs.tools.merge_args.label"), |ui| {
        text_field(ui, &mut et.merge_tool_args, MERGE_TOOL_ARGS);
        help(ui, t.tr("prefs.tools.merge_help"));
        warnings(ui, t, &et.merge_tool_args, &MERGE_PLACEHOLDERS, &["{ours}", "{theirs}", "{output}"]);
    });
    row(ui, t.tr("prefs.tools.arguments.label"), |ui| {
        if ui.button(t.tr("prefs.tools.restore.button")).clicked() {
            reset_merge(s);
        }
    });
}

/// A program's text field and its Browse… button.
fn program_field(ui: &mut Ui, t: Lang, value: &mut String, hint: &str, kind: ToolKind, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        text_field(ui, value, hint);
        if ui.button(t.tr("prefs.tools.browse.button")).clicked() {
            msgs.push(Msg::Tools(ToolsMsg::Browse(kind)));
        }
    });
}

/// The warnings under an arguments field whose placeholders are `known`, and which should name
/// each of `wanted`.
fn warnings(ui: &mut Ui, t: Lang, template: &str, known: &[&str], wanted: &[&str]) {
    for w in template_warnings(t, template, known).into_iter().chain(missing_warnings(t, template, wanted)) {
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
