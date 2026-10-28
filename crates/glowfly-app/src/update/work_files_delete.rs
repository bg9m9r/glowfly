//! Deleting working-tree files and discarding untracked ones: the confirmations (worded by what
//! status says about the path and by whether the file goes to the Recycle Bin / Trash), the
//! follow-up that offers a permanent delete when the trash can't take a file, and the toasts that
//! say how the file went.
//!
//! The trash-or-permanent choice is made when a confirmation opens and travels inside it, so
//! changing the preference while it is up doesn't change what runs or what the text promised.

use crate::effect::{DeleteMode, FileOp};
use crate::keymap::BindingId;
use crate::msg::Msg;
use crate::msg::WorkFileMsg;
use crate::state::files::TrashOffer;
use crate::state::files::{TRASH, staged_rename, unstages_on_delete};
use crate::state::{AppState, Modal};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{Deleted, TrashProblem, TrashRefusal};

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
    let hint = st.keymap.paren(BindingId::Undo);
    if unstages_on_delete(snap, &path) {
        let t = st.settings.lang();
        st.modal = Some(unstage_confirm(t, path.clone(), staged_rename(snap, &path), gone, unstaged.is_some(), mode, &hint));
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
        Some(f) if f.status == '?' => format!(
            "{path} isn't tracked by git. Glowfly keeps a copy so Undo{hint} can bring it back while this tab is open (files up to 64 MB)."
        ),
        Some(_) if trash => format!(
            "{path} goes to the {TRASH} with your uncommitted changes. Discarding the deletion afterwards brings back its staged or committed version instead."
        ),
        Some(_) => format!(
            "{path} is deleted with your uncommitted changes; Undo{hint} brings them back. Discarding the deletion instead brings back its staged or committed version."
        ),
        None if staged && trash => format!("{path} goes to the {TRASH}. Discarding the deletion brings it back as it is now."),
        None if staged => format!("{path} is deleted from your working tree. Discarding the deletion brings it back as it is now."),
        None if trash => format!("{path} goes to the {TRASH}. Discarding the deletion brings it back from the last commit."),
        None => format!("{path} is deleted from your working tree. Discarding the deletion brings it back from the last commit."),
    };
    let button = if trash { format!("Move to {TRASH}") } else { "Delete".into() };
    st.modal = Some(Modal::danger("Delete file", body, button, Msg::WorkFile(WorkFileMsg::DeleteConfirmed(FileOp::Delete { path, mode }))));
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
        // Glowfly keeps a copy for Undo, as for Delete.
        let hint = st.keymap.paren(BindingId::Undo);
        (st.settings.lang().trf("files.discard.permanent_body", &[("path", &path), ("hint", &hint)]), "Discard".to_owned())
    };
    st.modal =
        Some(Modal::danger("Discard changes", body, button, Msg::WorkFile(WorkFileMsg::DeleteConfirmed(FileOp::Discard { path, mode }))));
}

/// The confirmation for `path`, staged as new or (`renamed`) as a staged rename's new path:
/// unstage only when the working copy is `gone`, otherwise unstage and delete the way `mode`
/// says, warning about unstaged `edits`. `hint` is the Undo key (" (Ctrl+Z)").
pub(super) fn unstage_confirm(t: Lang, path: String, renamed: bool, gone: bool, edits: bool, mode: DeleteMode, hint: &str) -> Modal {
    if gone {
        let body = if renamed {
            format!(
                "{path} is staged as a rename but is no longer in your working tree. This removes the rename from the staging area; the file it was renamed from shows as deleted in your working tree, and discarding that brings back its committed version."
            )
        } else {
            t.trf("files.unstage_gone.body", &[("path", &path), ("hint", &hint)])
        };
        let then = Box::new(Msg::WorkFile(WorkFileMsg::DeleteConfirmed(FileOp::UnstageGone(path))));
        return Modal::Confirm { title: "Unstage deleted file".into(), body, button: "Unstage".into(), then };
    }
    let trash = mode == DeleteMode::Trash;
    let mut body = match (renamed, trash) {
        (true, true) => format!(
            "{path} is staged as a rename. It goes to the {TRASH} and the rename is unstaged: the file it was renamed from stays deleted in your working tree as an unstaged change, and discarding that brings back its committed version. Undo{hint} brings back the file and the staged rename."
        ),
        (true, false) => format!(
            "{path} is staged as a rename. Deleting it also unstages the rename: the file it was renamed from stays deleted in your working tree as an unstaged change, and discarding that brings back its committed version. Undo{hint} brings back the file and the staged rename."
        ),
        (false, true) => {
            format!(
                "{path} is a new file in the staging area. It goes to the {TRASH} and leaves the staging area; Undo{hint} stages it again."
            )
        }
        (false, false) => format!(
            "{path} is a new file in the staging area. Deleting it also takes it out of the staging area; Undo{hint} brings back both."
        ),
    };
    // Undo keeps the edits in the permanent case too: they are the bytes it puts back.
    if edits && trash {
        body.push_str(&format!(" The copy in the {TRASH} has your unstaged edits; Undo{hint} brings back the staged version too."));
    }
    let title = if renamed { "Delete renamed file" } else { "Delete new file" };
    let then = Msg::WorkFile(WorkFileMsg::DeleteConfirmed(FileOp::UnstageDelete { path, mode }));
    Modal::danger(title, body, "Unstage and delete", then)
}

