//! The WORKTREES section and the Create worktree dialog: opening a worktree in a tab, and
//! creating one (from a branch, a commit, a remote branch or a pull request's source branch).
//! Creating checks the form again, then the folder on disk (`Effect::CheckWorktreeFolder`), then
//! runs `git worktree add` as a job. Removing, locking, unlocking and pruning are in
//! `worktree_manage.rs`, moving in `worktree_move.rs`.

use super::ops::{self, JobSpec};
use super::worktree_manage as manage;
use super::worktree_move;
use crate::effect::Effect;
use crate::state::trust::InheritKind;
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
        WorktreeMsg::CreateForPr { id, source } => create_for_pr(st, id, &source),
        WorktreeMsg::Submit => submit(st, fx),
        WorktreeMsg::FolderChecked { request, result } => folder_checked(st, request, result, fx),
        WorktreeMsg::RemoveDialog(path) => manage::remove_dialog(st, path),
        WorktreeMsg::Remove => manage::remove(st, fx),
        WorktreeMsg::Removed(r) => manage::removed(st, *r, now, fx),
        WorktreeMsg::LockDialog(path) => manage::lock_dialog(st, path),
        WorktreeMsg::Lock => manage::lock(st, fx),
        WorktreeMsg::Unlock(path) => manage::unlock(st, path, fx),
        WorktreeMsg::Prune => manage::prune(st, fx),
        WorktreeMsg::MoveDialog(path) => worktree_move::move_dialog(st, path),
        WorktreeMsg::Move => worktree_move::submit(st, fx),
        WorktreeMsg::Moved { request, outcome, output } => worktree_move::moved(st, request, outcome, output, now, fx),
        WorktreeMsg::ChangesCounted { refs_key, counts } => super::worktree_counts::counted(st, refs_key, counts, now, fx),
    }
}

/// Opens the Create worktree dialog for a pull request's source branch: the local branch of that
/// name, else the code host remote's branch (`worktree_branches::pr_base`).
fn create_for_pr(st: &mut AppState, id: u64, source: &str) {
    let (Some(snap), Some(f)) = (st.snap.as_ref(), st.forge.as_ref()) else { return };
    let (noun, label) = (f.pr_noun(), f.pr_label(id));
    match crate::state::worktree_branches::pr_base(snap, &f.remote, source) {
        Ok(base) => {
            let form = WorktreeForm { title: Some(format!("Create worktree for {noun} {label}")), ..WorktreeForm::new(snap, base) };
            st.modal = Some(Modal::NewWorktree(form));
        }
        Err(why) => st.log(false, format!("Can't create a worktree for {noun} {label}: {why}")),
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
    let here = st.workdir().is_some_and(|w| trellis_core::paths::same_spelling(&w, &req.repo));
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
    if ops::start_job(st, spec, fx) == ops::Started::Now {
        super::trust_inherit::expect(st, "Create worktree", InheritKind::Worktree, vec![req.folder.clone()]);
        close(st);
    }
}

fn close(st: &mut AppState) {
    if matches!(st.modal, Some(Modal::NewWorktree(_))) {
        st.modal = None;
    }
}

/// Browse… picked `parent`: the folder becomes the slug inside it (Create), or the worktree's
/// own folder name inside it (Move).
pub(super) fn parent_picked(st: &mut AppState, parent: PathBuf) {
    match st.modal.as_mut() {
        Some(Modal::NewWorktree(form)) => {
            form.folder = parent.join(form.slug()).display().to_string();
            form.folder_edited = true;
        }
        Some(Modal::MoveWorktree(form)) => form.parent_picked(&parent),
        _ => {}
    }
}
