//! Deleting several picked files in one run (`FilesEffect::DeletePaths`): in on-screen order,
//! each working copy goes the way the confirmation said, through the single Delete's own core
//! calls (`undo::delete_saving`, or `undo::delete_staged_saving` for a file staged as new, which
//! leaves the index too). The run stops at the first file that fails (the trash refused it, no
//! copy could be kept, or an I/O error); the files before it stay deleted, and one undo entry
//! (`undo::batch`) puts back every file that went, even one whose delete reported an error.

use glowfly_core::error::WriteFailed;
use glowfly_core::undo::{self, BatchOp, DeleteFailed, Trashers};
use glowfly_model::effect::{DeleteEach, DeleteMode};
use glowfly_model::msg::{Msg, WorkFileMsg};
use std::path::Path;

/// Deletes `files` in `workdir` in order, stopping at the first failure.
pub(super) fn run(workdir: &Path, files: Vec<DeleteEach>, mode: DeleteMode, trashers: Trashers) -> Msg {
    let mut parts = Vec::new();
    let (mut done, mut no_undo) = (0, 0);
    let mut failed = None;
    for DeleteEach { path, unstage, promise } in files {
        let promised = mode.promises_copy() && promise;
        let result = if unstage {
            undo::delete_staged_saving(workdir, &path, mode.via(), promised, trashers)
        } else {
            undo::delete_saving(workdir, &path, mode.via(), promised, trashers)
        };
        match result {
            Ok(s) => {
                done += 1;
                match s.undo {
                    Some(a) => parts.push(a),
                    // A Windows folder link, or a file over 64 MB that went without a kept copy.
                    None => no_undo += 1,
                }
            }
            Err(DeleteFailed { error, undo }) => {
                // The file went anyway: Undo still puts it back.
                parts.extend(undo.map(|a| *a));
                failed = Some((path, WriteFailed::from(error)));
                break;
            }
        }
    }
    let undo = undo::batch(BatchOp::Delete, parts);
    Msg::WorkFile(WorkFileMsg::PathsDeleted { mode, done, no_undo, failed, undo })
}
