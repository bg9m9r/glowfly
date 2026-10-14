//! Removing, locking, unlocking and pruning worktrees from the WORKTREES section. Every request
//! is checked again against the current snapshot. Remove runs as its own effect
//! (`Effect::RemoveWorktree`: a lock or changes are found before git runs, and the branch is
//! deleted afterwards) and comes back as `WorktreeMsg::Removed`; the others are plain git jobs.

use super::ops::{self, JobSpec};
use crate::effect::{Effect, UndoResult};
use crate::state::worktree_manage::{
    LockForm, RemoveForm, branch_failed_text, folder_left_headline, gone_text, lock_refused, needs_force_text, prune_blocked,
    remove_refused, removed_headline, unlock_refused,
};
use crate::state::worktrees::find;
use crate::state::{AppState, Modal};
use crate::worktree::RemoveRequest;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::repo::WorktreeInfo;
use trellis_core::undo::Action;
use trellis_core::worktree_ops::{Refusal, RemoveOutcome, lock_cmd, prune_cmd, unlock_cmd};

/// `WorktreeMsg::Removed`'s fields.
pub(super) struct Removed {
    pub request: RemoveRequest,
    pub outcome: RemoveOutcome,
    pub output: String,
    pub branch: Option<Result<(), String>>,
    pub undo: Option<Action>,
}

/// The folders open in tabs.
fn open_tabs(st: &AppState) -> Vec<PathBuf> {
    st.tabs.iter().filter_map(|t| t.path.clone()).collect()
}

/// The listed worktree at `path`, or None after logging that it is gone.
fn listed(st: &mut AppState, path: &std::path::Path) -> Option<WorktreeInfo> {
    let found = find(st.snap.as_ref()?, path).cloned();
    if found.is_none() {
        st.log(false, gone_text(path));
    }
    found
}

pub(super) fn remove_dialog(st: &mut AppState, path: PathBuf) {
    let Some(wt) = listed(st, &path) else { return };
    if let Some(why) = remove_refused(&wt, &open_tabs(st)) {
        return st.log(false, format!("Can't remove {}: {why}", wt.folder_name()));
    }
    st.modal = Some(Modal::RemoveWorktree(RemoveForm::new(&wt)));
}

/// The Remove dialog's button.
pub(super) fn remove(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(Modal::RemoveWorktree(form)) = st.modal.as_ref() else { return };
    let mut form = form.clone();
    let Some(wt) = listed(st, &form.path) else {
        st.modal = None;
        return;
    };
    if let Some(why) = remove_refused(&wt, &open_tabs(st)) {
        return st.log(false, format!("Can't remove {}: {why}", form.name));
    }
    if form.delete_branch
        && let Some(b) = &form.branch
        && wt.branch.as_ref() != Some(b)
    {
        return st.log(false, format!("{} no longer has {b} checked out; nothing was removed.", form.name));
    }
    // Locked since the dialog opened: show that first, so force is only used once confirmed.
    if let Some(reason) = &wt.locked
        && !matches!(form.refusal, Some(Refusal::Locked(_)))
    {
        let refusal = Refusal::Locked(reason.clone());
        st.log(false, needs_force_text(&form.name, &refusal));
        form.refusal = Some(refusal);
        st.modal = Some(Modal::RemoveWorktree(form));
        return;
    }
    let Some(cwd) = ops::begin_job(st, "Remove worktree", true) else { return };
    st.modal = None;
    let delete_branch = form.delete_branch.then(|| form.branch.clone()).flatten();
    fx.push(Effect::RemoveWorktree(RemoveRequest {
        repo: cwd,
        path: wt.path,
        name: form.name.clone(),
        force: form.force(),
        delete_branch,
    }));
}

