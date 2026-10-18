//! What repositories across folders ask the runner (runner/repos.rs): listing the project
//! folders, and fetching or pulling one listed repository.

use std::path::PathBuf;
use trellis_core::repo_sync::SyncOp;
use trellis_core::settings::SavedTrust;

#[derive(Debug)]
pub enum ReposEffect {
    /// Looks for repositories under `roots` (`trellis_core::repo_scan`); nothing in them runs.
    Scan { generation: u64, roots: Vec<PathBuf> },
    /// Fetches or pulls `path` without prompting (`trellis_core::repo_sync`), row `index` of run
    /// `generation`. It is skipped when its config or hooks run programs its saved decision
    /// `trust` doesn't allow (`trellis_core::trust::allows`).
    Sync { generation: u64, index: usize, path: PathBuf, op: SyncOp, trust: Option<SavedTrust> },
}

impl ReposEffect {
    pub fn name(&self) -> &'static str {
        match self {
            ReposEffect::Scan { .. } => "repo-scan",
            ReposEffect::Sync { .. } => "repo-sync",
        }
    }

    pub fn timed(&self) -> bool {
        match self {
            ReposEffect::Scan { .. } => true,
            // Waits on the network.
            ReposEffect::Sync { .. } => false,
        }
    }
}
