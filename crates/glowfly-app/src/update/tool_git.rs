//! git config's diff and merge tools, used when Preferences > External tools sets no program:
//! a click reads git config first (`read_then`), and its result runs the tool git names, opens
//! Preferences when it names none, or says why it can't run. The External tools page reads
//! them too (`read_for_page`), to show what an empty field means. A tool that needs trust (a
//! command from the repository's own config, or a built-in tool run by git) never runs while
//! git is held: the diff asks for trust, and the merge job waits for the trust prompt. The
//! rules and texts are in state/tool_git.rs; the runner side is runner/tools/git_tool.rs.

use super::tools::{fail, fail_to_prefs, show_page};
use crate::effect::Effect;
use crate::msg::{Msg, TrustMsg};
use crate::state::AppState;
use crate::state::tool_compare::{ToolSetup, merge_label};
use crate::state::tool_git::{held_text, inactive_text, read_failed_text, unknown_text};
use crate::tools::{GitDiffRequest, GitMergeRequest, GitToolNext, ToolEffect};
use glowfly_core::external_tools::git_config::{GitToolRun, GitTools, ToolMode};
use glowfly_core::paths::same_spelling;
use glowfly_core::repo::DiffTarget;
use std::path::PathBuf;
use std::time::Instant;

/// Reads git config's tools in the active repository, then runs `next` with them.
pub(super) fn read_then(st: &mut AppState, next: GitToolNext, fx: &mut Vec<Effect>) {
    let Some(w) = st.workdir() else { return };
    fx.push(Effect::Tool(ToolEffect::ReadGitTools { cwd: w, has_repo: true, next: Some(next) }));
}

/// Reads git config's tools for the External tools page. `git config` runs nothing, so this
/// reads a held repository too.
pub(super) fn read_for_page(st: &mut AppState, fx: &mut Vec<Effect>) {
    fx.push(Effect::Tool(ToolEffect::ReadGitTools { cwd: st.any_dir(), has_repo: st.snap.is_some(), next: None }));
}

/// git config's tools were read in `cwd`: kept for the External tools page, and run for the
/// click that waited for them.
pub(super) fn on_read(
    st: &mut AppState,
    cwd: PathBuf,
    tools: Result<Box<GitTools>, String>,
    next: Option<GitToolNext>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    if st.prefs.is_some() && same_spelling(&cwd, &st.any_dir()) {
        match &tools {
            Ok(t) => {
                if let Some(p) = st.prefs.as_mut() {
                    p.git_tools = Some((**t).clone());
                }
            }
            Err(e) => st.log_quiet(false, format!("Could not read git config's diff and merge tools: {e}")),
        }
    }
    let Some(next) = next else { return };
    let mode = match next {
        GitToolNext::Diff(_) => ToolMode::Diff,
        GitToolNext::Merge(_) => ToolMode::Merge,
    };
    let Some(w) = st.workdir().filter(|w| same_spelling(w, &cwd)) else {
        st.log(false, inactive_text(mode));
        return;
    };
    let tools = match tools {
        Ok(t) => *t,
        Err(e) => return fail(st, read_failed_text(mode, &e), now),
    };
    match next {
        GitToolNext::Diff(target) => open_diff_with(st, w, tools, target, now, fx),
        GitToolNext::Merge(path) => resolve_with(st, tools, path, now, fx),
    }
}

/// Opens Preferences > External tools for a click git config names no tool for, showing what
/// was read.
fn show_page_with(st: &mut AppState, tools: GitTools, now: Instant, fx: &mut Vec<Effect>) {
    show_page(st, now, fx);
    if let Some(p) = st.prefs.as_mut() {
        p.git_tools = Some(tools);
    }
}

/// Open in external diff tool with git config's diff tool.
fn open_diff_with(st: &mut AppState, w: PathBuf, tools: GitTools, target: DiffTarget, now: Instant, fx: &mut Vec<Effect>) {
    // A program set in Preferences while git config was read wins.
    if ToolSetup::of(st.settings.lang(), &st.settings.external_tools).diff.is_none() {
        return super::tool_compare::open_diff(st, target, now, fx);
    }
    let Some(tool) = tools.diff.clone() else { return show_page_with(st, tools, now, fx) };
    if tool.run == GitToolRun::Unknown {
        return fail_to_prefs(st, unknown_text(ToolMode::Diff, &tool), now);
    }
    if tool.needs_trust() && !st.runs_git_unasked() {
        st.log(false, held_text(ToolMode::Diff, &tool));
        if let Some(real) = st.snap.as_ref().map(|s| s.real_workdir.clone()) {
            fx.extend(super::update(st, Msg::Trust(TrustMsg::Ask { real }), now));
        }
        return;
    }
    // A tool that needs trust only runs in a trusted repository, so its copies may use filters.
    let filters = tool.needs_trust() || st.runs_git_unasked();
    fx.push(Effect::Tool(ToolEffect::OpenGitDiff(GitDiffRequest { tool, workdir: w, target, filters })));
}

/// Resolve in external merge tool with git config's merge tool, as the active job. One that
/// needs trust waits for the trust prompt (`jobs::begin_job`); the user's own command never
/// does (`jobs::begin_tool_job`).
fn resolve_with(st: &mut AppState, tools: GitTools, path: String, now: Instant, fx: &mut Vec<Effect>) {
    if ToolSetup::of(st.settings.lang(), &st.settings.external_tools).merge.is_none() {
        return super::tool_compare::resolve(st, path, now, fx);
    }
    if !conflicted(st, &path) {
        st.log(false, format!("{path} is not conflicted"));
        return;
    }
    let Some(tool) = tools.merge.clone() else { return show_page_with(st, tools, now, fx) };
    if tool.run == GitToolRun::Unknown {
        return fail_to_prefs(st, unknown_text(ToolMode::Merge, &tool), now);
    }
    if super::jobs::busy(st, false) {
        return;
    }
    let label = merge_label(&path);
    let needs_trust = tool.needs_trust();
    let filters = needs_trust || st.runs_git_unasked();
    let effect = move |cwd| Effect::Tool(ToolEffect::RunGitMerge(GitMergeRequest { tool, workdir: cwd, path, filters }));
    if needs_trust {
        super::jobs::begin_job(st, &label, true, effect, fx);
    } else {
        super::jobs::begin_tool_job(st, &label, effect, fx);
    }
}

/// Whether `path` is conflicted in the active repository's status.
fn conflicted(st: &AppState, path: &str) -> bool {
    st.snap.as_ref().is_some_and(|s| s.unstaged.iter().any(|f| f.status == '!' && f.path == path))
}
