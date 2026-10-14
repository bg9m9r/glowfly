//! Creating, renaming and deleting working-tree files: the New file and Rename dialogs (checked
//! against the snapshot before anything runs), the delete confirmations (a staged new file or a
//! staged rename's new path is also taken out of the index, and one already gone from the working
//! tree is only unstaged), and the results (a toast, the dialog's inline error, or a log line).

use crate::effect::{Effect, FileOp};
use crate::msg::Msg;
use crate::state::files::{PathForm, listed, staged_rename, tracked, unstages_on_delete};
use crate::state::{AppState, Modal};
use std::time::Instant;
use trellis_core::repo::{DiffTarget, PathProblem, check_new_path};

pub(super) fn handle(st: &mut AppState, msg: Msg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        Msg::NewFileDialog(prefill) => {
            if st.snap.is_some() {
                st.modal = Some(Modal::NewFile(PathForm::new(prefill)));
            }
        }
        Msg::CreateFile(text) => create(st, text, fx),
        Msg::RenameFileDialog(path) => rename_dialog(st, path),
        Msg::RenameFile { from, to, stage } => rename(st, from, to, stage, fx),
        Msg::SetRenameStages(on) if st.settings.stage_renames != on => {
            st.settings.stage_renames = on;
            fx.push(super::prefs::save_settings(st));
        }
        Msg::DeleteFile { path, confirmed: false } => ask_delete(st, path),
        Msg::DeleteFile { path, confirmed: true } => {
            if let Some(workdir) = st.workdir() {
                fx.push(Effect::WorkFile { workdir, op: FileOp::Delete(path) });
            }
        }
        Msg::DeleteStagedNewFile(path) => {
            if let Some(workdir) = st.workdir() {
                fx.push(Effect::WorkFile { workdir, op: FileOp::UnstageDelete(path) });
            }
        }
        Msg::UnstageGoneFile(path) => {
            if let Some(workdir) = st.workdir() {
                fx.push(Effect::WorkFile { workdir, op: FileOp::UnstageGone(path) });
            }
        }
        Msg::WorkFileDone { op, result } => {
            match result {
                Ok(ignored) => succeeded(st, &op, ignored, now),
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
        _ => {}
    }
}

/// A result for a tab that is no longer active: the error goes to the log, the success to a
/// toast, and a dialog still waiting for it is closed.
pub(super) fn inactive_result(st: &mut AppState, op: FileOp, result: Result<bool, String>, now: Instant) {
    if matches!(&st.modal, Some(Modal::NewFile(f) | Modal::RenameFile { form: f, .. }) if f.busy) {
        st.modal = None;
    }
    match result {
        Ok(ignored) => st.toast(true, success_text(&op, ignored), now),
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

fn success_text(op: &FileOp, ignored: bool) -> String {
    let mut text = match op {
        FileOp::Create(p) => format!("Created {p}."),
        // The new path stays tracked, so the result is never "ignored".
        FileOp::Rename { from, to, stage: true } => return format!("Renamed {from} to {to} and staged the rename."),
        FileOp::Rename { from, to, stage: false } => format!("Renamed {from} to {to}."),
        FileOp::Delete(p) => format!("Deleted {p}."),
        FileOp::UnstageDelete(p) => return format!("Deleted {p} and removed it from the staging area."),
        FileOp::UnstageGone(p) => return format!("Removed {p} from the staging area."),
    };
    if ignored {
        text.push_str(" Git ignores it, so it isn't listed with your changes.");
    }
    text
}

fn failure(op: &FileOp, e: &str) -> String {
    match op {
        FileOp::Create(p) => format!("Could not create {p}: {e}"),
        FileOp::Rename { from, .. } => format!("Could not rename {from}: {e}"),
        FileOp::Delete(p) | FileOp::UnstageDelete(p) => format!("Could not delete {p}: {e}"),
        FileOp::UnstageGone(p) => format!("Could not unstage {p}: {e}"),
    }
}

fn succeeded(st: &mut AppState, op: &FileOp, ignored: bool, now: Instant) {
    if form_for(&mut st.modal, op).is_some() {
        st.modal = None;
    }
    st.toast(true, success_text(op, ignored), now);
    // The old path's working-tree diff is gone; don't leave it on screen.
    let old = match op {
        FileOp::Rename { from, .. } | FileOp::Delete(from) | FileOp::UnstageDelete(from) | FileOp::UnstageGone(from) => from,
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

/// Opens the confirmation for deleting `path`, worded by what status says about it. Conflicted
/// paths are refused, and so are files already gone from the working tree unless they are staged
/// as new or as a rename's new path. Those get their own confirmation that also takes them out of
/// the index (only that, when the working copy is gone). The choice is made here and carried by
/// the confirmation, so a refresh before it is accepted can't change what runs.
fn ask_delete(st: &mut AppState, path: String) {
    let Some(snap) = &st.snap else { return };
    let unstaged = snap.unstaged.iter().find(|f| f.path == path);
    if unstaged.is_some_and(|f| f.status == '!') {
        return st.log(false, format!("Resolve the conflict in {path} before deleting it."));
    }
    let gone = snap.missing_from_workdir(&path);
    if unstages_on_delete(snap, &path) {
        st.modal = Some(unstage_confirm(path.clone(), staged_rename(snap, &path), gone, unstaged.is_some()));
        return;
    }
    if gone {
        return st.log(false, format!("{path} is not in your working tree."));
    }
    let body = match unstaged {
        Some(f) if f.status == '?' => format!("{path} isn't tracked by git, so deleting it can't be undone."),
        Some(_) => format!(
            "Your uncommitted changes to {path} are lost. Discarding the deletion afterwards brings back its staged or committed version."
        ),
        None if snap.staged.iter().any(|f| f.path == path) => {
            format!("{path} is deleted from your working tree. Discarding the deletion brings it back as it is now.")
        }
        None => format!("{path} is deleted from your working tree. Discarding the deletion brings it back from the last commit."),
    };
    st.modal = Some(Modal::Confirm {
        title: "Delete file".into(),
        body,
        button: "Delete".into(),
        then: Box::new(Msg::DeleteFile { path, confirmed: true }),
    });
}

/// The confirmation for `path`, staged as new or (`renamed`) as a staged rename's new path:
/// unstage only when the working copy is `gone`, otherwise unstage and delete, warning about
/// unstaged `edits`.
fn unstage_confirm(path: String, renamed: bool, gone: bool, edits: bool) -> Modal {
    if gone {
        let body = if renamed {
            format!(
                "{path} is staged as a rename but is no longer in your working tree. This removes the rename from the staging area; the file it was renamed from shows as deleted in your working tree, and discarding that brings back its committed version."
            )
        } else {
            format!(
                "{path} is staged as a new file but is no longer in your working tree. This removes it from the staging area too, so its staged version can't be brought back."
            )
        };
        let then = Box::new(Msg::UnstageGoneFile(path));
        return Modal::Confirm { title: "Unstage deleted file".into(), body, button: "Unstage".into(), then };
    }
    let mut body = if renamed {
        format!(
            "{path} is staged as a rename. Deleting it also unstages the rename: the file it was renamed from stays deleted in your working tree as an unstaged change, and discarding that brings back its committed version. Staged changes to {path} can't be brought back."
        )
    } else {
        format!("{path} is a new file in the staging area. Deleting it also takes it out of the staging area, so it can't be brought back.")
    };
    if edits {
        body.push_str(" Your unstaged edits to it are lost too.");
    }
    let title = if renamed { "Delete renamed file" } else { "Delete new file" };
    Modal::Confirm { title: title.into(), body, button: "Unstage and delete".into(), then: Box::new(Msg::DeleteStagedNewFile(path)) }
}
