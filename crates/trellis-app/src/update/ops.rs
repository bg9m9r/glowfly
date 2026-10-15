//! Staging, committing, git commands (push, pull, branch…), and auto-fetch.

use crate::effect::{Effect, LocalOp, UndoResult};
use crate::msg::{JobMsg, StageMsg};
use crate::state::trust::Created;
use crate::state::{AppState, CommitDraft, RunningJob};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use trellis_core::gitcli::args;
use trellis_core::repo::{DiffTarget, HunkOp, Snapshot};
use trellis_core::undo::{ResetMode, UndoKind};

pub(super) fn stage(st: &mut AppState, msg: StageMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        StageMsg::StageFile(p) => local(st, LocalOp::StageFile(p), fx),
        StageMsg::UnstageFile(p) => local(st, LocalOp::UnstagePaths(vec![p]), fx),
        StageMsg::StageAll => {
            // Staging a conflicted file marks it resolved, markers and all, so that is done
            // file by file in the conflict view. A toast, not the log: Ctrl+Shift+S sends this.
            if st.snap.as_ref().is_some_and(|s| s.unstaged.iter().any(|f| f.status == '!')) {
                st.toast(false, "Resolve the conflicted files in the conflict view before staging everything.", now);
                return;
            }
            local(st, LocalOp::StageAll, fx);
        }
        StageMsg::UnstageAll => {
            let paths = st.snap.as_ref().map(|s| s.staged.iter().map(|f| f.path.clone()).collect()).unwrap_or_default();
            local(st, LocalOp::UnstagePaths(paths), fx);
        }
        StageMsg::DiscardFile(path) => local(st, LocalOp::DiscardFile(path), fx),
        StageMsg::Hunk { op, path, index, generation } => {
            // A confirm dialog or a message sent before whitespace was hidden can still arrive.
            if let Some(why) = st.hunk_ops_blocked() {
                st.log(false, why);
                return;
            }
            // The index counts hunks in the diff the user saw. If that diff has been reloaded,
            // replaced, or a reload is still on its way, the index may name a different hunk.
            if st.hunk_ops_gen() != Some(generation) {
                st.log(false, "The diff changed before the hunk operation ran, so nothing was changed. Try again on the updated diff.");
                return;
            }
            // Core refuses the operation unless git still produces this hunk at `index`.
            let Some(expected) = st.diff.as_ref().and_then(|(_, d)| d.hunks.get(index)).cloned() else { return };
            // Its hunks are stale from now until the reload that follows the operation lands.
            st.diff_gen += 1;
            local(st, LocalOp::Hunk { op, path, index, expected }, fx);
        }
        StageMsg::ApplyLines(op) => apply_lines(st, op, fx),
        StageMsg::RevertHunk { index, generation } => revert_hunk(st, index, generation, fx),
        StageMsg::HunkReverted { path, result } => {
            match result {
                Ok(()) => st.toast(true, format!("Reverted the hunk in {path}. The change is in your working tree."), now),
                Err(e) => st.log(false, format!("Could not revert the hunk in {path}: {e}")),
            }
            super::watch::settled(st, now);
            super::repo::refresh_status(st, fx);
        }
        StageMsg::LocalOpDone(result) => {
            match result {
                Err(e) => st.log(false, e),
                Ok(Some(action)) => st.undo.record(action),
                Ok(None) => {}
            }
            // A save that failed can be tried again; one that worked closes the view once the
            // status shows the file resolved.
            if let Some(c) = st.conflict.as_mut() {
                c.saving = false;
            }
            super::watch::settled(st, now);
            super::repo::refresh_status(st, fx);
        }
    }
}

pub(super) fn job(st: &mut AppState, msg: JobMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        JobMsg::Git { label, mut cmds } => {
            let record = if cmds.len() == 1 { UndoKind::classify(&cmds[0]) } else { None };
            // A hard reset stashes uncommitted changes first, so undoing it brings them back.
            if record == Some(UndoKind::Reset(ResetMode::Hard)) {
                cmds = trellis_core::undo::hard_reset_commands(&cmds[0][2]);
            }
            start_job(st, JobSpec { record, ..JobSpec::new(label, cmds) }, fx);
        }
        JobMsg::Push { force } => push(st, force, fx),
        JobMsg::Deepen(how) => super::shallow::deepen(st, how, fx),
        JobMsg::Stash { action, index, oid } => super::stash::run(st, action, index, oid, fx),
        JobMsg::Undo => super::undo::run(st, false, now, fx),
        JobMsg::Redo => super::undo::run(st, true, now, fx),
        JobMsg::JobDone { ok, output, undo } => {
            let flow = super::gitflow_finish::headline(st, ok, &output);
            // Always called so a stale pending tag job or patch apply is cleared.
            let tag = super::tags::headline(st, ok, &output);
            let patch = super::patch::headline(st, ok, &output);
            let headline = flow.or(tag).or(patch);
            finish_job(st, ok, output, headline.clone(), undo, now, fx);
            // Toasts fade after a few seconds; the log keeps what a stopped Finish left to do.
            if let (false, Some(h)) = (ok, headline) {
                st.log(false, h);
            }
        }
    }
}

