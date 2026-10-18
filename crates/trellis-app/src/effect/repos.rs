//! What finding repositories asks the runner (runner/repos.rs): listing the project folders.

use std::path::PathBuf;

#[derive(Debug)]
pub enum ReposEffect {
    /// Looks for repositories under `roots` (`trellis_core::repo_scan`); nothing in them runs.
    Scan { generation: u64, roots: Vec<PathBuf> },
}

impl ReposEffect {
    pub fn name(&self) -> &'static str {
        match self {
            ReposEffect::Scan { .. } => "repo-scan",
        }
    }

    pub fn timed(&self) -> bool {
        match self {
            ReposEffect::Scan { .. } => true,
        }
    }
}
