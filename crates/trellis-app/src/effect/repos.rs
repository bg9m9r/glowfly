//! What repositories across folders ask the runner (runner/repos.rs): listing the project
//! folders, and fetching or pulling one listed repository.

use std::path::PathBuf;
use trellis_core::repo_sync::SyncOp;

#[derive(Debug)]
pub enum ReposEffect {
    /// Looks for repositories under `roots` (`trellis_core::repo_scan`); nothing in them runs.
    Scan { generation: u64, roots: Vec<PathBuf> },
    /// Fetches or pulls `path` without prompting (`trellis_core::repo_sync`), row `index` of run
    /// `generation`. Unless `trusted`, it is skipped when its config or hooks run programs.
    Sync { generation: u64, index: usize, path: PathBuf, op: SyncOp, trusted: bool },
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
