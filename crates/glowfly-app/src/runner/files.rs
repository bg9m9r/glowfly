//! Restoring a file from a commit with an undo entry (`Effect::RestoreFile`), creating,
//! renaming, deleting and discarding (untracked) a working-tree file (`Effect::WorkFile`),
//! listing the tracked files (`Effect::LoadTrackedPaths`), and checking the Clone dialog's
//! parent folder (`CloneEffect::CheckParent`).

use crate::effect::{FileOp, LocalOp, SeenLines, WorkFileOk};
use crate::msg::CloneMsg;
use crate::msg::FilesMsg;
use crate::msg::Msg;
use crate::msg::PaletteMsg;
use crate::msg::WorkFileMsg;
use git2::Oid;
use glowfly_core::error::Error;
use glowfly_core::repo;
use glowfly_core::undo;
use glowfly_core::undo::RestoreFailed;
use std::path::Path;

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

/// Runs `op` in `workdir`. A create or plain rename reports whether git ignores the new path (a
/// staged rename's new path stays tracked) and any warnings about the new name; a delete reports how the working copy went. A staged
/// rename goes through git mv; UnstageDelete also takes the file out of the index, and
/// UnstageGone only does that. Discard deletes an untracked file, refusing one that is tracked by
/// then; one sent by discarding every line of its diff first checks that git still shows that
/// diff, and discards the lines of an intent-to-add file instead (`StageMsg::LocalOpDone`).
/// When the trash refuses a file, nothing has changed and the result is
/// `WorkFileMsg::TrashRefused`, so the user can retry or be asked about deleting it permanently.
/// Delete, UnstageDelete and Discard keep the file's bytes first, and UnstageGone its staged
/// entry, so all of them can be undone; one that fails after the file went still reports its
/// undo entry (`WorkFileMsg::WorkFileFailed`). A deleted file goes to `trashers` (the
/// runner's `Seams`: the system trash, or the home Trash when its drive's can't be used), and
/// Undo takes it back out of them.
pub(super) fn work_file(workdir: &Path, op: FileOp, trashers: undo::Trashers) -> Msg {
    let deleting = |r: Result<undo::DeletedSaving, undo::DeleteFailed>| match r {
        Ok(s) => Ok(WorkFileOk::from_saving(s)),
        Err(undo::DeleteFailed { error, undo: Some(undo) }) => Err((error, Some(undo))),
        Err(undo::DeleteFailed { error, undo: None }) => Err((error, None)),
    };
    let plain = |r: glowfly_core::error::Result<WorkFileOk>| r.map_err(|e| (e, None));
    let r = match &op {
        FileOp::Create(p) => plain(repo::create_file(workdir, p).map(WorkFileOk::written)),
        FileOp::Rename { from, to, stage: true } => plain(repo::rename_file_staged(workdir, from, to).map(WorkFileOk::written)),
        FileOp::Rename { from, to, stage: false } => plain(repo::rename_file(workdir, from, to).map(WorkFileOk::written)),
        FileOp::Delete { path, mode } => deleting(undo::delete_saving(workdir, path, mode.via(), mode.promises_copy(), trashers)),
        FileOp::UnstageDelete { path, mode } => {
            deleting(undo::delete_staged_saving(workdir, path, mode.via(), mode.promises_copy(), trashers))
        }
        FileOp::UnstageGone(p) => plain(undo::unstage_gone_saving(workdir, p).map(|a| WorkFileOk::Unstaged { undo: Some(Box::new(a)) })),
        FileOp::Discard { path, mode, seen } => {
            let discard = || deleting(undo::discard_untracked_saving(workdir, path, mode.via(), mode.promises_copy(), trashers));
            match seen.as_deref() {
                None => discard(),
                // Discarding every line: only while git still shows the diff the user saw.
                Some(s) => match repo::whole_discard(workdir, path, &s.expected, s.raw, s.ignore_whitespace) {
                    Ok(repo::WholeDiscard::Delete) => discard(),
                    // Git has an intent-to-add file: its lines are discarded like any others.
                    Ok(repo::WholeDiscard::Lines) => return lines_instead(workdir, path, s),
                    Err(e) => Err((e, None)),
                },
            }
        }
    };
    match r {
        Err((Error::TrashRefused(refusal), None)) => {
            // The file is still there: whether a permanent delete could be undone.
            let too_large = !op.delete_path().is_some_and(|p| undo::keeps_copy(workdir, p));
            Msg::WorkFile(WorkFileMsg::TrashRefused { op, refusal, too_large })
        }
        Err((Error::NoCopyKept(reason), None)) => Msg::WorkFile(WorkFileMsg::NoCopyKept { op, reason }),
        Err((e, Some(undo))) => Msg::WorkFile(WorkFileMsg::WorkFileFailed { op, error: e.to_string(), undo }),
        r => Msg::WorkFile(WorkFileMsg::WorkFileDone { op, result: r.map_err(|(e, _)| e.to_string()) }),
    }
}

/// Discards the `seen` lines of `path` the way a partial line discard does, undo entry and all.
fn lines_instead(workdir: &Path, path: &str, seen: &SeenLines) -> Msg {
    let s = seen.clone();
    let op = LocalOp::Lines {
        op: repo::HunkOp::Discard,
        path: path.to_owned(),
        hunk: 0,
        expected: s.expected,
        lines: s.lines,
        ignore_whitespace: s.ignore_whitespace,
    };
    super::git_ops::local_done(workdir, op)
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
    let found = clone_parent(&parent, glowfly_core::settings::home_dir().as_deref());
    Msg::Clone(CloneMsg::ParentChecked { parent, found })
}

fn clone_parent(text: &str, home: Option<&Path>) -> Option<std::path::PathBuf> {
    let path = glowfly_core::paths::expand_home(text, home);
    (path.is_absolute() && path.is_dir()).then(|| glowfly_core::paths::resolve(&path))
}

#[cfg(test)]
mod tests {
    use super::clone_parent;

    #[test]
    fn the_clone_parent_is_expanded_resolved_and_never_relative() {
        let home = glowfly_core::paths::temp_dir();
        assert_eq!(clone_parent(" ~ ", Some(&home)), Some(home.clone()));
        assert_eq!(clone_parent(&home.display().to_string(), None), Some(home.clone()));
        assert_eq!(clone_parent("~/no-such-folder-for-glowfly", Some(&home)), None);
        // A relative path would be the process's working folder (System32 for a GUI start).
        assert_eq!(clone_parent(".", Some(&home)), None);
        assert_eq!(clone_parent("~", None), None);
    }
}
