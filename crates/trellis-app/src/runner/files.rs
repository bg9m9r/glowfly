//! Restoring a file from a commit with an undo entry (`Effect::RestoreFile`), creating,
//! renaming, deleting and discarding (untracked) a working-tree file (`Effect::WorkFile`),
//! listing the tracked files (`Effect::LoadTrackedPaths`), and checking the Clone dialog's
//! parent folder (`Effect::CheckCloneParent`).

use crate::effect::{DeleteMode, FileOp, WorkFileOk};
use crate::msg::CloneMsg;
use crate::msg::FilesMsg;
use crate::msg::Msg;
use crate::msg::PaletteMsg;
use crate::msg::WorkFileMsg;
use git2::Oid;
use std::path::Path;
use trellis_core::error::Error;
use trellis_core::repo;
use trellis_core::undo;

/// Restores `path` from commit `oid`, saving the working copy first so the restore can be undone.
/// Contents only the working tree holds, without `overwrite`, come back as a second confirmation.
pub(super) fn restore(workdir: &Path, oid: Oid, path: String, overwrite: bool) -> Msg {
    match undo::restore_saving(workdir, oid, &path, overwrite) {
        Err(Error::WouldOverwrite(reason)) => Msg::Files(FilesMsg::RestoreWouldOverwrite { path, oid, reason }),
        result => Msg::Files(FilesMsg::FileRestored { result: result.map_err(|e| e.to_string()), path, oid }),
    }
}

/// Moves a deleted file to the system trash. The app's test build uses a stand-in folder
/// instead, so tests never touch the real Recycle Bin / Trash.
#[cfg(not(test))]
const TRASHER: repo::Trasher = repo::os_trash;
#[cfg(test)]
const TRASHER: repo::Trasher = test_trash;

/// The stand-in trash of the app's test build: a folder in the temp dir, one per process.
#[cfg(test)]
pub(crate) fn test_bin() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("trellis-app-test-bin-{}", std::process::id()))
}

/// Moves `full` into `test_bin()` under a unique name (the temp dir and scratch repositories
/// share a volume, so a rename works).
#[cfg(test)]
fn test_trash(full: &Path) -> Result<(), String> {
    let bin = test_bin();
    std::fs::create_dir_all(&bin).map_err(|e| e.to_string())?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let name = full.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    std::fs::rename(full, bin.join(format!("{nanos}-{name}"))).map_err(|e| e.to_string())
}

fn removal(mode: DeleteMode) -> repo::Removal {
    match mode {
        DeleteMode::Trash => repo::Removal::Trash(TRASHER),
        DeleteMode::Permanent | DeleteMode::PermanentNoTrash => repo::Removal::Permanent,
    }
}

/// Runs `op` in `workdir`. A create or plain rename reports whether git ignores the new path (a
/// staged rename's new path stays tracked); a delete reports how the working copy went. A staged
/// rename goes through git mv; UnstageDelete also takes the file out of the index, and
/// UnstageGone only does that. Discard deletes an untracked file, refusing one that is tracked by
/// then. When the trash refuses a file, nothing has changed and the result
/// is `WorkFileMsg::TrashRefused`, so the user can be asked about deleting it permanently.
pub(super) fn work_file(workdir: &Path, op: FileOp) -> Msg {
    let r = match &op {
        FileOp::Create(p) => repo::create_file(workdir, p).map(|ignored| WorkFileOk::Written { ignored }),
        FileOp::Rename { from, to, stage: true } => {
            repo::rename_file_staged(workdir, from, to).map(|()| WorkFileOk::Written { ignored: false })
        }
        FileOp::Rename { from, to, stage: false } => repo::rename_file(workdir, from, to).map(|ignored| WorkFileOk::Written { ignored }),
        FileOp::Delete { path, mode } => repo::delete_file(workdir, path, removal(*mode)).map(WorkFileOk::Deleted),
        FileOp::UnstageDelete { path, mode } => repo::delete_staged_new_file(workdir, path, removal(*mode)).map(WorkFileOk::Deleted),
        FileOp::UnstageGone(p) => repo::unstage_gone_new_file(workdir, p).map(|()| WorkFileOk::Unstaged),
        FileOp::Discard { path, mode } => repo::discard_untracked(workdir, path, removal(*mode)).map(WorkFileOk::Deleted),
    };
    match r {
        Err(Error::NoTrash(reason)) => Msg::WorkFile(WorkFileMsg::TrashRefused { op, reason }),
        r => Msg::WorkFile(WorkFileMsg::WorkFileDone { op, result: r.map_err(|e| e.to_string()) }),
    }
}

/// The paths in the index, for the palette's file finder.
pub(super) fn tracked_paths(workdir: &Path, generation: u64) -> Msg {
    let result = repo::tracked_paths(workdir).map_err(|e| format!("Could not list the repository's files: {e}"));
    Msg::Palette(PaletteMsg::TrackedPathsLoaded { generation, result })
}

/// Whether `parent` is a folder, looked at after `delay` (`update` drops answers for text the
/// user has changed since).
pub(super) fn check_clone_parent(parent: String, delay: std::time::Duration) -> Msg {
    if !delay.is_zero() {
        std::thread::sleep(delay);
    }
    let is_dir = Path::new(&parent).is_dir();
    Msg::Clone(CloneMsg::ParentChecked { parent, is_dir })
}
