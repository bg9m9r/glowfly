//! Deleting working-tree files and discarding untracked ones: the confirmations (worded by what
//! status says about the path and by whether the file goes to the Recycle Bin / Trash), the
//! follow-up that offers a permanent delete when the trash can't take a file, and the toasts that
//! say how the file went.
//!
//! The trash-or-permanent choice is made when a confirmation opens and travels inside it, so
//! changing the preference while it is up doesn't change what runs or what the text promised.

use crate::effect::{DeleteMode, FileOp};
use crate::msg::Msg;
use crate::msg::WorkFileMsg;
use crate::state::files::{TRASH, staged_rename, unstages_on_delete};
use crate::state::{AppState, Modal};
use trellis_core::repo::Deleted;

/// Opens the confirmation for deleting `path`, worded by what status says about it. Conflicted
/// paths are refused, and so are files already gone from the working tree unless they are staged
/// as new or as a rename's new path. Those get their own confirmation that also takes them out of
/// the index (only that, when the working copy is gone). The choice is made here and carried by
/// the confirmation, so a refresh before it is accepted can't change what runs.
pub(super) fn ask_delete(st: &mut AppState, path: String) {
    let mode = DeleteMode::for_settings(&st.settings);
    let Some(snap) = &st.snap else { return };
    let unstaged = snap.unstaged.iter().find(|f| f.path == path);
    if unstaged.is_some_and(|f| f.status == '!') {
        return st.log(false, format!("Resolve the conflict in {path} before deleting it."));
    }
    let gone = snap.missing_from_workdir(&path);
    if unstages_on_delete(snap, &path) {
        st.modal = Some(unstage_confirm(path.clone(), staged_rename(snap, &path), gone, unstaged.is_some(), mode));
        return;
    }
    if gone {
        return st.log(false, format!("{path} is not in your working tree."));
    }
    let staged = snap.staged.iter().any(|f| f.path == path);
    let trash = mode == DeleteMode::Trash;
    let body = match unstaged {
        Some(f) if f.status == '?' && trash => {
            format!("{path} isn't tracked by git. It goes to the {TRASH}, so you can restore it from there.")
        }
        Some(f) if f.status == '?' => format!("{path} isn't tracked by git, so deleting it can't be undone."),
        Some(_) if trash => format!(
            "{path} goes to the {TRASH} with your uncommitted changes. Discarding the deletion afterwards brings back its staged or committed version instead."
        ),
        Some(_) => format!(
            "Your uncommitted changes to {path} are lost. Discarding the deletion afterwards brings back its staged or committed version."
        ),
        None if staged && trash => format!("{path} goes to the {TRASH}. Discarding the deletion brings it back as it is now."),
        None if staged => format!("{path} is deleted from your working tree. Discarding the deletion brings it back as it is now."),
        None if trash => format!("{path} goes to the {TRASH}. Discarding the deletion brings it back from the last commit."),
        None => format!("{path} is deleted from your working tree. Discarding the deletion brings it back from the last commit."),
    };
    st.modal = Some(Modal::Confirm {
        title: "Delete file".into(),
        body,
        button: if trash { format!("Move to {TRASH}") } else { "Delete".into() },
        then: Box::new(Msg::WorkFile(WorkFileMsg::DeleteConfirmed(FileOp::Delete { path, mode }))),
    });
}

/// Opens the confirmation for discarding the untracked file `path`, worded by whether it goes
/// to the Recycle Bin / Trash. The choice is fixed here and carried by the confirmation.
pub(super) fn ask_discard(st: &mut AppState, path: String) {
    let mode = DeleteMode::for_settings(&st.settings);
    let untracked = st.snap.as_ref().is_some_and(|snap| snap.unstaged.iter().any(|f| f.path == path && f.status == '?'));
    if !untracked {
        return st.log(false, format!("{path} is no longer an untracked file; refresh and try again."));
    }
    let (body, button) = if mode == DeleteMode::Trash {
        (
            format!("{path} isn't tracked by git. Discarding it moves it to the {TRASH}, so you can restore it from there."),
            format!("Move to {TRASH}"),
        )
    } else {
        (format!("Delete untracked file {path}? This cannot be undone."), "Discard".to_owned())
    };
    st.modal = Some(Modal::Confirm {
        title: "Discard changes".into(),
        body,
        button,
        then: Box::new(Msg::WorkFile(WorkFileMsg::DeleteConfirmed(FileOp::Discard { path, mode }))),
    });
}