/// The trash couldn't take `op`'s file and nothing changed. Only when there is definitely no
/// trash for it is a permanent delete offered; a file in use, a permission problem or anything
/// else gets its own message and Retry, since deleting it for good would lose what the trash
/// was meant to keep.
pub(super) fn trash_refused(st: &mut AppState, op: FileOp, refusal: TrashRefusal) {
    let Some(path) = op.delete_path().map(str::to_owned) else {
        return tracing::debug!("the trash refused {op:?}, which isn't a delete");
    };
    let detail = refusal.detail.trim().trim_end_matches('.');
    let retry = |title: String, body: String| Modal::Confirm {
        title,
        body,
        button: "Retry".into(),
        then: Box::new(Msg::WorkFile(WorkFileMsg::DeleteConfirmed(op.clone()))),
    };
    // Delete, Unstage and delete and Discard all keep a copy for Undo.
    let hint = st.keymap.paren(BindingId::Undo);
    st.modal = Some(match refusal.problem {
        TrashProblem::NoTrash => {
            let mut body = format!(
                "{path} couldn't be moved to the {TRASH}: {detail}.\n\nDelete it permanently instead? It won't be in the {TRASH}, but Undo{hint} can still bring it back while this tab is open."
            );
            if matches!(op, FileOp::UnstageDelete { .. }) {
                body.push_str(" It also leaves the staging area.");
            }
            let then = Msg::WorkFile(WorkFileMsg::DeleteConfirmed(op.clone().permanent()));
            Modal::danger("Delete permanently?", body, "Delete permanently", then)
        }
        TrashProblem::InUse => retry(
            "File in use".into(),
            format!("{path} is open in another program, so it couldn't be moved to the {TRASH}.\n\nClose it there, then try again."),
        ),
        TrashProblem::AccessDenied => retry(
            format!("Can't move to {TRASH}"),
            format!(
                "Glowfly isn't allowed to move {path} to the {TRASH}: {detail}.\n\nCheck that the file isn't locked and that you may change its folder, then try again."
            ),
        ),
        TrashProblem::Other => retry(
            format!("Can't move to {TRASH}"),
            format!(
                "{path} couldn't be moved to the {TRASH}: {detail}.\n\nTry again. To delete files for good instead, turn off \"Move deleted files to the {TRASH}\" in Preferences."
            ),
        ),
        TrashProblem::HomeTrashOnly => {
            let permanent = format!("Or delete it permanently; Undo{hint} can still bring it back while this tab is open.");
            let body = format!(
                "{path} is on a drive whose trash folder Glowfly can't use: {detail}.\n\nMove it to the Trash in your home folder instead? It is copied there, which can take a while for a large file. {permanent}"
            );
            Modal::TrashOffer(TrashOffer { op: op.clone(), path, body })
        }
    });
}

/// What a delete's toast says about Undo: nothing, that Undo (with its key, when the result is
/// for the active tab) puts the file back, or that it was too large to keep a copy of.
#[derive(Clone, Copy, Debug)]
pub(super) enum UndoNote<'a> {
    None,
    Undo(&'a str),
    TooLarge,
}

/// The toast after `path` was deleted (and, when `unstaged`, taken out of the index), with
/// `note` about Undo.
pub(super) fn deleted_text(path: &str, mode: DeleteMode, deleted: Deleted, unstaged: bool, note: UndoNote) -> String {
    let also = if unstaged { " and removed it from the staging area" } else { "" };
    let mut text = match (deleted, mode) {
        (Deleted::Trashed, DeleteMode::HomeTrash) => format!("Moved {path} to the Trash in your home folder{also}."),
        (Deleted::Trashed, _) => format!("Moved {path} to the {TRASH}{also}."),
        (Deleted::LinkRemoved, _) => {
            format!("Deleted the symbolic link {path}{also}. Links don't go to the {TRASH}; what it points to is untouched.")
        }
        (Deleted::Permanently, DeleteMode::PermanentNoTrash) => {
            format!("Deleted {path} permanently{also}; the {TRASH} couldn't take it.")
        }
        (Deleted::Permanently, _) => format!("Deleted {path}{also}."),
    };
    match (note, deleted) {
        (UndoNote::None, _) | (_, Deleted::LinkRemoved) => {}
        (UndoNote::Undo(hint), _) => text.push_str(&format!(" Undo{hint} puts it back.")),
        (UndoNote::TooLarge, Deleted::Trashed) => {
            text.push_str(&format!(" It's over 64 MB, so Undo can't put it back; restore it from the {TRASH}."));
        }
        (UndoNote::TooLarge, Deleted::Permanently) => text.push_str(" It's over 64 MB, so no copy was kept for Undo."),
    }
    text
}
