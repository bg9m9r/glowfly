//! Removing, locking, unlocking and pruning worktrees from the WORKTREES section. Every request
//! is checked again against the current snapshot. Remove closes the other tabs showing the
//! worktree first, then runs as its own effect (`Effect::RemoveWorktree`: a lock or changes are
//! found before git runs, and the branch, and that branch on its remote, are deleted afterwards)
//! and comes back as `WorktreeMsg::Removed`; the others are plain git jobs.

use super::ops::{self, JobSpec};
use crate::effect::{Effect, UndoResult};
use crate::state::worktree_manage::{
    LockForm, RemoveForm, branch_failed_text, folder_left_headline, gone_text, lock_refused, needs_force_text, prune_blocked,
    remote_delete, remote_failed_text, remote_kept_text, remove_refused, removed_headline, showing_tabs, unlock_refused,
};
use crate::state::worktrees::find;
use crate::state::{AppState, Modal};
use crate::worktree::{RemoteOutcome, RemoveRequest};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::repo::{Tracking, WorktreeInfo};
use trellis_core::undo::Action;
use trellis_core::worktree_ops::{Refusal, RemoveOutcome, lock_cmd, prune_cmd, unlock_cmd};

/// `WorktreeMsg::Removed`'s fields.
pub(super) struct Removed {
    pub request: RemoveRequest,
    pub outcome: RemoveOutcome,
    pub output: String,
    pub branch: Option<Result<(), String>>,
    pub remote: Option<RemoteOutcome>,
    pub undo: Option<Action>,
}

/// The folders open in tabs.
fn open_tabs(st: &AppState) -> Vec<crate::state::tabs::Place> {
    crate::state::tabs::open_places(&st.tabs)
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
    if let Some(why) = remove_refused(&wt) {
        return st.log(false, format!("Can't remove {}: {why}", wt.folder_name()));
    }
    let open = open_tabs(st);
    let Some(snap) = st.snap.as_ref() else { return };
    st.modal = Some(Modal::RemoveWorktree(RemoveForm::new(&wt).with_snap(snap, &open)));
}

/// Why the remote branch the form would delete can't be deleted now: the branch no longer has
/// that upstream, or HEAD no longer contains it. Updates what the form offers.
fn remote_changed(st: &AppState, form: &mut RemoveForm) -> Option<String> {
    let asked = form.remote_ticked().cloned()?;
    let now = st.snap.as_ref().and_then(|s| remote_delete(s, form.branch.as_deref()));
    let why = match &now {
        Some(r) if r.tracking == asked => r.blocked.clone(),
        _ => Some(format!("{} no longer tracks {}; nothing was removed.", form.branch.as_deref().unwrap_or_default(), asked.display())),
    };
    form.remote = now;
    why
}

