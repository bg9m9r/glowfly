//! Staging, committing, git commands (push, pull, branch…), and auto-fetch; reverts are in update/revert.rs. Jobs start in
//! update/jobs.rs, re-exported here.

use crate::effect::{Effect, LocalOp, UndoResult};
use crate::msg::{JobMsg, StageMsg};
use crate::state::trust::Created;
use crate::state::{AppState, CommitDraft};
use glowfly_core::gitcli::args;
use glowfly_core::repo::{DiffTarget, HunkOp, Snapshot};
use std::time::{Duration, Instant};

pub(super) use super::jobs::{JobSpec, Started, begin_job, busy, start_job};

pub(super) fn stage(st: &mut AppState, msg: StageMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        StageMsg::StageFile(p) => {
            // Staging a conflicted file marks it resolved: checked for markers first.
            if st.snap.as_ref().is_some_and(|s| s.unstaged.iter().any(|f| f.status == '!' && f.path == p)) {
                super::conflict_files::mark_resolved(st, vec![p], fx);
                return;
            }
            // Marking a file resolved while the merge tool has it open stages the markers.
            if !super::conflict::in_tool(st, &p) {
                local(st, LocalOp::StageFile(p), fx);
            }
        }
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
        StageMsg::StageFolder(p) => super::stage_folder::stage(st, p, now, fx),
        StageMsg::UnstageFolder(p) => super::stage_folder::unstage(st, p, fx),
        StageMsg::DiscardFile(path) => {
            // `git restore` writes the file, so filters and attributes apply.
            if super::git_env::allows(st, glowfly_core::git_version::Feature::Restore, now) {
                local(st, LocalOp::DiscardFile(path), fx);
            }
        }
        StageMsg::Hunk { op, path, index, generation } => {
            // The index counts hunks in the diff the user saw. If that diff has been reloaded,
            // replaced, or a reload is still on its way, the index may name a different hunk.
            if st.hunk_ops_gen() != Some(generation) {
                st.log(false, "The diff changed before the hunk operation ran, so nothing was changed. Try again on the updated diff.");
                return;
            }
            // Core refuses the operation unless git still produces this hunk at `index`.
            let Some(expected) = st.session.diff.view.as_ref().and_then(|(_, d)| d.hunks.get(index)).cloned() else { return };
            // Its hunks are stale from now until the reload that follows the operation lands.
            st.session.diff.generation += 1;
            // Toggling the whitespace filter reloads the diff, so `generation` matched this one.
            let ignore_whitespace = st.settings.diff_ignore_whitespace;
            local(st, LocalOp::Hunk { op, path, index, expected, ignore_whitespace }, fx);
        }
        StageMsg::ApplyLines(op) => apply_lines(st, op, fx),
        StageMsg::RevertHunk { index, generation } => super::revert::hunk(st, index, generation, fx),
        StageMsg::RevertLines => super::revert::lines(st, fx),
        StageMsg::RevertFile(target) => super::revert::file(st, target, fx),
        StageMsg::Reverted { path, what, result, undo } => super::revert::done(st, path, what, result, undo, now, fx),
        StageMsg::LocalOpDone(result) => {
            let failed = result.is_err();
            match result {
                Err(e) => st.log(false, e),
                Ok(Some(action)) => st.session.undo.record(action),
                Ok(None) => {}
            }
            // A save that failed can be tried again; one that worked moves the view on once the
            // status shows the file resolved.
            let mut saved = None;
            if let Some(c) = st.session.conflict.view.as_mut() {
                c.saving = false;
                c.step_on_resolve &= !failed;
                // What Save with markers wrote is now what the file holds.
                if let Some((note, picks, cleans)) = c.pending_save.take().filter(|_| !failed) {
                    c.baseline = (picks, cleans);
                    saved = Some(note);
                }
            }
            // Save with markers leaves the view open: say it worked.
            if let Some(note) = saved {
                st.toast(true, note, now);
            }
            super::watch::settled(st, now);
            super::wip_list::op_done(st, !failed, fx);
            super::repo::refresh_status(st, fx);
        }
    }
}