/// Starts the Clone dialog's clone as the active job.
pub(super) fn clone_repo(st: &mut AppState, req: crate::state::clone::CloneRequest, fx: &mut Vec<Effect>) {
    // The dialog blocks these; checked again so no other sender can start a bad clone.
    if req.name.is_empty() || trellis_core::remote_ops::check_url(&req.url).is_err() {
        st.log(false, format!("Could not clone: {} isn't a URL or folder git can clone.", req.url));
        return;
    }
    let dest = req.parent.join(&req.name);
    let cmd = trellis_core::clone_ops::clone_cmd(&req.url, &req.name, &req.options);
    start_job(
        st,
        JobSpec { cwd: Some(req.parent), then_open: Some((dest, Created::Clone)), ..JobSpec::new("Clone".into(), vec![cmd]) },
        fx,
    );
}

/// A git command sequence to run as the (single) active job.
pub(super) struct JobSpec {
    pub label: String,
    pub cmds: Vec<Vec<String>>,
    pub cwd: Option<PathBuf>,
    pub quiet: bool,
    pub clear_commit: bool,
    /// Push the branch once the job (a commit) succeeds.
    pub then_push: bool,
    /// Open this repository in the active tab once the job succeeds; how it was made decides
    /// whether it is trusted (update/trust.rs).
    pub then_open: Option<(PathBuf, Created)>,
    pub write_file: Option<(PathBuf, String)>,
    /// Record the job for undo when it is this kind of operation.
    pub record: Option<UndoKind>,
    /// Run nothing unless this revision still resolves to this commit.
    pub expect: Option<(String, git2::Oid)>,
}

impl JobSpec {
    pub fn new(label: String, cmds: Vec<Vec<String>>) -> Self {
        Self {
            label,
            cmds,
            cwd: None,
            quiet: false,
            clear_commit: false,
            then_push: false,
            then_open: None,
            write_file: None,
            record: None,
            expect: None,
        }
    }
}

/// Whether a job is already running (only one runs at a time); tells the user unless `quiet`.
pub(super) fn busy(st: &mut AppState, quiet: bool) -> bool {
    if st.job.is_some() && !quiet {
        st.log(false, "Another operation is still running.");
    }
    st.job.is_some()
}

pub(super) fn start_job(st: &mut AppState, spec: JobSpec, fx: &mut Vec<Effect>) {
    let job = RunningJob {
        label: spec.label,
        workdir: None,
        quiet: spec.quiet,
        clear_commit: spec.clear_commit,
        then_push: spec.then_push,
        then_open: spec.then_open,
        keeps_undo: super::undo::keeps_history(&spec.cmds),
    };
    let Some(cwd) = claim(st, job, spec.cwd) else { return };
    // Background jobs fail rather than pop up a credential prompt.
    let prompts = !spec.quiet;
    fx.push(Effect::RunGit { cwd, cmds: spec.cmds, write_file: spec.write_file, record: spec.record, prompts, expect: spec.expect });
}

/// Makes a job labelled `label` the active one in the active tab's repository, unless another
/// job is running (said in the log). Returns the folder to run it in.
pub(super) fn begin_job(st: &mut AppState, label: &str, keeps_undo: bool) -> Option<PathBuf> {
    let job =
        RunningJob { label: label.into(), workdir: None, quiet: false, clear_commit: false, then_push: false, then_open: None, keeps_undo };
    claim(st, job, None)
}

/// Makes `job` the active job (in the active tab) unless one is running; returns `cwd`, or the
/// active tab's repository.
fn claim(st: &mut AppState, mut job: RunningJob, cwd: Option<PathBuf>) -> Option<PathBuf> {
    if busy(st, job.quiet) {
        return None;
    }
    let cwd = cwd.or_else(|| st.workdir())?;
    job.workdir = st.workdir();
    st.job = Some(job);
    Some(cwd)
}

