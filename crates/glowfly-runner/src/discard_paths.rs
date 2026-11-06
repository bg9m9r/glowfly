//! Discarding several Unstaged Files rows in one run (`FilesEffect::DiscardPaths`): in
//! on-screen order, a tracked file goes back to its index version through the single discard's
//! own code (`git_ops::local_op`), an untracked one is deleted the way the confirmation said
//! (`undo::discard_untracked_saving`). The run stops at the first file that fails; the files
//! before it stay discarded, and one undo entry (`undo::batch`) puts back every file it changed.

use glowfly_core::error::WriteFailed;
use glowfly_core::undo::{self, BatchOp, DeleteFailed, Trashers};
use glowfly_model::effect::{DeleteMode, DiscardEach, LocalOp};
use glowfly_model::msg::{Msg, WorkFileMsg};
use std::path::Path;

/// Discards `files` in `workdir` in order, stopping at the first failure.
pub(super) fn run(workdir: &Path, files: Vec<DiscardEach>, mode: DeleteMode, trashers: Trashers) -> Msg {
    let mut parts = Vec::new();
    let mut done = 0;
    let mut failed = None;
    for DiscardEach { path, untracked } in files {
        let result = if untracked {
            match undo::discard_untracked_saving(workdir, &path, mode.via(), mode.promises_copy(), trashers) {
                Ok(s) => Ok(s.undo),
                Err(DeleteFailed { error, undo }) => {
                    // The file went anyway: Undo still puts it back.
                    parts.extend(undo.map(|a| *a));
                    Err(error)
                }
            }
        } else {
            super::git_ops::local_op(workdir, LocalOp::DiscardFile(path.clone()))
        };
        match result {
            Ok(undo) => {
                parts.extend(undo);
                done += 1;
            }
            Err(e) => {
                failed = Some((path, WriteFailed::from(e)));
                break;
            }
        }
    }
    Msg::WorkFile(WorkFileMsg::PathsDiscarded { done, failed, undo: undo::batch(BatchOp::Discard, parts) })
}
