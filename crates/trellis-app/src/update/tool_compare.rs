//! The external diff and merge tools: opening a diff in the diff tool (an unscoped effect that
//! reports only failures), and resolving a conflict in the merge tool as the single active job,
//! ended through `ops::finish_job` when the program exits. The rules are in
//! state/tool_compare.rs; the runner side is runner/tools/compare.rs.

use super::tools::{fail, fail_to_prefs, prefs_action};
use crate::effect::Effect;
use crate::msg::ConflictMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::ToastAction;
use crate::state::tool_compare::{NO_DIFF_TOOL, NO_MERGE_TOOL, diff_request, merge_end_text, merge_label, merge_request, not_found};
use crate::tools::{MergeEnd, MergeToolRequest, ToolEffect};
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::paths::same_spelling;
use trellis_core::repo::DiffTarget;

/// Open in external diff tool. Copies go through git's checkout filters only while the
/// repository is trusted.
pub(super) fn open_diff(st: &mut AppState, target: DiffTarget, now: Instant, fx: &mut Vec<Effect>) {
    let Some(w) = st.workdir() else { return };
    match diff_request(&st.settings.external_tools, &w, &target, st.runs_git_unasked()) {
        Ok(req) => fx.push(Effect::Tool(ToolEffect::OpenDiff(req))),
        Err(text) if text == NO_DIFF_TOOL => fail_to_prefs(st, text, now),
        Err(text) => fail(st, text, now),
    }
}

/// Resolve in external merge tool: runs the tool on the conflicted `path` as the active job,
/// so nothing else (a commit, continuing the merge) starts while it is open. The trust prompt
/// doesn't hold it (see `jobs::begin_tool_job`).
pub(super) fn resolve(st: &mut AppState, path: String, now: Instant, fx: &mut Vec<Effect>) {
    let Some(w) = st.workdir() else { return };
    let conflicted = st.snap.as_ref().is_some_and(|s| s.unstaged.iter().any(|f| f.status == '!' && f.path == path));
    if !conflicted {
        st.log(false, format!("{path} is not conflicted"));
        return;
    }
    let req = match merge_request(&st.settings.external_tools, &w, &path, st.runs_git_unasked()) {
        Ok(r) => r,
        Err(text) if text == NO_MERGE_TOOL => return fail_to_prefs(st, text, now),
        Err(text) => return fail(st, text, now),
    };
    if super::jobs::busy(st, false) {
        return;
    }
    let label = merge_label(&path);
    super::jobs::begin_tool_job(st, &label, move |cwd| Effect::Tool(ToolEffect::RunMerge(MergeToolRequest { workdir: cwd, ..req })), fx);
}

/// The merge tool exited: ends the job (re-reading status when its tab is active), with a
/// Mark resolved button when it saved the file, or Open Preferences when it wasn't found. A
/// conflict view open on the file closes when the tool changed it: its picks would write over
/// the tool's result.
pub(super) fn exited(
    st: &mut AppState,
    workdir: PathBuf,
    path: String,
    program: String,
    end: MergeEnd,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    let (ok, output, headline) = merge_end_text(&program, &path, &end);
    if matches!(end, MergeEnd::Saved | MergeEnd::Resolved | MergeEnd::Exited { changed: true, .. }) {
        close_view(st, &workdir, &path);
    }
    if st.job.is_none() {
        // No job to end (it was ended some other way): only say what happened.
        st.log(ok, headline);
        return;
    }
    super::ops::finish_job(st, ok, output, Some(headline), None, now, fx);
    if end == MergeEnd::Saved {
        let msg = Box::new(Msg::Conflict(ConflictMsg::MarkResolved(vec![path])));
        st.set_last_toast_action(ToastAction { label: "Mark resolved".into(), msg, workdir: Some(workdir) });
    } else if not_found(&end) {
        st.set_last_toast_action(prefs_action());
    }
}

/// Closes the conflict view on `path` of `workdir`, in the active tab or the parked one.
fn close_view(st: &mut AppState, workdir: &Path, path: &str) {
    let here = st.workdir().is_some_and(|w| same_spelling(&w, workdir));
    if here {
        if st.conflict.as_ref().is_some_and(|c| c.path == path) {
            super::conflict::close(st);
        }
    } else if let Some(parked) = st.tabs.iter_mut().find(|t| t.is(workdir)).and_then(|t| t.parked.as_mut()) {
        parked.conflict.take_if(|c| c.path == path);
    }
}
