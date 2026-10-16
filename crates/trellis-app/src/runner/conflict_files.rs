//! Reading the conflicted files' working copies for markers, and the resolved files.

use crate::effect::ConflictRead;
use crate::msg::{ConflictMsg, Msg};
use std::path::Path;
use trellis_core::repo;

/// Counts the markers in `paths` and, for a summary, lists the resolved files.
pub(super) fn read(workdir: &Path, paths: Vec<String>, purpose: ConflictRead) -> Msg {
    let with_resolved = matches!(purpose, ConflictRead::Summary { .. });
    let result = repo::conflict_summary(workdir, &paths, with_resolved).map_err(|e| format!("Could not read the conflicted files: {e}"));
    Msg::Conflict(ConflictMsg::ConflictFilesRead { purpose, paths, result })
}
