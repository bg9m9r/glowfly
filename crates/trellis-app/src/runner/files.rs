//! Creating, renaming and deleting a working-tree file (`Effect::WorkFile`), and listing the
//! tracked files (`Effect::LoadTrackedPaths`).

use crate::effect::FileOp;
use crate::msg::Msg;
use std::path::Path;
use trellis_core::repo;

/// Runs `op` in `workdir`; the result says whether git ignores the new path (false for a delete,
/// and for a staged rename, whose new path stays tracked). A staged rename goes through git mv.
pub(super) fn work_file(workdir: &Path, op: FileOp) -> Msg {
    let r = match &op {
        FileOp::Create(p) => repo::create_file(workdir, p),
        FileOp::Rename { from, to, stage: true } => repo::rename_file_staged(workdir, from, to).map(|()| false),
        FileOp::Rename { from, to, stage: false } => repo::rename_file(workdir, from, to),
        FileOp::Delete(p) => repo::delete_file(workdir, p).map(|()| false),
    };
    Msg::WorkFileDone { op, result: r.map_err(|e| e.to_string()) }
}

/// The paths in the index, for the palette's file finder.
pub(super) fn tracked_paths(workdir: &Path, generation: u64) -> Msg {
    let result = repo::tracked_paths(workdir).map_err(|e| format!("Could not list the repository's files: {e}"));
    Msg::TrackedPathsLoaded { generation, result }
}
