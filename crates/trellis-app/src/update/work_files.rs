//! Creating, renaming, deleting and discarding (untracked) working-tree files: the New file and
//! Rename dialogs (checked against the snapshot before anything runs), running the delete or
//! discard a confirmation agreed to (the confirmations themselves are in `work_files_delete`), and
//! the results (a toast, the dialog's inline error, or a log line).

use super::work_files_delete::{ask_delete, ask_discard, deleted_text, trash_refused};
use crate::effect::{DeleteMode, Effect, FileOp, WorkFileOk};
use crate::msg::WorkFileMsg;
use crate::state::files::{PathForm, listed, tracked};
use crate::state::{AppState, Modal};
use std::time::Instant;
use trellis_core::repo::{DiffTarget, PathProblem, check_new_path};

pub(super) fn handle(st: &mut AppState, msg: WorkFileMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        WorkFileMsg::NewFileDialog(prefill) => {
            if st.snap.is_some() {
                st.modal = Some(Modal::NewFile(PathForm::new(prefill)));
            }
        }
        WorkFileMsg::CreateFile(text) => create(st, text, fx),
        WorkFileMsg::RenameFileDialog(path) => rename_dialog(st, path),
        WorkFileMsg::RenameFile { from, to, stage } => rename(st, from, to, stage, fx),
        WorkFileMsg::SetRenameStages(on) if st.settings.stage_renames != on => {
            st.settings.stage_renames = on;
            fx.push(super::prefs::save_settings(st));
        }
        WorkFileMsg::DeleteFile { path, confirmed: false } => ask_delete(st, path),
        WorkFileMsg::DiscardUntracked(path) => ask_discard(st, path),
        WorkFileMsg::DeleteFile { path, confirmed: true } => {
            if let Some(workdir) = st.workdir() {
                let mode = DeleteMode::for_settings(&st.settings);
                fx.push(Effect::WorkFile { workdir, op: FileOp::Delete { path, mode } });
            }
        }
        WorkFileMsg::DeleteConfirmed(op) => match op {
            FileOp::Delete { .. } | FileOp::UnstageDelete { .. } | FileOp::UnstageGone(_) | FileOp::Discard { .. } => {
                if let Some(workdir) = st.workdir() {
                    fx.push(Effect::WorkFile { workdir, op });
                }
            }
            op => tracing::debug!("ignored a confirmed {op:?}, which isn't a delete"),
        },
        WorkFileMsg::TrashRefused { op, refusal } => {
            trash_refused(st, op, refusal);
            super::watch::settled(st, now);
            super::repo::refresh_status(st, fx);
        }
        WorkFileMsg::WorkFileDone { op, result } => {
            match result {
                Ok(ok) => succeeded(st, &op, ok, now),
                Err(e) => match form_for(&mut st.modal, &op) {
                    Some(form) => {
                        form.busy = false;
                        form.error = Some((form.text.clone(), e));
                    }
                    None => st.log(false, failure(&op, &e)),
                },
            }
            super::watch::settled(st, now);
            super::repo::refresh_status(st, fx);
        }
        WorkFileMsg::SetRenameStages(_) => {}
    }
}

/// A result for a tab that is no longer active: the error goes to the log, the success to a
/// toast, and a dialog still waiting for it is closed.
pub(super) fn inactive_result(st: &mut AppState, op: FileOp, result: Result<WorkFileOk, String>, now: Instant) {
    if matches!(&st.modal, Some(Modal::NewFile(f) | Modal::RenameFile { form: f, .. }) if f.busy) {
        st.modal = None;
    }
    match result {
        Ok(ok) => st.toast(true, success_text(&op, ok), now),
        Err(e) => st.log(false, failure(&op, &e)),
    }
}

/// The open dialog's form when it is the one `op` came from.
fn form_for<'a>(modal: &'a mut Option<Modal>, op: &FileOp) -> Option<&'a mut PathForm> {
    match (modal.as_mut()?, op) {
        (Modal::NewFile(form), FileOp::Create(_)) => Some(form),
        (Modal::RenameFile { old, form }, FileOp::Rename { from, .. }) if old == from => Some(form),
        _ => None,
    }
}

fn success_text(op: &FileOp, ok: WorkFileOk) -> String {
    let deleted = if let WorkFileOk::Deleted(d) = ok { d } else { trellis_core::repo::Deleted::Permanently };
    let (ignored, warnings) = match ok {
        WorkFileOk::Written { ignored, warnings } => (ignored, warnings),
        _ => (false, Vec::new()),
    };
    let mut text = match op {
        FileOp::Create(p) => format!("Created {p}."),
        FileOp::Rename { from, to, stage: true } => format!("Renamed {from} to {to} and staged the rename."),
        FileOp::Rename { from, to, stage: false } => format!("Renamed {from} to {to}."),
        FileOp::Delete { path, mode } => return deleted_text(path, *mode, deleted, false),
        FileOp::UnstageDelete { path, mode } => return deleted_text(path, *mode, deleted, true),
        FileOp::UnstageGone(p) => return format!("Removed {p} from the staging area."),
        FileOp::Discard { path, mode } => return deleted_text(path, *mode, deleted, false),
    };
    // A staged rename's new path stays tracked, so it is never "ignored".
    if ignored && !matches!(op, FileOp::Rename { stage: true, .. }) {
        text.push_str(" Git ignores it, so it isn't listed with your changes.");
    }
    for w in warnings {
        text.push(' ');
        text.push_str(&w);
    }
    text
}