/// The remove finished: one toast and log entry for the outcome (plus a second entry for a
/// branch failure when the folder-left report took the toast). A remove that needs force opens
/// the dialog again in force mode.
pub(super) fn removed(st: &mut AppState, r: Removed, now: Instant, fx: &mut Vec<Effect>) {
    let Some(job) = &st.job else { return };
    let here = match (job.workdir.as_deref(), st.workdir()) {
        (Some(a), Some(b)) => crate::state::tabs::same_path(a, &b),
        _ => false,
    };
    let name = r.request.name.as_str();
    let branch_err = |e: &str| branch_failed_text(name, r.request.delete_branch.as_deref().unwrap_or_default(), e);
    let (ok, headline, extra) = match (&r.outcome, &r.branch) {
        (RemoveOutcome::Removed, None) => (true, Some(removed_headline(name, None)), None),
        (RemoveOutcome::Removed, Some(Ok(()))) => (true, Some(removed_headline(name, r.request.delete_branch.as_deref())), None),
        (RemoveOutcome::Removed, Some(Err(e))) => (false, Some(branch_err(e)), None),
        (RemoveOutcome::FolderLeft, b) => {
            (false, Some(folder_left_headline(name, &r.request.path)), b.as_ref().and_then(|b| b.as_ref().err()).map(|e| branch_err(e)))
        }
        (RemoveOutcome::NeedsForce(refusal), _) => (false, Some(needs_force_text(name, refusal)), None),
        (RemoveOutcome::Failed, _) => (false, None, None),
    };
    // The log entry starts with what happened; the git output follows.
    let output = match &headline {
        Some(h) if !ok || r.output.is_empty() => format!("{h}\n{}", r.output).trim_end().to_owned(),
        _ => r.output.clone(),
    };
    // A branch deleted next to a folder that was left is still undoable, though the job failed.
    let left_undo = matches!(r.outcome, RemoveOutcome::FolderLeft).then(|| r.undo.clone()).flatten();
    let undo = r.request.delete_branch.is_some().then(|| UndoResult::Recorded(r.undo.clone()));
    ops::finish_job(st, ok, output, headline, undo, now, fx);
    if let Some(e) = extra {
        st.log(false, e);
    }
    if here && let Some(a) = left_undo {
        st.undo.record(a);
    }
    if let RemoveOutcome::NeedsForce(refusal) = r.outcome
        && here
        && st.modal.is_none()
        && let Some(wt) = st.snap.as_ref().and_then(|s| find(s, &r.request.path))
    {
        let form = RemoveForm { refusal: Some(refusal), delete_branch: r.request.delete_branch.is_some(), ..RemoveForm::new(wt) };
        st.modal = Some(Modal::RemoveWorktree(form));
    }
}

pub(super) fn lock_dialog(st: &mut AppState, path: PathBuf) {
    let Some(wt) = listed(st, &path) else { return };
    if let Some(why) = lock_refused(&wt) {
        return st.log(false, format!("Can't lock {}: {why}", wt.folder_name()));
    }
    st.modal = Some(Modal::LockWorktree(LockForm::new(&wt)));
}

/// The Lock dialog's button.
pub(super) fn lock(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(Modal::LockWorktree(form)) = st.modal.as_ref() else { return };
    let form = form.clone();
    let Some(wt) = listed(st, &form.path) else {
        st.modal = None;
        return;
    };
    if let Some(why) = lock_refused(&wt) {
        return st.log(false, format!("Can't lock {}: {why}", form.name));
    }
    let before = fx.len();
    ops::start_job(st, JobSpec::new("Lock worktree".into(), vec![lock_cmd(&wt.path, Some(&form.reason))]), fx);
    if fx.len() > before {
        st.modal = None;
    }
}

pub(super) fn unlock(st: &mut AppState, path: PathBuf, fx: &mut Vec<Effect>) {
    let Some(wt) = listed(st, &path) else { return };
    if let Some(why) = unlock_refused(&wt) {
        return st.log(false, format!("Can't unlock {}: {why}", wt.folder_name()));
    }
    ops::start_job(st, JobSpec::new("Unlock worktree".into(), vec![unlock_cmd(&wt.path)]), fx);
}

pub(super) fn prune(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    if prune_blocked(snap).is_some() {
        return st.log(false, "Nothing to prune: every worktree's folder is present, or it is locked.");
    }
    ops::start_job(st, JobSpec::new("Prune worktrees".into(), vec![prune_cmd()]), fx);
}
