//! The external editor, a custom terminal and the diff and merge tools: planning each launch
//! from Preferences > External tools (state/tools.rs), the program pickers, and reporting
//! launches that failed; the diff and merge tools' own steps are in update/tool_compare.rs.
//! A tool entry run while its program isn't set opens Preferences > External Tools instead
//! (`show_page`), whoever sent it: a menu, a header, the palette or a stale frame.
//! Repository trust doesn't hold these up: the programs and their arguments come from the
//! user's own settings, never from the repository.

use crate::effect::Effect;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::state::AppState;
use crate::state::PrefPage;
use crate::state::ToastAction;
use crate::state::tool_compare::ToolSetup;
use crate::state::tools::{editor_file, editor_repo, failure_text, terminal};
use crate::tools::{LaunchFailure, ToolEffect, ToolKind, ToolsMsg};
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: ToolsMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        ToolsMsg::OpenFile { path, line } => {
            let Some(w) = st.workdir() else { return };
            if ToolSetup::of(&st.settings.external_tools).editor.is_some() {
                return show_page(st, now, fx);
            }
            // Always absolute, so a file named like an option can't be read as one.
            let file = w.join(&path);
            match editor_file(&st.settings.external_tools, &w, &file, line.unwrap_or(1)) {
                Ok(launch) => fx.push(Effect::Tool(ToolEffect::Launch { kind: ToolKind::Editor, launch, target: path, check: Some(file) })),
                Err(text) => fail(st, text, now),
            }
        }
        ToolsMsg::OpenRepo => {
            let (Some(w), Some(name)) = (st.workdir(), st.snap.as_ref().map(|s| s.name.clone())) else {
                st.log(false, "Open a repository first.");
                return;
            };
            if ToolSetup::of(&st.settings.external_tools).editor.is_some() {
                return show_page(st, now, fx);
            }
            match editor_repo(&st.settings.external_tools, &w) {
                Ok(launch) => fx.push(Effect::Tool(ToolEffect::Launch { kind: ToolKind::Editor, launch, target: name, check: None })),
                Err(text) => fail(st, text, now),
            }
        }
        ToolsMsg::ShowPage => show_page(st, now, fx),
        ToolsMsg::Browse(kind) => fx.push(Effect::Tool(ToolEffect::PickProgram(kind))),
        ToolsMsg::ProgramPicked { path: None, .. } => {}
        ToolsMsg::ProgramPicked { kind, path: Some(p) } => {
            let value = p.display().to_string();
            let tools = &mut st.settings.external_tools;
            match kind {
                ToolKind::Editor => tools.editor = value,
                ToolKind::Terminal => tools.terminal = value,
                ToolKind::DiffTool => tools.diff_tool = value,
                ToolKind::MergeTool => tools.merge_tool = value,
            }
            fx.push(super::prefs::save_settings(st));
        }
        ToolsMsg::LaunchFailed { kind, program, target, failure } => {
            let text = failure_text(kind, &program, &target, &failure);
            if failure == LaunchFailure::NotFound && matches!(kind, ToolKind::DiffTool | ToolKind::MergeTool) {
                fail_to_prefs(st, text, now);
            } else {
                fail(st, text, now);
            }
        }
        ToolsMsg::OpenDiff(target) => super::tool_compare::open_diff(st, target, now, fx),
        ToolsMsg::ResolveInMergeTool(path) => super::tool_compare::resolve(st, path, now, fx),
        ToolsMsg::MergeToolExited { workdir, path, program, end } => super::tool_compare::exited(st, workdir, path, program, end, now, fx),
    }
}

/// Open in terminal: a new tab in the terminal panel when Preferences > Terminal says
/// so; else the custom terminal when one is set, else the usual one is detected. A custom
/// terminal that fails is reported, never replaced: the user chose it.
pub(super) fn open_terminal(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    if st.settings.terminal.open_in_panel && st.snap.is_some() {
        super::terminal_tabs::new_tab(st, None, fx);
        return;
    }
    let Some(w) = st.workdir() else { return };
    match terminal(&st.settings.external_tools, &w) {
        None => fx.push(Effect::OpenTerminal(w)),
        Some(Ok(launch)) => {
            let target = st.snap.as_ref().map(|s| s.name.clone()).unwrap_or_default();
            fx.push(Effect::Tool(ToolEffect::Launch { kind: ToolKind::Terminal, launch, target, check: None }));
        }
        Some(Err(text)) => fail(st, text, now),
    }
}

/// Opens Preferences > External Tools (a tool entry run while its program isn't set).
pub(super) fn show_page(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    super::prefs::handle(st, PrefsMsg::OpenPrefs(PrefPage::Tools), now, fx);
}

/// A failure for the activity log and a red toast.
pub(super) fn fail(st: &mut AppState, text: String, now: Instant) {
    st.toast(false, text.clone(), now);
    st.log(false, text);
}

/// The toast action that opens Preferences > External tools.
pub(super) fn prefs_action() -> ToastAction {
    ToastAction { label: "Open preferences".into(), msg: Box::new(Msg::Tools(ToolsMsg::ShowPage)), workdir: None }
}

/// A failure the settings can fix (a program that wasn't found): the log and a red toast with an
/// Open Preferences button.
pub(super) fn fail_to_prefs(st: &mut AppState, text: String, now: Instant) {
    st.toast_action(false, text.clone(), prefs_action(), now);
    st.log(false, text);
}
