//! The external editor and a custom terminal: planning each launch from Preferences > External
//! Tools (state/tools.rs), the program pickers, and reporting launches that failed. Repository
//! trust doesn't hold these up: the programs and their arguments come from the user's own
//! settings, never from the repository.

use crate::effect::Effect;
use crate::msg::PrefsMsg;
use crate::state::AppState;
use crate::state::PrefPage;
use crate::state::tools::{editor_file, editor_repo, failure_text, terminal};
use crate::tools::{ToolEffect, ToolKind, ToolsMsg};
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: ToolsMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        ToolsMsg::OpenFile { path, line } => {
            let Some(w) = st.workdir() else { return };
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
            match editor_repo(&st.settings.external_tools, &w) {
                Ok(launch) => fx.push(Effect::Tool(ToolEffect::Launch { kind: ToolKind::Editor, launch, target: name, check: None })),
                Err(text) => fail(st, text, now),
            }
        }
        ToolsMsg::ShowPage => super::prefs::handle(st, PrefsMsg::OpenPrefs(PrefPage::Tools), now, fx),
        ToolsMsg::Browse(kind) => fx.push(Effect::Tool(ToolEffect::PickProgram(kind))),
        ToolsMsg::ProgramPicked { path: None, .. } => {}
        ToolsMsg::ProgramPicked { kind, path: Some(p) } => {
            let value = p.display().to_string();
            let tools = &mut st.settings.external_tools;
            match kind {
                ToolKind::Editor => tools.editor = value,
                ToolKind::Terminal => tools.terminal = value,
            }
            fx.push(super::prefs::save_settings(st));
        }
        ToolsMsg::LaunchFailed { kind, program, target, failure } => fail(st, failure_text(kind, &program, &target, &failure), now),
    }
}

/// Open in terminal: the custom terminal when one is set, else the usual one is detected.
/// A custom terminal that fails is reported, never replaced: the user chose it.
pub(super) fn open_terminal(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
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

/// A failure for the activity log and a red toast.
fn fail(st: &mut AppState, text: String, now: Instant) {
    st.toast(false, text.clone(), now);
    st.log(false, text);
}
