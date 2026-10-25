//! The conflict effects (`Effect::Conflict`): loading a conflicted file, and reading the
//! conflicted files' working copies for markers, and the resolved files.

use super::execute::{Cx, Execute, err};
use crate::effect::{ConflictEffect, ConflictRead};
use crate::msg::{ConflictMsg, Msg};
use glowfly_core::repo;
use std::path::Path;

impl Execute for ConflictEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(match self {
            ConflictEffect::Load { workdir, path, generation } => {
                let result = repo::load_conflict(&workdir, &path).map_err(err);
                // The picks a file saved with markers holds come back with it.
                let saved = result.as_ref().ok().and_then(|f| repo::saved_picks(&workdir, f));
                Msg::Conflict(ConflictMsg::ConflictLoaded { path: path.clone(), generation, result, saved })
            }
            ConflictEffect::ReadFiles { workdir, paths, purpose } => read(&workdir, paths, purpose),
        })
    }
}

/// Counts the markers in `paths` and, for a summary, lists the resolved files.
pub(super) fn read(workdir: &Path, paths: Vec<String>, purpose: ConflictRead) -> Msg {
    let with_resolved = matches!(purpose, ConflictRead::Summary { .. });
    let result = repo::conflict_summary(workdir, &paths, with_resolved).map_err(|e| format!("Could not read the conflicted files: {e}"));
    Msg::Conflict(ConflictMsg::ConflictFilesRead { purpose, paths, result })
}
