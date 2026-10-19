//! Stash list jobs that aren't a plain git command line (`Effect::Stash`).

use git2::Oid;
use std::path::PathBuf;

#[derive(Debug)]
pub enum StashEffect {
    /// Renames the stash list entry stash@{index} (only if it still is commit `oid`) to
    /// `message`, keeping its position. Unscoped like RunGit: it ends the active job.
    Rename { cwd: PathBuf, index: usize, oid: Oid, message: String },
}

impl StashEffect {
    pub fn name(&self) -> &'static str {
        match self {
            StashEffect::Rename { .. } => "rename-stash",
        }
    }

    /// The repository the job changes, for the runner's per-repository queue.
    pub fn cwd(&self) -> &PathBuf {
        match self {
            StashEffect::Rename { cwd, .. } => cwd,
        }
    }
}