pub(super) fn job(st: &mut AppState, msg: JobMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        JobMsg::Git { label, cmds } => super::git_job::run(st, label, cmds, None, None, fx),
        JobMsg::Journaled { label, cmd, op, expect } => super::git_job::run(st, label, vec![cmd], Some(op), expect, fx),
        JobMsg::Abort { label, args, keep } => super::op_progress::abort(st, label, args, keep, now, fx),
        JobMsg::Push { force } => push(st, force, fx),
        JobMsg::Deepen(how) => super::shallow::deepen(st, how, fx),
        JobMsg::DeepenNow(how) => super::shallow::deepen_now(st, how, fx),
        JobMsg::Cancel => super::job_stop::cancel(st, fx),
        JobMsg::Stopped { output } => super::job_stop::stopped(st, output, now, fx),
        JobMsg::Stash { action, index, oid } => super::stash::run(st, action, index, oid, fx),
        JobMsg::Undo => super::undo::run(st, false, now, fx),
        JobMsg::Redo => super::undo::run(st, true, now, fx),
        JobMsg::UndoWith { redo, action, opts } => super::undo::run_with(st, redo, *action, opts, fx),
        JobMsg::PickDone { ok, output, undo, result } => super::pick_done::finished(st, ok, output, undo, result, now, fx),
        JobMsg::JobDone { ok, output, undo } => {
            // A download started from a waiting pane reads its view again.
            super::partial_content::job_done(st, ok, &output, fx);
            let flow = super::gitflow_finish::headline(st, ok, &output);
            // Always called so a stale pending tag job or patch apply is cleared.
            let tag = super::tags::headline(st, ok, &output);
            let patch = super::patch::headline(st, ok, &output);
            let stash = super::stash_push::headline(st, ok, &output);
            let (checkout, warn) = super::tag_checkout::headline(st, ok).map_or((None, None), |(t, w)| (Some(t), w));
            let headline = flow.or(tag).or(patch).or(stash).or(checkout);
            finish_job(st, ok, output, headline.clone(), undo, now, fx);
            if let Some(p) = warn {
                super::tag_checkout::warned(st, p, fx);
            }
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
    if req.name.is_empty() || glowfly_core::remote_ops::check_url(&req.url).is_err() {
        st.log(false, format!("Could not clone: {} isn't a URL or folder git can clone.", req.url));
        return;
    }
    // Never resolved against the process's working folder.
    if !req.parent.is_absolute() {
        st.log(false, format!("Could not clone: {} isn't the full path of a folder.", req.parent.display()));
        return;
    }
    let dest = req.parent.join(&req.name);
    let cmd = glowfly_core::clone_ops::clone_cmd(&req.url, &req.name, &req.options);
    if let Some(sparse) = req.options.sparse {
        if let Some(why) = st.git.blocked(glowfly_core::git_version::Feature::SparseCheckout) {
            st.log(false, format!("Could not clone: {why}"));
            return;
        }
        let (then_open, folders) = (Some((dest.clone(), Created::Clone)), sparse.folders);
        let effect = |parent| Effect::Clone(crate::effect::CloneEffect::Sparse { parent, cmd, dest, folders });
        super::jobs::start_effect_job(st, "Clone", req.parent, then_open, effect, fx);
        return;
    }
    start_job(
        st,
        JobSpec { cwd: Some(req.parent), then_open: Some((dest, Created::Clone)), ..JobSpec::new("Clone".into(), vec![cmd]) },
        fx,
    );
}

/// Stages, unstages or discards the selected diff lines, when the selection belongs to the
/// diff on screen and `op` fits it (stage or discard unstaged lines, unstage staged ones).
fn apply_lines(st: &mut AppState, op: HunkOp, fx: &mut Vec<Effect>) {
    let Some((target, diff)) = &st.session.diff.view else { return };
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
    st.session.diff.generation += 1;
    let lines = sel.lines.into_iter().collect();
    let ignore_whitespace = st.settings.diff_ignore_whitespace;
    local(st, LocalOp::Lines { op, path, hunk: sel.hunk, expected, lines, ignore_whitespace }, fx);
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
        let if_includes = st.git.has(glowfly_core::git_version::Feature::ForceIfIncludes);
        let Some(cmd) = snap.head_info().and_then(|b| force_push_cmd(b, snap, if_includes)) else {
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
/// the user never saw. With `if_includes` (git 2.30 or newer), `--force-if-includes` also
/// requires that tip to be in `b`'s reflog.
pub(super) fn force_push_cmd(b: &glowfly_core::repo::BranchInfo, snap: &Snapshot, if_includes: bool) -> Option<Vec<String>> {
    let (remote, dest, set_upstream) = crate::state::branches::push_target(b, snap, None)?;
    let seen = snap.remotes.iter().find(|(r, _)| *r == remote).and_then(|(_, bs)| bs.iter().find(|(n, _)| *n == dest)).map(|(_, o)| *o);
    let expect = seen.map(|o| o.to_string()).unwrap_or_default();
    let mut cmd = args(&["push", &format!("--force-with-lease=refs/heads/{dest}:{expect}")]);
    if if_includes {
        cmd.push("--force-if-includes".into());
    }
    if set_upstream {
        cmd.push("-u".into());
    }
    cmd.extend([remote, format!("refs/heads/{}:refs/heads/{dest}", b.name)]);
    Some(cmd)
}

/// The active job ended: logs it as a run and toasts it (update/logs_runs.rs), closes the credential prompts it left, updates the undo history of the
/// tab it ran in, and refreshes that tab if it is still active.
pub(super) fn finish_job(
    st: &mut AppState,
    ok: bool,
    output: String,
    headline: Option<String>,
    undo: Option<UndoResult>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    let Some(mut job) = st.job.take() else { return };
    // Cancelled from the status bar, and the runner stopped it (update/job_stop.rs).
    let cancelled = st.job_stop.take().is_some_and(|s| s.stopped);
    let stash_retry = super::auto_stash::take_retry(st, &job);
    // Trust passed on to what the job made opens `then_open` itself, once it is recorded.
    let took_open = super::trust_inherit::job_ended(st, &job.label, ok, job.then_open.as_ref().map(|(p, _)| p.as_path()), fx);
    super::sign_in::job_finished(st, job.workdir.as_deref(), ok, &output);
    let then_pr = job.then_pr.take();
    // A cancelled or timed-out credential prompt explains a failure better than git's output.
    let headline = super::askpass::job_finished(st, ok, cancelled, &job.label, job.workdir.as_deref(), &output, fx).or(headline);
    match headline {
        Some(h) if cancelled => super::logs_runs::job_cancelled(st, &job, &output, h, now),
        headline => super::logs_runs::job_ended(st, &job, ok, &output, headline, stash_retry, now, fx),
    }
    // The user may have switched tabs while the job ran.
    let here = match (job.workdir.as_deref(), st.workdir()) {
        (Some(a), Some(b)) => glowfly_core::paths::same_spelling(a, &b),
        (a, b) => a.is_none() && b.is_none(),
    };
    // Pull's tooltip says when the repository last fetched; other tabs read FETCH_HEAD on load.
    let fetched = st.view.now;
    if ok
        && job.fetches
        && here
        && let Some(snap) = st.snap.as_mut()
    {
        snap.fetched_at = Some(fetched);
    }
    let reopen = super::conflict::after_undo(&undo, ok);
    super::stack::job_finished(st, ok && !job.quiet, here, &undo, now);
    super::undo::finished(st, &job, ok, undo, here);
    super::auto_stash::job_finished(st, &job);
    if here && let Some((p, open_closed)) = reopen {
        super::conflict::reopen_after_undo(st, p, open_closed, fx);
    }
    super::submodule_batch::job_finished(st, &job.label, job.workdir.as_deref(), ok, here, fx);
    if job.then_link {
        super::deep_link_open::fetched(st, ok, here, fx);
    }
    if ok {
        if job.clear_commit {
            if here {
                st.session.draft = CommitDraft::default();
            } else if let Some(parked) =
                job.workdir.as_deref().and_then(|w| st.tabs.iter_mut().find(|t| t.is(w))).and_then(|t| t.parked.as_mut())
            {
                parked.session.draft = CommitDraft::default();
            }
        }
        if let Some((p, how)) = job.then_open {
            super::trust::created(st, &p, how, &output);
            if !took_open {
                super::tabs::open(st, p, fx);
            }
            return;
        }
    }
    if !here {
        if ok && job.then_push {
            st.log(true, "Push after committing was skipped: that repository's tab is no longer active.");
        }
        super::push_pr::job_finished(st, then_pr, ok, false);
        return; // that tab refreshes when it is selected again
    }
    super::notify::fetched(st, &job.label, job.quiet, ok);
    super::watch::settled(st, now);
    super::repo::refresh(st, fx);
    super::lfs::job_finished(st, &job.label, ok, job.quiet, fx);
    // Pushes and branch changes affect pull requests; auto-fetch uses the normal interval.
    if !job.quiet {
        super::forge::refresh(st, true, false, now, fx);
    }
    super::push_pr::job_finished(st, then_pr, ok, true);
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
    // Remotes that need a sign-in are skipped (update/sign_in.rs).
    let Some((cmd, _)) = super::sign_in::auto_fetch_cmd(st) else { return };
    start_job(st, JobSpec { quiet: true, ..JobSpec::new(super::notify::AUTO_FETCH.into(), vec![cmd]) }, fx);
}
