//! Messages of the file menus: restoring a file from a commit, opening and showing working-tree
//! files, and creating, renaming and deleting them (update/files.rs, update/work_files.rs).

use super::Inactive;
use crate::effect::{FileOp, WorkFileOk};
use git2::Oid;
use trellis_core::repo::RestoredFrom;
use trellis_core::undo::Action;

/// Restoring a file as it was in a commit, and opening or showing a working-tree file
/// (update/files.rs).
pub enum FilesMsg {
    /// Write `path` as of commit `oid` into the working tree (its parent's version when `deleted`,
    /// i.e. the commit deleted it). `confirmed: false` opens the confirmation first.
    /// `overwrite` is set once the user was warned that the file's current contents will be lost;
    /// without it, core refuses to write over contents that discarding couldn't bring back.
    RestoreFile { oid: Oid, path: String, deleted: bool, confirmed: bool, overwrite: bool },
    /// Open a working-tree file (repo-relative path) in the OS default program. One the
    /// system would run instead of show asks first.
    OpenWorkingFile(String),
    /// The user agreed to open (and so run) the working-tree file `path`.
    OpenConfirmed(String),
    /// Opening `path` in `workdir` would run it (`kind`: "a program", "a script", ...), found
    /// when the runner looked at the file; nothing was opened.
    OpenNeedsConfirm { workdir: std::path::PathBuf, path: String, kind: &'static str },
    /// Select a working-tree file in the file manager (its folder where selecting isn't supported).
    ShowWorkingFile(String),
    /// A RestoreFile finished. `undo` is its undo entry (`None`: nothing changed, or the working
    /// copy before or after couldn't be saved); a failed restore has one when it changed the
    /// file anyway.
    FileRestored { path: String, oid: Oid, result: Result<RestoredFrom, String>, undo: Option<Action> },
    /// A RestoreFile without `overwrite` found contents at `path` that only the working tree
    /// holds (`reason` says why), so it wrote nothing.
    RestoreWouldOverwrite { path: String, oid: Oid, reason: String },
}

impl FilesMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::RestoreFile { .. }
            | Self::OpenWorkingFile(..)
            | Self::OpenConfirmed(..)
            | Self::ShowWorkingFile(..)
            | Self::FileRestored { result: Ok(_), undo: None, .. } => None,
            Self::FileRestored { result: Ok(_), undo: Some(a), .. } => Some(Inactive::Undo(a)),
            Self::FileRestored { path, result: Err(e), undo: None, .. } => Some(Inactive::Log(restore_failed(&path, &e, false, None))),
            Self::FileRestored { path, result: Err(e), undo: Some(a), .. } => {
                Some(Inactive::LogAndUndo { text: restore_failed(&path, &e, true, None), undo: a })
            }
            Self::RestoreWouldOverwrite { path, reason, .. } => Some(Inactive::Log(format!("Did not restore {path}: {reason}."))),
            Self::OpenNeedsConfirm { path, kind, .. } => Some(Inactive::Log(not_opened(&path, kind))),
        }
    }
}

/// The log line for a restore of `path` that failed with `e`; `undoable` when it changed the
/// file anyway and recorded an undo entry, which `undo_key` (if Undo has one) runs.
pub fn restore_failed(path: &str, e: &str, undoable: bool, undo_key: Option<&str>) -> String {
    let undo = match (undoable, undo_key) {
        (false, _) => String::new(),
        (true, Some(k)) => format!(". Undo ({k}) puts back what it held before."),
        (true, None) => ". Undo puts back what it held before.".to_owned(),
    };
    format!("Could not restore {path}: {e}{undo}")
}

/// Creating, renaming, deleting and discarding (untracked) working-tree files, and the results
/// (update/work_files.rs, update/work_files_delete.rs).
pub enum WorkFileMsg {
    /// Open the confirmation for discarding the untracked file `path`; it goes to the Recycle Bin / Trash or is deleted the way the preference says.
    DiscardUntracked(String),
    /// Open the New file dialog with this text in its field.
    NewFileDialog(String),
    /// Create an empty file at the typed path (checked and normalized in update).
    CreateFile(String),
    /// Open the rename dialog for the working-tree file `.0`.
    RenameFileDialog(String),
    /// Move the working-tree file `from` to the typed path `to`; the index is untouched, or with
    /// `stage`, through git mv so the index records the rename.
    RenameFile { from: String, to: String, stage: bool },
    /// The rename dialog's "Stage the rename" checkbox (saved in settings).
    SetRenameStages(bool),
    /// Delete the working-tree file `path`: `confirmed: false` opens the confirmation, and
    /// `confirmed: true` deletes straight away the way the preference says.
    DeleteFile { path: String, confirmed: bool },
    /// Run the delete-family op a confirmation agreed to (Delete, UnstageDelete, UnstageGone or Discard),
    /// exactly as the confirmation described it.
    DeleteConfirmed(FileOp),
    /// A WorkFile effect finished.
    WorkFileDone { op: FileOp, result: Result<WorkFileOk, String> },
    /// The trash refused `op`'s file; nothing changed.
    TrashRefused { op: FileOp, refusal: trellis_core::repo::TrashRefusal },
}

impl WorkFileMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::DiscardUntracked(..)
            | Self::NewFileDialog(..)
            | Self::CreateFile(..)
            | Self::RenameFileDialog(..)
            | Self::RenameFile { .. }
            | Self::SetRenameStages(..)
            | Self::DeleteFile { .. }
            | Self::DeleteConfirmed(..) => None,
            Self::WorkFileDone { op, result } => Some(Inactive::WorkFile { op, result }),
            Self::TrashRefused { op, refusal } => Some(Inactive::Log(op.not_deleted(&refusal.detail))),
        }
    }
}

/// The log line for a file that would run, when it can't be asked about.
pub fn not_opened(path: &str, kind: &str) -> String {
    format!("Did not open {path}: it is {kind}, and opening it would run it. Open it again from its repository's tab to choose.")
}
