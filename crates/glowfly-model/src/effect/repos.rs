//! What repositories across folders ask the runner (runner/repos.rs): listing the project
//! folders, and fetching or pulling one listed repository.

use super::EffectMeta;
use super::SandboxRule;
use glowfly_core::repo_sync::SyncOp;
use glowfly_core::settings::SavedTrust;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ReposEffect {
    /// Looks for repositories under `roots` (`glowfly_core::repo_scan`); nothing in them runs.
    Scan { generation: u64, roots: Vec<PathBuf> },
    /// Fetches or pulls `path` without prompting (`glowfly_core::repo_sync`), row `index` of run
    /// `generation`. It is skipped when its config or hooks run programs its saved decision
    /// `trust` doesn't allow (`glowfly_core::trust::allows`).
    Sync { generation: u64, index: usize, path: PathBuf, op: SyncOp, trust: Option<SavedTrust> },
}

impl EffectMeta for ReposEffect {
    fn name(&self) -> &'static str {
        match self {
            ReposEffect::Scan { .. } => "repo-scan",
            ReposEffect::Sync { .. } => "repo-sync",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    fn timed(&self) -> bool {
        match self {
            ReposEffect::Scan { .. } => true,
            // Waits on the network.
            ReposEffect::Sync { .. } => false,
        }
    }

    /// A pull changes the repository; a scan reads folder listings and a fetch only adds objects.
    fn queue(&self) -> Option<&Path> {
        match self {
            ReposEffect::Sync { path, op: SyncOp::Pull, .. } => Some(path),
            ReposEffect::Scan { .. } | ReposEffect::Sync { op: SyncOp::Fetch { .. }, .. } => None,
        }
    }

    /// A fetch too, after which git may repack.
    fn holds(&self) -> Option<&Path> {
        match self {
            ReposEffect::Sync { path, .. } => Some(path),
            ReposEffect::Scan { .. } => None,
        }
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Drop
    }
}