/// The confirmation for `path`, staged as new or (`renamed`) as a staged rename's new path:
/// unstage only when the working copy is `gone`, otherwise unstage and delete the way `mode`
/// says, warning about unstaged `edits`.
pub(super) fn unstage_confirm(path: String, renamed: bool, gone: bool, edits: bool, mode: DeleteMode) -> Modal {
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
        let then = Box::new(Msg::WorkFile(WorkFileMsg::DeleteConfirmed(FileOp::UnstageGone(path))));
        return Modal::Confirm { title: "Unstage deleted file".into(), body, button: "Unstage".into(), then };
    }
    let trash = mode == DeleteMode::Trash;
    let mut body = match (renamed, trash) {
        (true, true) => format!(
            "{path} is staged as a rename. It goes to the {TRASH} and the rename is unstaged: the file it was renamed from stays deleted in your working tree as an unstaged change, and discarding that brings back its committed version."
        ),
        (true, false) => format!(
            "{path} is staged as a rename. Deleting it also unstages the rename: the file it was renamed from stays deleted in your working tree as an unstaged change, and discarding that brings back its committed version. Staged changes to {path} can't be brought back."
        ),
        (false, true) => format!("{path} is a new file in the staging area. It goes to the {TRASH} and leaves the staging area."),
        (false, false) => {
            format!(
                "{path} is a new file in the staging area. Deleting it also takes it out of the staging area, so it can't be brought back."
            )
        }
    };
    if edits {
        body.push_str(&if trash {
            format!(" The copy in the {TRASH} has your unstaged edits; the staged version can't be brought back.")
        } else {
            " Your unstaged edits to it are lost too.".to_owned()
        });
    }
    let title = if renamed { "Delete renamed file" } else { "Delete new file" };
    let then = Box::new(Msg::WorkFile(WorkFileMsg::DeleteConfirmed(FileOp::UnstageDelete { path, mode })));
    Modal::Confirm { title: title.into(), body, button: "Unstage and delete".into(), then }
}

/// The trash couldn't take `op`'s file and nothing changed: ask before deleting it permanently.
pub(super) fn trash_refused(st: &mut AppState, op: FileOp, reason: String) {
    let Some(path) = op.delete_path().map(str::to_owned) else {
        return tracing::debug!("the trash refused {op:?}, which isn't a delete");
    };
    let reason = reason.trim_end_matches('.');
    let mut body = format!("{path} couldn't be moved to the {TRASH}: {reason}.\n\nDelete it permanently instead? This can't be undone.");
    if matches!(op, FileOp::UnstageDelete { .. }) {
        body.push_str(" It also leaves the staging area.");
    }
    st.modal = Some(Modal::Confirm {
        title: "Delete permanently?".into(),
        body,
        button: "Delete permanently".into(),
        then: Box::new(Msg::WorkFile(WorkFileMsg::DeleteConfirmed(op.permanent()))),
    });
}

/// The toast after `path` was deleted (and, when `unstaged`, taken out of the index).
pub(super) fn deleted_text(path: &str, mode: DeleteMode, deleted: Deleted, unstaged: bool) -> String {
    let also = if unstaged { " and removed it from the staging area" } else { "" };
    match (deleted, mode) {
        (Deleted::Trashed, _) => format!("Moved {path} to the {TRASH}{also}."),
        (Deleted::LinkRemoved, _) => {
            format!("Deleted the symbolic link {path}{also}. Links don't go to the {TRASH}; what it points to is untouched.")
        }
        (Deleted::Permanently, DeleteMode::PermanentNoTrash) => {
            format!("Deleted {path} permanently{also}; the {TRASH} couldn't take it.")
        }
        (Deleted::Permanently, _) => format!("Deleted {path}{also}."),
    }
}
