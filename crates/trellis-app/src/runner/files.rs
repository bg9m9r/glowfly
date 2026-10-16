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
use trellis_core::undo::RestoreFailed;

/// Restores `path` from commit `oid`, saving the working copy first so the restore can be undone
/// (even one that failed after changing the file). Contents only the working tree holds, without
/// `overwrite`, come back as a second confirmation.
pub(super) fn restore(workdir: &Path, oid: Oid, path: String, overwrite: bool) -> Msg {
    let (result, undo) = match undo::restore_saving(workdir, oid, &path, overwrite) {
        Err(RestoreFailed { error: Error::WouldOverwrite(reason), undo: None }) => {
            return Msg::Files(FilesMsg::RestoreWouldOverwrite { path, oid, reason });
        }
        Ok((from, undo)) => (Ok(from), undo),
        Err(RestoreFailed { error, undo }) => (Err(error.to_string()), undo.map(|a| *a)),
    };
    Msg::Files(FilesMsg::FileRestored { path, oid, result, undo })
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
    trellis_core::paths::temp_dir().join(format!("trellis-app-test-bin-{}", std::process::id()))
}

/// Moves `full` into `test_bin()` under a unique name (the temp dir and scratch repositories
/// share a volume, so a rename works).
#[cfg(test)]
fn test_trash(full: &Path) -> Result<(), repo::TrashRefusal> {
    let other = |e: std::io::Error| repo::TrashRefusal::new(repo::TrashProblem::Other, e.to_string());
    let bin = test_bin();
    std::fs::create_dir_all(&bin).map_err(other)?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let name = full.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    std::fs::rename(full, bin.join(format!("{nanos}-{name}"))).map_err(other)
}

fn removal(mode: DeleteMode) -> repo::Removal {
    match mode {
        DeleteMode::Trash => repo::Removal::Trash(TRASHER),
        DeleteMode::Permanent | DeleteMode::PermanentNoTrash => repo::Removal::Permanent,
    }
}

/// Runs `op` in `workdir`. A create or plain rename reports whether git ignores the new path (a
/// staged rename's new path stays tracked) and any warnings about the new name; a delete reports how the working copy went. A staged
/// rename goes through git mv; UnstageDelete also takes the file out of the index, and
/// UnstageGone only does that. Discard deletes an untracked file, refusing one that is tracked by
/// then. When the trash refuses a file, nothing has changed and the result
/// is `WorkFileMsg::TrashRefused`, so the user can retry or be asked about deleting it permanently.
pub(super) fn work_file(workdir: &Path, op: FileOp) -> Msg {
    let r = match &op {
        FileOp::Create(p) => repo::create_file(workdir, p).map(WorkFileOk::written),
        FileOp::Rename { from, to, stage: true } => repo::rename_file_staged(workdir, from, to).map(WorkFileOk::written),
        FileOp::Rename { from, to, stage: false } => repo::rename_file(workdir, from, to).map(WorkFileOk::written),
        FileOp::Delete { path, mode } => repo::delete_file(workdir, path, removal(*mode)).map(WorkFileOk::Deleted),
        FileOp::UnstageDelete { path, mode } => repo::delete_staged_new_file(workdir, path, removal(*mode)).map(WorkFileOk::Deleted),
        FileOp::UnstageGone(p) => repo::unstage_gone_new_file(workdir, p).map(|()| WorkFileOk::Unstaged),
        FileOp::Discard { path, mode } => repo::discard_untracked(workdir, path, removal(*mode)).map(WorkFileOk::Deleted),
    };
    match r {
        Err(Error::TrashRefused(refusal)) => Msg::WorkFile(WorkFileMsg::TrashRefused { op, refusal }),
        r => Msg::WorkFile(WorkFileMsg::WorkFileDone { op, result: r.map_err(|e| e.to_string()) }),
    }
}

/// The paths in the index, for the palette's file finder.
pub(super) fn tracked_paths(workdir: &Path, generation: u64) -> Msg {
    let result = repo::tracked_paths(workdir).map_err(|e| format!("Could not list the repository's files: {e}"));
    Msg::Palette(PaletteMsg::TrackedPathsLoaded { generation, result })
}

/// The folder the typed `parent` names (`~` expanded, resolved), when it is one, looked at after
/// `delay` (`update` drops answers for text the user has changed since). Typed text that isn't a
/// full path names nothing: it is never resolved against the process's working folder.
pub(super) fn check_clone_parent(parent: String, delay: std::time::Duration) -> Msg {
    if !delay.is_zero() {
        std::thread::sleep(delay);
    }
    let found = clone_parent(&parent, trellis_core::settings::home_dir().as_deref());
    Msg::Clone(CloneMsg::ParentChecked { parent, found })
}

fn clone_parent(text: &str, home: Option<&Path>) -> Option<std::path::PathBuf> {
    let path = trellis_core::paths::expand_home(text, home);
    (path.is_absolute() && path.is_dir()).then(|| trellis_core::paths::resolve(&path))
}

#[cfg(test)]
mod tests {
    use super::clone_parent;

    #[test]
    fn the_clone_parent_is_expanded_resolved_and_never_relative() {
        let home = trellis_core::paths::temp_dir();
        assert_eq!(clone_parent(" ~ ", Some(&home)), Some(home.clone()));
        assert_eq!(clone_parent(&home.display().to_string(), None), Some(home.clone()));
        assert_eq!(clone_parent("~/no-such-folder-for-trellis", Some(&home)), None);
        // A relative path would be the process's working folder (System32 for a GUI start).
        assert_eq!(clone_parent(".", Some(&home)), None);
        assert_eq!(clone_parent("~", None), None);
    }
}