/// Stages, unstages or discards the selected diff lines, when the selection belongs to the
/// diff on screen and `op` fits it (stage or discard unstaged lines, unstage staged ones).
fn apply_lines(st: &mut AppState, op: HunkOp, fx: &mut Vec<Effect>) {
    let Some((target, diff)) = &st.diff else { return };
    let sel = &st.view.diff_lines;
    let fits =
        matches!((target, op), (DiffTarget::Unstaged(_), HunkOp::Stage | HunkOp::Discard) | (DiffTarget::Staged(_), HunkOp::Unstage));
    if sel.is_empty() || sel.target.as_ref() != Some(target) || !fits {
        return;
    }
    if let Some(why) = st.line_ops_blocked() {
        st.log(false, why);
        return;
    }
    // Like hunk operations: line indexes are only good for the diff on screen.
    if st.hunk_ops_gen().is_none() {
        st.log(false, "The diff changed before the line operation ran, so nothing was changed. Try again on the updated diff.");
        return;
    }
    let Some(expected) = diff.hunks.get(st.view.diff_lines.hunk).cloned() else { return };
    let path = target.path().to_owned();
    let sel = std::mem::take(&mut st.view.diff_lines);
    // Its hunks are stale from now until the reload that follows the operation lands.
    st.diff_gen += 1;
    let lines = sel.lines.into_iter().collect();
    local(st, LocalOp::Lines { op, path, hunk: sel.hunk, expected, lines }, fx);
}

/// Reverts a hunk of the open commit or comparison diff into the working tree. Unlike
/// `StageMsg::Hunk` this leaves `diff_gen` alone: a commit diff's hunks can't change, so the other
/// buttons stay valid (the status refresh that follows reloads the diff anyway).
fn revert_hunk(st: &mut AppState, index: usize, generation: u64, fx: &mut Vec<Effect>) {
    if st.diff.as_ref().is_none_or(|(target, _)| target.is_working_tree()) {
        return;
    }
    if let Some(why) = st.hunk_ops_blocked() {
        st.log(false, why);
        return;
    }
    if st.hunk_ops_gen() != Some(generation) {
        st.log(false, "The diff changed before the revert ran, so nothing was changed. Try again on the updated diff.");
        return;
    }
    // Always the normal hunks, also in the whole-file view: core counts those.
    let Some((target, diff)) = &st.diff else { return };
    let Some(expected) = diff.hunks.get(index).cloned() else { return };
    let target = target.clone();
    let Some(workdir) = st.workdir() else { return };
    fx.push(Effect::RevertHunk { workdir, target, index, expected });
}

pub(super) fn local(st: &mut AppState, op: LocalOp, fx: &mut Vec<Effect>) {
    if let Some(workdir) = st.workdir() {
        fx.push(Effect::LocalOp { workdir, op });
    }
}

/// The remote to push a new branch to: origin if present, else the first remote.
pub(super) fn default_remote(snap: &Snapshot) -> Option<String> {
    crate::state::branches::default_remote(snap)
}

pub(super) fn push(st: &mut AppState, force: bool, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let Some(branch) = snap.head_branch.clone() else {
        st.log(false, "Push is not available in detached HEAD.");
        return;
    };
    let has_upstream = snap.head_info().is_some_and(|b| b.upstream.is_some());
    if force {
        let Some(cmd) = snap.head_info().and_then(|b| force_push_cmd(b, snap)) else {
            // No remote to push to: offer to create the repository on GitHub.
            super::publish::open(st, fx);
            return;
        };
        start_job(st, JobSpec::new("Force push".into(), vec![cmd]), fx);
        return;
    }
    let mut cmd = args(&["push"]);
    if !has_upstream {
        let Some(remote) = default_remote(snap) else {
            // Nowhere to push yet: offer to create the repository on GitHub.
            super::publish::open(st, fx);
            return;
        };
        cmd.extend(["-u".to_owned(), remote, branch]);
    }
    start_job(st, JobSpec::new("Push".into(), vec![cmd]), fx);
}

