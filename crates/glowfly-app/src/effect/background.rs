//! Work moved off the UI thread (runner/background.rs): laying out a large history's graph, the
//! default branch's ancestry for hover highlighting, freeing a replaced snapshot, and letting go
//! of a closed tab's open repository handles. Reads only what it is handed; never reads or
//! changes the repository.

use super::EffectMeta;
use super::SandboxRule;
use super::Shared;
use git2::Oid;
use glowfly_core::repo::branch_colours::LayoutJob;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum BackgroundEffect {
    /// Lays out the graph of a large history (its WIP row came or went, or another worktree's),
    /// reported as `GraphMsg::LaidOut` with `request`.
    Relayout { workdir: PathBuf, request: u64, job: LayoutJob },
    /// The loaded commits reachable from `tip` (the default branch), for highlighting a hovered
    /// branch's own rows; reported as `GraphMsg::AncestryFound` with `request`.
    Ancestry { workdir: PathBuf, request: u64, commits: Shared, tip: Option<Oid> },
    /// Frees what it holds (a replaced snapshot: about a million allocations at 200k commits).
    /// Reports nothing.
    Free(Discard),
    /// Lets go of the open handles the repository pool keeps for the repository at `.0`
    /// (`repo_pool::forget`): its tab closed. Reports nothing.
    Forget(PathBuf),
}

/// Something to free on a worker; its `Debug` names only what it is.
pub struct Discard(pub &'static str, pub Box<dyn Send>);

impl std::fmt::Debug for Discard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Discard({})", self.0)
    }
}

impl EffectMeta for BackgroundEffect {
    fn name(&self) -> &'static str {
        match self {
            BackgroundEffect::Relayout { .. } => "relayout",
            BackgroundEffect::Ancestry { .. } => "ancestry",
            BackgroundEffect::Free(_) => "free",
            BackgroundEffect::Forget(_) => "forget-repo",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        match self {
            BackgroundEffect::Relayout { workdir, .. } | BackgroundEffect::Ancestry { workdir, .. } => Some(workdir),
            BackgroundEffect::Free(_) | BackgroundEffect::Forget(_) => None,
        }
    }

    fn timed(&self) -> bool {
        true
    }

    fn queue(&self) -> Option<&Path> {
        None
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}