fn failure(op: &FileOp, e: &str) -> String {
    match op {
        FileOp::Create(p) => format!("Could not create {p}: {e}"),
        FileOp::Rename { from, .. } => format!("Could not rename {from}: {e}"),
        FileOp::Delete { path: p, .. } | FileOp::UnstageDelete { path: p, .. } => format!("Could not delete {p}: {e}"),
        FileOp::UnstageGone(p) => format!("Could not unstage {p}: {e}"),
        FileOp::Discard { path: p, .. } => format!("Could not discard {p}: {e}"),
    }
}

fn succeeded(st: &mut AppState, op: &FileOp, ok: WorkFileOk, now: Instant) {
    if form_for(&mut st.modal, op).is_some() {
        st.modal = None;
    }
    st.toast(true, success_text(op, ok), now);
    // The old path's working-tree diff is gone; don't leave it on screen.
    let old = match op {
        FileOp::Rename { from, .. }
        | FileOp::Delete { path: from, .. }
        | FileOp::UnstageDelete { path: from, .. }
        | FileOp::UnstageGone(from)
        | FileOp::Discard { path: from, .. } => from,
        FileOp::Create(_) => return,
    };
    if matches!(&st.diff, Some((DiffTarget::Unstaged(p) | DiffTarget::Staged(p), _)) if p == old) {
        super::repo::close_diff(st);
    }
}

/// Shows `message` for `text` in the open dialog of `op`, or logs it when that dialog is closed.
fn fail(st: &mut AppState, op: &FileOp, text: &str, message: String) {
    match form_for(&mut st.modal, op) {
        Some(form) => {
            form.busy = false;
            form.error = Some((text.to_owned(), message));
        }
        None => st.log(false, failure(op, &message)),
    }
}

/// Marks the dialog of `op` busy; false when it is already waiting for a result.
fn start(st: &mut AppState, op: &FileOp) -> bool {
    match form_for(&mut st.modal, op) {
        Some(form) if form.busy => false,
        Some(form) => {
            form.busy = true;
            true
        }
        None => true,
    }
}

fn create(st: &mut AppState, text: String, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let checked = check_new_path(&text);
    let exists = checked.as_ref().is_ok_and(|p| listed(snap, p, None));
    let path = match checked {
        Ok(p) => p,
        Err(PathProblem::Empty) => return,
        Err(p) => return fail(st, &FileOp::Create(text.trim().to_owned()), &text, p.message()),
    };
    let op = FileOp::Create(path.clone());
    if exists {
        return fail(st, &op, &text, format!("{path} already exists."));
    }
    if let Some(workdir) = st.workdir()
        && start(st, &op)
    {
        fx.push(Effect::WorkFile { workdir, op });
    }
}

fn rename_dialog(st: &mut AppState, path: String) {
    let Some(snap) = &st.snap else { return };
    let mut entries = snap.unstaged.iter().chain(&snap.staged).filter(|f| f.path == path);
    let why = if snap.unstaged.iter().any(|f| f.path == path && f.status == '!') {
        format!("Resolve the conflict in {path} before renaming it.")
    } else if snap.missing_from_workdir(&path) {
        format!("{path} is not in your working tree.")
    } else if entries.next().is_none() {
        format!("{path} is not listed with your changes.")
    } else {
        st.modal = Some(Modal::RenameFile { old: path.clone(), form: PathForm::new(path) });
        return;
    };
    st.log(false, why);
}

fn rename(st: &mut AppState, from: String, to: String, stage: bool, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let checked = check_new_path(&to);
    let exists = checked.as_ref().is_ok_and(|p| listed(snap, p, Some(&from)));
    // Only a tracked file can be moved in the index; anything else is a plain rename.
    let stage = stage && tracked(snap, &from);
    let op = FileOp::Rename { from: from.clone(), to: checked.clone().unwrap_or_else(|_| to.trim().to_owned()), stage };
    let path = match checked {
        Ok(p) if p == from => return,
        Ok(p) => p,
        Err(PathProblem::Empty) => return,
        Err(p) => return fail(st, &op, &to, p.message()),
    };
    if exists {
        return fail(st, &op, &to, format!("{path} already exists."));
    }
    if let Some(workdir) = st.workdir()
        && start(st, &op)
    {
        fx.push(Effect::WorkFile { workdir, op });
    }
}
