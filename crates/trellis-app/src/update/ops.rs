//! Staging, committing, git commands (push, pull, branch…), and auto-fetch.

use crate::effect::{Effect, LocalOp, UndoResult};
use crate::msg::Msg;
use crate::state::{AppState, CommitDraft, RunningJob};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use trellis_core::gitcli::args;
use trellis_core::repo::{DiffTarget, HunkOp, Snapshot};
use trellis_core::undo::UndoKind;

pub(super) fn handle(st: &mut AppState, msg: Msg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        Msg::Tick(t) => auto_fetch(st, t, fx),
        Msg::StageFile(p) => local(st, LocalOp::StageFile(p), fx),
        Msg::UnstageFile(p) => local(st, LocalOp::UnstagePaths(vec![p]), fx),
        Msg::StageAll => local(st, LocalOp::StageAll, fx),
        Msg::UnstageAll => {
            let paths = st.snap.as_ref().map(|s| s.staged.iter().map(|f| f.path.clone()).collect()).unwrap_or_default();
            local(st, LocalOp::UnstagePaths(paths), fx);
        }
        Msg::DiscardFile(path, untracked) => local(st, LocalOp::DiscardFile { path, untracked }, fx),
        Msg::Hunk { op, path, index, generation } => {
            // The index counts hunks in the diff the user saw. If that diff has been reloaded,
            // replaced, or a reload is still on its way, the index may name a different hunk.
            if st.hunk_ops_gen() != Some(generation) {
                st.log(false, "The diff changed before the hunk operation ran, so nothing was changed. Try again on the updated diff.");
                return;
            }
            // Its hunks are stale from now until the reload that follows the operation lands.
            st.diff_gen += 1;
            local(st, LocalOp::Hunk { op, path, index }, fx);
        }
        Msg::ApplyLines(op) => apply_lines(st, op, fx),
        Msg::LocalOpDone(result) => {
            if let Err(e) = result {
                st.log(false, e);
            }
            super::repo::refresh_status(st, fx);
        }
        Msg::Commit => commit(st, fx),
        Msg::PrefillAmend => {
            // Pre-fill from HEAD's message, when the user hasn't typed anything yet.
            if st.draft.summary.trim().is_empty()
                && let Some(m) = st.snap.as_ref().and_then(|s| s.head_message.clone())
            {
                st.draft.fill_from(&m);
            }
        }
        Msg::AppendToDescription(text) => {
            if !st.draft.description.trim().is_empty() {
                st.draft.description.push_str("\n\n");
            }
            st.draft.description.push_str(&text);
        }
        Msg::Git { label, cmds } => {
            let record = if cmds.len() == 1 { UndoKind::classify(&cmds[0]) } else { None };
            start_job(st, JobSpec { record, ..JobSpec::new(label, cmds) }, fx);
        }
        Msg::Push { force } => push(st, force, fx),
        Msg::InitRepo => {
            fx.push(Effect::PickFolder { title: "Initialize a repository in…".into(), purpose: crate::msg::FolderPurpose::InitRepo })
        }
        Msg::CloneRepo { url, parent, name } => {
            let dest = parent.join(&name);
            start_job(
                st,
                JobSpec { cwd: Some(parent), then_open: Some(dest), ..JobSpec::new("Clone".into(), vec![args(&["clone", &url, &name])]) },
                fx,
            );
        }
        Msg::JobDone { ok, output, undo } => job_done(st, ok, output, undo, now, fx),
        _ => {}
    }
}

/// A git command sequence to run as the (single) active job.
pub(super) struct JobSpec {
    pub label: String,
    pub cmds: Vec<Vec<String>>,
    pub cwd: Option<PathBuf>,
    pub quiet: bool,
    pub clear_commit: bool,
    pub then_open: Option<PathBuf>,
    pub write_file: Option<(PathBuf, String)>,
    /// Record the job for undo when it is this kind of operation.
    pub record: Option<UndoKind>,
}

