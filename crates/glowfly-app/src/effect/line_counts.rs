//! Per-file line counts of a commit or of the working tree (runner/line_counts.rs). Reads only
//! (libgit2, no program runs), scoped to the repository's tab.

use super::EffectMeta;
#[cfg(test)]
use super::SandboxRule;
use git2::Oid;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum LineCountsEffect {
    /// Commit `oid`'s files against its first parent; reported as `RepoMsg::CommitLineCountsLoaded`.
    Commit { workdir: PathBuf, oid: Oid },
    /// The working tree's staged and unstaged files; reported as `RepoMsg::WipLineCountsLoaded`
    /// with `generation`.
    Wip { workdir: PathBuf, generation: u64 },
}

/// The request a failed (panicked) count answers, so it doesn't stay pending.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FailedLineCounts {
    Commit(Oid),
    Wip(u64),
}

impl LineCountsEffect {
    pub fn failure(&self) -> FailedLineCounts {
        match self {
            LineCountsEffect::Commit { oid, .. } => FailedLineCounts::Commit(*oid),
            LineCountsEffect::Wip { generation, .. } => FailedLineCounts::Wip(*generation),
        }
    }
}

impl EffectMeta for LineCountsEffect {
    fn name(&self) -> &'static str {
        match self {
            LineCountsEffect::Commit { .. } => "line-counts-commit",
            LineCountsEffect::Wip { .. } => "line-counts-wip",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        match self {
            LineCountsEffect::Commit { workdir, .. } | LineCountsEffect::Wip { workdir, .. } => Some(workdir),
        }
    }

    fn timed(&self) -> bool {
        true
    }

    fn queue(&self) -> Option<&Path> {
        None
    }

    #[cfg(test)]
    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}
