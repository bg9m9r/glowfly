//! Creating, renaming and deleting a working-tree file (`Effect::WorkFile`).

use crate::effect::FileOp;
use crate::msg::Msg;
use std::path::Path;
use trellis_core::repo;

/// Runs `op` in `workdir`; the result says whether git ignores the new path (false for a delete).
pub(super) fn work_file(workdir: &Path, op: FileOp) -> Msg {
    let r = match &op {
        FileOp::Create(p) => repo::create_file(workdir, p),
        FileOp::Rename { from, to } => repo::rename_file(workdir, from, to),
        FileOp::Delete(p) => repo::delete_file(workdir, p).map(|()| false),
    };
    Msg::WorkFileDone { op, result: r.map_err(|e| e.to_string()) }
}