impl JobSpec {
    pub fn new(label: String, cmds: Vec<Vec<String>>) -> Self {
        Self { label, cmds, cwd: None, quiet: false, clear_commit: false, then_open: None, write_file: None, record: None }
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
    if busy(st, spec.quiet) {
        return;
    }
    let Some(cwd) = spec.cwd.or_else(|| st.workdir()) else { return };
    st.job = Some(RunningJob {
        label: spec.label,
        workdir: st.workdir(),
        quiet: spec.quiet,
        clear_commit: spec.clear_commit,
        then_open: spec.then_open,
        keeps_undo: super::undo::keeps_history(&spec.cmds),
    });
    fx.push(Effect::RunGit { cwd, cmds: spec.cmds, write_file: spec.write_file, record: spec.record });
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

fn local(st: &mut AppState, op: LocalOp, fx: &mut Vec<Effect>) {
    if let Some(workdir) = st.workdir() {
        fx.push(Effect::LocalOp { workdir, op });
    }
}

fn commit(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let file = snap.git_dir.join("TRELLIS_COMMIT_EDITMSG");
    let mut cmd = args(&["commit", "-F"]);
    cmd.push(file.display().to_string());
    if st.draft.amend {
        cmd.push("--amend".into());
    }
    let label = if st.draft.amend { "Amend" } else { "Commit" };
    let spec = JobSpec {
        clear_commit: true,
        write_file: Some((file, st.draft.message() + "\n")),
        record: Some(UndoKind::Commit { amend: st.draft.amend }),
        ..JobSpec::new(label.into(), vec![cmd])
    };
    start_job(st, spec, fx);
}

/// The remote to push a new branch to: origin if present, else the first remote.
pub(super) fn default_remote(snap: &Snapshot) -> Option<String> {
    let names: Vec<&str> = snap.remotes.iter().map(|(r, _)| r.as_str()).collect();
    names.iter().find(|r| **r == "origin").or(names.first()).map(|r| r.to_string())
}

fn push(st: &mut AppState, force: bool, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let Some(branch) = snap.head_branch.clone() else {
        st.log(false, "Push is not available in detached HEAD.");
        return;
    };
    let has_upstream = snap.head_info().is_some_and(|b| b.upstream.is_some());
    let mut cmd = args(&["push"]);
    if force {
        cmd.push("--force-with-lease".into());
    }
    if !has_upstream {
        let Some(remote) = default_remote(snap) else {
            // Nowhere to push yet: offer to create the repository on GitHub.
            super::publish::open(st, fx);
            return;
        };
        cmd.extend(["-u".to_owned(), remote, branch]);
    }
    start_job(st, JobSpec::new(if force { "Force push" } else { "Push" }.into(), vec![cmd]), fx);
}

fn job_done(st: &mut AppState, ok: bool, output: String, undo: Option<UndoResult>, now: Instant, fx: &mut Vec<Effect>) {
    let Some(job) = st.job.take() else { return };
    let status = if ok { "done" } else { "FAILED" };
    let text = format!("{} {status}\n{output}", job.label);
    if job.quiet {
        st.log.push(crate::state::LogEntry { ok, text });
    } else {
        let headline = if ok {
            format!("{} succeeded", job.label)
        } else {
            let reason = output.lines().rfind(|l| !l.starts_with("$ ")).unwrap_or("").to_owned();
            format!("{} failed: {reason}", job.label)
        };
        st.toast(ok, headline, now);
        st.log(ok, text);
    }
    // The user may have switched tabs while the job ran.
    let here = job.workdir == st.workdir();
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
        if let Some(p) = job.then_open {
            super::tabs::open(st, p, fx);
            return;
        }
    }
    if !here {
        return; // that tab refreshes when it is selected again
    }
    super::repo::refresh(st, fx);
    // Pushes and branch changes affect pull requests; auto-fetch uses the normal interval.
    if !job.quiet {
        super::forge::refresh(st, true, false, now, fx);
    }
}

/// Background fetch on the interval from Preferences > General.
fn auto_fetch(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    let minutes = st.settings.auto_fetch_minutes;
    if minutes == 0 || st.snap.as_ref().is_none_or(|s| s.remotes.is_empty()) || st.job.is_some() {
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
