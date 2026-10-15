//! The WORKTREES section and the Create worktree dialog: opening a worktree in a tab, and
//! creating one. Creating checks the form again, then the folder on disk
//! (`Effect::CheckWorktreeFolder`), then runs `git worktree add` as a job. Removing, locking,
//! unlocking and pruning are in `worktree_manage.rs`.

use super::ops::{self, JobSpec};
use super::worktree_manage as manage;
use crate::effect::Effect;
use crate::state::worktrees::{WorktreeForm, find, open_blocked, recheck, request};
use crate::state::{AppState, Modal};
use crate::worktree::{NewWorktree, WorktreeMsg};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::worktree_ops::add_cmd;

/// Logged when the folder check comes back for a tab that is no longer active.
const TAB_GONE: &str = "The worktree was not created: its repository's tab is no longer active.";

pub(super) fn handle(st: &mut AppState, msg: WorktreeMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        WorktreeMsg::Open(path) => open(st, path, fx),
        WorktreeMsg::CreateDialog(base) => {
            let Some(snap) = st.snap.as_ref() else { return };
            st.modal = Some(Modal::NewWorktree(WorktreeForm::new(snap, base)));
        }
        WorktreeMsg::Submit => submit(st, fx),
        WorktreeMsg::FolderChecked { request, result } => folder_checked(st, request, result, fx),
        WorktreeMsg::RemoveDialog(path) => manage::remove_dialog(st, path),
        WorktreeMsg::Remove => manage::remove(st, fx),
        WorktreeMsg::Removed { request, outcome, output, branch, undo } => {
            manage::removed(st, manage::Removed { request, outcome, output, branch, undo }, now, fx)
        }
        WorktreeMsg::LockDialog(path) => manage::lock_dialog(st, path),
        WorktreeMsg::Lock => manage::lock(st, fx),
        WorktreeMsg::Unlock(path) => manage::unlock(st, path, fx),
        WorktreeMsg::Prune => manage::prune(st, fx),
    }
}

fn open(st: &mut AppState, path: PathBuf, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    let Some(wt) = find(snap, &path).cloned() else {
        st.log(false, format!("{} is no longer a worktree of this repository.", path.display()));
        return;
    };
    if let Some(why) = open_blocked(&wt) {
        st.log(false, format!("Can't open {}: {why}", wt.folder_name()));
        return;
    }
    super::tabs::open(st, wt.path, fx);
}

/// The dialog's Create worktree: checks the form again, then asks the runner about the folder.
fn submit(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(Modal::NewWorktree(form)) = st.modal.as_ref() else { return };
    if form.busy() {
        return;
    }
    let Some(snap) = st.snap.as_ref() else { return };
    let req = match request(snap, form) {
        Ok(r) => r,
        Err(e) => return st.log(false, e),
    };
    if ops::busy(st, false) {
        return;
    }
    if let Some(Modal::NewWorktree(form)) = st.modal.as_mut() {
        form.pending = Some(req.clone());
    }
    fx.push(Effect::CheckWorktreeFolder(req));
}

/// The disk check's result. It acts only while the dialog that asked is still open and waiting
/// for this request: after Cancel, or for an older request than the open dialog's, it is dropped.
fn folder_checked(st: &mut AppState, req: NewWorktree, result: Result<(), String>, fx: &mut Vec<Effect>) {
    match st.modal.as_mut() {
        Some(Modal::NewWorktree(form)) if form.pending.as_ref() == Some(&req) => form.pending = None,
        _ => {
            tracing::debug!(folder = %req.folder.display(), "dropped a folder check: its dialog was closed");
            return;
        }
    }
    let here = st.workdir().is_some_and(|w| crate::state::tabs::same_path(&w, &req.repo));
    if !here {
        st.log(false, TAB_GONE);
        close(st);
        return;
    }
    if let Err(e) = result {
        match st.modal.as_mut() {
            Some(Modal::NewWorktree(form)) => form.error = Some((form.folder.clone(), e)),
            _ => st.log(false, e),
        }
        return;
    }
    let Some(snap) = st.snap.as_ref() else { return };
    if let Some(p) = recheck(snap, &req) {
        st.log(false, p);
        return;
    }
    let cmd = add_cmd(&req.folder, &req.base, req.branch.as_deref());
    let spec = JobSpec {
        then_open: req.open.then(|| (req.folder.clone(), crate::state::trust::Created::Worktree)),
        ..JobSpec::new("Create worktree".into(), vec![cmd])
    };
    let before = fx.len();
    ops::start_job(st, spec, fx);
    if fx.len() > before {
        close(st);
    }
}

fn close(st: &mut AppState) {
    if matches!(st.modal, Some(Modal::NewWorktree(_))) {
        st.modal = None;
    }
}

/// Browse… picked `parent`: the folder becomes the slug inside it.
pub(super) fn parent_picked(st: &mut AppState, parent: PathBuf) {
    if let Some(Modal::NewWorktree(form)) = st.modal.as_mut() {
        form.folder = parent.join(form.slug()).display().to_string();
        form.folder_edited = true;
    }
}