/// A force push of `b` that only replaces the remote branch if it still is where the snapshot
/// last saw it (or still doesn't exist): bare `--force-with-lease` compares against the
/// remote-tracking ref, which background fetches keep moving, so it would overwrite commits
/// the user never saw. `--force-if-includes` also requires that tip to be in `b`'s reflog.
pub(super) fn force_push_cmd(b: &trellis_core::repo::BranchInfo, snap: &Snapshot) -> Option<Vec<String>> {
    let (remote, dest, set_upstream) = crate::state::branches::push_target(b, snap, None)?;
    let seen = snap.remotes.iter().find(|(r, _)| *r == remote).and_then(|(_, bs)| bs.iter().find(|(n, _)| *n == dest)).map(|(_, o)| *o);
    let expect = seen.map(|o| o.to_string()).unwrap_or_default();
    let mut cmd = args(&["push", &format!("--force-with-lease=refs/heads/{dest}:{expect}"), "--force-if-includes"]);
    if set_upstream {
        cmd.push("-u".into());
    }
    cmd.extend([remote, format!("refs/heads/{}:refs/heads/{dest}", b.name)]);
    Some(cmd)
}

/// The active job ended: logs and toasts it (`headline`, or "<label> succeeded" / "<label>
/// failed: <reason>"), closes the credential prompts it left, updates the undo history of the tab it ran in, and refreshes that tab if
/// it is still active.
pub(super) fn finish_job(
    st: &mut AppState,
    ok: bool,
    output: String,
    headline: Option<String>,
    undo: Option<UndoResult>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    let Some(job) = st.job.take() else { return };
    // A cancelled or timed-out credential prompt explains a failure better than git's output.
    let headline = super::askpass::job_finished(st, ok, &job.label, fx).or(headline);
    let status = if ok { "done" } else { "FAILED" };
    let text = format!("{} {status}\n{output}", job.label);
    if job.quiet {
        st.log_quiet(ok, text);
    } else {
        let headline = headline.unwrap_or_else(|| {
            if ok { format!("{} succeeded", job.label) } else { format!("{} failed: {}", job.label, last_line(&output)) }
        });
        st.toast(ok, headline, now);
        st.log(ok, text);
    }
    // The user may have switched tabs while the job ran.
    let here = match (job.workdir.as_deref(), st.workdir()) {
        (Some(a), Some(b)) => crate::state::tabs::same_path(a, &b),
        (a, b) => a.is_none() && b.is_none(),
    };
    super::undo::finished(st, &job, ok, undo, here);
    if ok {
        if job.clear_commit {
            if here {
                st.draft = CommitDraft::default();
            } else if let Some(parked) =
                job.workdir.as_deref().and_then(|w| st.tabs.iter_mut().find(|t| t.is(w))).and_then(|t| t.parked.as_mut())
            {
                parked.draft = CommitDraft::default();
            }
        }
        if let Some((p, how)) = job.then_open {
            super::trust::created(st, &p, how, &output);
            super::tabs::open(st, p, fx);
            return;
        }
    }
    if !here {
        if ok && job.then_push {
            st.log(true, "Push after committing was skipped: that repository's tab is no longer active.");
        }
        return; // that tab refreshes when it is selected again
    }
    super::watch::settled(st, now);
    super::repo::refresh(st, fx);
    super::lfs::job_finished(st, &job.label, ok, job.quiet, fx);
    // Pushes and branch changes affect pull requests; auto-fetch uses the normal interval.
    if !job.quiet {
        super::forge::refresh(st, true, false, now, fx);
    }
    // Push after committing, from the snapshot before the commit: the upstream is unchanged.
    if ok && job.then_push {
        push(st, false, fx);
    }
}

/// The last line of a job's output that isn't a command echo.
pub(super) fn last_line(output: &str) -> &str {
    output.lines().rfind(|l| !l.starts_with("$ ")).unwrap_or("")
}

/// Background fetch on the interval from Preferences > General.
pub(super) fn auto_fetch(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    let minutes = st.settings.auto_fetch_minutes;
    // Held until the user trusts a repository whose config or hooks run programs.
    if minutes == 0 || st.snap.as_ref().is_none_or(|s| s.remotes.is_empty()) || st.job.is_some() || !st.runs_git_unasked() {
        return;
    }
    if now.duration_since(st.last_fetch) < Duration::from_secs(minutes as u64 * 60) {
        return;
    }
    st.last_fetch = now;
    let mut cmd = args(&["fetch", "--all"]);
    if st.settings.auto_prune {
        cmd.push("--prune".into());
    }
    start_job(st, JobSpec { quiet: true, ..JobSpec::new("Auto-fetch".into(), vec![cmd]) }, fx);
}
