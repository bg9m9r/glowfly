//! Stash list jobs that aren't a plain git command line, the saved auto-stash records, the read
//! of a stash's files and the abort that keeps an operation's work for Undo (`Effect::Stash`).

use super::EffectMeta;
use super::SandboxRule;
use crate::state::stash_confirm::StashAsk;
use git2::Oid;
use glowfly_core::stash_ops::SavedAutoStash;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum StashEffect {
    /// Renames the stash list entry stash@{index} (only if it still is commit `oid`) to
    /// `message`, keeping its position. Unscoped like RunGit: it ends the active job.
    Rename { cwd: PathBuf, index: usize, oid: Oid, message: String },
    /// Puts an auto-stash back once nothing is in progress. Unscoped like RunGit: it ends the
    /// active job.
    PutBack(PutBackRequest),
    /// Reads the auto-stashes a restart left waiting in `cwd` (`stash_ops::read_saved`);
    /// reports `AutoStashMsg::SavedRead` only when there are any.
    ReadAutoStashes { cwd: PathBuf },
    /// Replaces `cwd`'s saved auto-stash records with `saved` (none removes the file); a
    /// failure is logged.
    SaveAutoStashes { cwd: PathBuf, saved: Vec<SavedAutoStash> },
    /// Reads the files stash `ask.oid` changes (`stash_ops::changed_paths`) for the question
    /// before applying it; reports `StashMsg::PathsRead`, scoped to the tab that asked.
    ReadPaths { cwd: PathBuf, ask: StashAsk },
    /// Aborts the merge, single cherry-pick or single revert in progress in `cwd`, keeping its
    /// work first (`undo::abort_keeping`) and recording the abort for Undo; nothing is aborted
    /// when it can't all be kept. Old kept aborts' refs are dropped, except those of the commits
    /// in `keep` (named by undo entries). Unscoped like RunGit: it ends the active job.
    AbortKeeping { cwd: PathBuf, keep: Vec<Oid> },
}

/// The auto-stash to put back: the stash with commit `oid` in `cwd`'s list.
#[derive(Debug, Clone, PartialEq)]
pub struct PutBackRequest {
    pub cwd: PathBuf,
    pub oid: Oid,
}

impl StashEffect {
    /// The repository the job changes, for the runner's per-repository queue.
    pub fn cwd(&self) -> &PathBuf {
        match self {
            StashEffect::Rename { cwd, .. }
            | StashEffect::PutBack(PutBackRequest { cwd, .. })
            | StashEffect::ReadAutoStashes { cwd }
            | StashEffect::SaveAutoStashes { cwd, .. }
            | StashEffect::ReadPaths { cwd, .. }
            | StashEffect::AbortKeeping { cwd, .. } => cwd,
        }
    }
}

impl EffectMeta for StashEffect {
    fn name(&self) -> &'static str {
        match self {
            StashEffect::Rename { .. } => "rename-stash",
            StashEffect::PutBack(_) => "put-back-stash",
            StashEffect::ReadAutoStashes { .. } => "read-auto-stashes",
            StashEffect::SaveAutoStashes { .. } => "save-auto-stashes",
            StashEffect::ReadPaths { .. } => "read-stash-paths",
            StashEffect::AbortKeeping { .. } => "abort-keeping",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        match self {
            StashEffect::ReadPaths { cwd, .. } => Some(cwd),
            _ => None,
        }
    }

    fn timed(&self) -> bool {
        true
    }

    /// The read of a stash's files changes nothing, so it doesn't wait behind a running job.
    fn queue(&self) -> Option<&Path> {
        match self {
            StashEffect::ReadPaths { .. } => None,
            _ => Some(self.cwd()),
        }
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}
