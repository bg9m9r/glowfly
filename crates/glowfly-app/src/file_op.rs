//! Working-tree file operations (`Effect::WorkFile`) and what they report back.

use glowfly_core::repo::Deleted;
use glowfly_core::settings::Settings;
use glowfly_core::undo::{self, Action, DeletedSaving};

/// How Delete…, Unstage and delete… and Discard of an untracked file remove the working copy.
/// Chosen when the confirmation opens and carried in it, so changing the preference meanwhile
/// doesn't change what runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteMode {
    /// To the Recycle Bin / Trash.
    Trash,
    /// Removed for good (the preference is off).
    Permanent,
    /// Permanent after the trash refused the file and the user agreed.
    PermanentNoTrash,
    /// To the home Trash, copied from another drive, after the drive's own trash couldn't be
    /// used and the user agreed (Linux).
    HomeTrash,
}

impl DeleteMode {
    pub fn for_settings(s: &Settings) -> Self {
        if s.delete_to_trash { DeleteMode::Trash } else { DeleteMode::Permanent }
    }

    /// How the delete goes, as its undo entry records it for redo.
    pub fn via(self) -> undo::DeleteVia {
        match self {
            DeleteMode::Trash => undo::DeleteVia::Trash,
            DeleteMode::HomeTrash => undo::DeleteVia::HomeTrash,
            DeleteMode::Permanent | DeleteMode::PermanentNoTrash => undo::DeleteVia::Permanent,
        }
    }
}

/// A working-tree file operation (`Effect::WorkFile`). Paths are repo-relative.
#[derive(Clone, Debug, PartialEq)]
pub enum FileOp {
    Create(String),
    Rename {
        from: String,
        to: String,
        stage: bool,
    },
    Delete {
        path: String,
        mode: DeleteMode,
    },
    /// Deletes a file staged as new, or the new path of a staged rename, and takes it out of the
    /// index (a rename's source is unstaged too).
    UnstageDelete {
        path: String,
        mode: DeleteMode,
    },
    /// Takes a staged new file or staged rename's new path that is already gone from the working
    /// tree out of the index; the working tree is not touched.
    UnstageGone(String),
    /// Discards the untracked file `path`: removed the way `mode` says. Refused if it is tracked by then.
    Discard {
        path: String,
        mode: DeleteMode,
    },
}

impl FileOp {
    /// The path of a Delete, UnstageDelete or Discard.
    /// The log line for a refused delete that arrives for a tab that is no longer active.
    pub fn not_deleted(&self, reason: &str) -> String {
        format!("Did not delete {}: {}.", self.delete_path().unwrap_or_default(), reason.trim_end_matches('.'))
    }

    pub fn delete_path(&self) -> Option<&str> {
        match self {
            FileOp::Delete { path, .. } | FileOp::UnstageDelete { path, .. } | FileOp::Discard { path, .. } => Some(path),
            _ => None,
        }
    }

    /// The same delete, removing the file for good because the trash couldn't take it; other
    /// operations are returned unchanged.
    pub fn permanent(self) -> Self {
        match self {
            FileOp::Delete { path, .. } => FileOp::Delete { path, mode: DeleteMode::PermanentNoTrash },
            FileOp::UnstageDelete { path, .. } => FileOp::UnstageDelete { path, mode: DeleteMode::PermanentNoTrash },
            FileOp::Discard { path, .. } => FileOp::Discard { path, mode: DeleteMode::PermanentNoTrash },
            op => op,
        }
    }

    /// The same delete, moving the file into the home Trash because its drive's trash couldn't
    /// be used; other operations are returned unchanged.
    pub fn home_trash(self) -> Self {
        match self {
            FileOp::Delete { path, .. } => FileOp::Delete { path, mode: DeleteMode::HomeTrash },
            FileOp::UnstageDelete { path, .. } => FileOp::UnstageDelete { path, mode: DeleteMode::HomeTrash },
            FileOp::Discard { path, .. } => FileOp::Discard { path, mode: DeleteMode::HomeTrash },
            op => op,
        }
    }
}

/// What a successful `FileOp` did.
#[derive(Clone, Debug, PartialEq)]
pub enum WorkFileOk {
    /// Created or renamed; whether git ignores the new path, and warnings about the new name
    /// (it won't check out on Windows, or it is one name with a tracked path there and on macOS).
    Written { ignored: bool, warnings: Vec<String> },
    /// Deleted or discarded (UnstageDelete also unstaged): how the working copy went, the action
    /// that undoes it (when a copy of the file was kept), and whether it was too large to keep a
    /// copy of.
    Deleted { how: Deleted, undo: Option<Box<Action>>, too_large: bool },
    /// UnstageGone: only the index changed; `undo` stages it again.
    Unstaged { undo: Option<Box<Action>> },
}

impl WorkFileOk {
    pub fn written(w: glowfly_core::repo::Written) -> Self {
        WorkFileOk::Written { ignored: w.ignored, warnings: w.warnings }
    }

    /// Deleted without an undo entry.
    pub fn deleted(how: Deleted) -> Self {
        WorkFileOk::Deleted { how, undo: None, too_large: false }
    }

    pub fn from_saving(s: DeletedSaving) -> Self {
        WorkFileOk::Deleted { how: s.how, undo: s.undo.map(Box::new), too_large: s.too_large }
    }

    /// The action that undoes this result, if it has one.
    pub fn undo(&self) -> Option<&Action> {
        match self {
            WorkFileOk::Deleted { undo, .. } | WorkFileOk::Unstaged { undo } => undo.as_deref(),
            WorkFileOk::Written { .. } => None,
        }
    }
}