/// The Remove dialog's button.
pub(super) fn remove(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(Modal::RemoveWorktree(form)) = st.modal.as_ref() else { return };
    let mut form = form.clone();
    let Some(wt) = listed(st, &form.path) else {
        st.modal = None;
        return;
    };
    if let Some(why) = remove_refused(&wt) {
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
    if let Some(why) = remote_changed(st, &mut form) {
        st.log(false, why);
        st.modal = Some(Modal::RemoveWorktree(form));
        return;
    }
    // Opened in another tab since the dialog opened: say so first, as for a lock.
    let showing: Vec<usize> = showing_tabs(&st.tabs, &wt).into_iter().filter(|&i| i != st.active_tab).collect();
    if !showing.is_empty() && !form.close_tabs {
        st.log(false, format!("{} is now open in another tab. Remove closes it.", form.name));
        form.close_tabs = true;
        st.modal = Some(Modal::RemoveWorktree(form));
        return;
    }
    if ops::busy(st, false) {
        return;
    }
    super::tabs::close_tabs(st, &showing, fx);
    let (name, force) = (form.name.clone(), form.force());
    let delete_remote = form.remote_ticked().cloned();
    let delete_branch = form.delete_branch.then(|| form.branch.clone()).flatten();
    let effect = move |repo| Effect::RemoveWorktree(RemoveRequest { repo, path: wt.path, name, force, delete_branch, delete_remote });
    if ops::begin_job(st, "Remove worktree", true, effect, fx) == ops::Started::Now {
        st.modal = None;
    }
}

/// What a remote branch delete that didn't happen leaves to say, after the branch was deleted.
fn remote_text(name: &str, branch: &str, remote: Option<&str>, outcome: Option<&RemoteOutcome>) -> Option<String> {
    match (remote, outcome) {
        (Some(r), Some(RemoteOutcome::Failed(e))) => Some(remote_failed_text(name, branch, r, e)),
        (Some(r), Some(RemoteOutcome::Kept)) => Some(remote_kept_text(name, branch, r)),
        _ => None,
    }
}

/// The remove finished: one toast and log entry for the outcome (plus a second entry for a
/// branch failure when the folder-left report took the toast). A remove that needs force opens
/// the dialog again in force mode.
pub(super) fn removed(st: &mut AppState, r: Removed, now: Instant, fx: &mut Vec<Effect>) {
    let Some(job) = &st.job else { return };
    let here = match (job.workdir.as_deref(), st.workdir()) {
        (Some(a), Some(b)) => trellis_core::paths::same_spelling(a, &b),
        _ => false,
    };
    let name = r.request.name.as_str();
    let branch = r.request.delete_branch.as_deref().unwrap_or_default();
    let remote = r.request.delete_remote.as_ref().map(Tracking::display);
    let branch_err = |e: &str| match &remote {
        Some(rn) => format!("{} {rn} was kept.", branch_failed_text(name, branch, e)),
        None => branch_failed_text(name, branch, e),
    };
    // After the branch was deleted: the remote branch's failure, if any.
    let remote_err = remote_text(name, branch, remote.as_deref(), r.remote.as_ref());
    let deleted_remote = remote.as_deref().filter(|_| r.remote == Some(RemoteOutcome::Deleted));
    let (ok, headline, extra) = match (&r.outcome, &r.branch) {
        (RemoveOutcome::Removed, None) => (true, Some(removed_headline(name, None, None)), None),
        (RemoveOutcome::Removed, Some(Ok(()))) => match remote_err {
            Some(text) => (false, Some(text), None),
            None => (true, Some(removed_headline(name, Some(branch), deleted_remote)), None),
        },
        (RemoveOutcome::Removed, Some(Err(e))) => (false, Some(branch_err(e)), None),
        (RemoveOutcome::FolderLeft, b) => {
            let extra = match b {
                Some(Err(e)) => Some(branch_err(e)),
                _ => remote_err,
            };
            (false, Some(folder_left_headline(name, &r.request.path)), extra)
        }
        (RemoveOutcome::NeedsForce(refusal), _) => (false, Some(needs_force_text(name, refusal)), None),
        (RemoveOutcome::Failed, _) => (false, None, None),
    };
    // The log entry starts with what happened; the git output follows.
    let output = match &headline {
        Some(h) if !ok || r.output.is_empty() => format!("{h}\n{}", r.output).trim_end().to_owned(),
        _ => r.output.clone(),
    };
    // A branch deleted next to a folder that was left, or before the remote delete failed, is
    // still undoable, though the job failed.
    let left_undo = (!ok).then(|| r.undo.clone()).flatten();
    let undo = r.request.delete_branch.is_some().then(|| UndoResult::Recorded(r.undo.clone()));
    ops::finish_job(st, ok, output, headline, undo, now, fx);
    if let Some(e) = extra {
        st.log(false, e);
    }
    if here && let Some(a) = left_undo {
        st.undo.record(a);
    }
    let open = open_tabs(st);
    if let RemoveOutcome::NeedsForce(refusal) = r.outcome
        && here
        && st.modal.is_none()
        && let Some(snap) = st.snap.as_ref()
        && let Some(wt) = find(snap, &r.request.path)
    {
        let form = RemoveForm {
            refusal: Some(refusal),
            delete_branch: r.request.delete_branch.is_some(),
            delete_remote: r.request.delete_remote.is_some(),
            ..RemoveForm::new(wt).with_snap(snap, &open)
        };
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
    if let Some(why) = prune_blocked(snap) {
        return st.log(false, format!("Can't prune: {why}"));
    }
    ops::start_job(st, JobSpec::new("Prune worktrees".into(), vec![prune_cmd()]), fx);
}
