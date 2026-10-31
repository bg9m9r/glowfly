//! The worktree effects (`Effect::Worktree`); their requests are in `crate::worktree`
//! (runner/worktrees.rs).

use super::SandboxRule;
use super::meta::EffectMeta;
use crate::worktree::{CountRequest, MoveRequest, NewWorktree, RemoveRequest};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum WorktreeEffect {
    /// Checks the folder of a worktree about to be created (missing, or an empty folder) before
    /// `git worktree add` runs. Unscoped: the dialog waits for the answer even if tabs change.
    CheckFolder(NewWorktree),
    /// `git worktree remove` then, if asked, `git branch -d`; unscoped like RunGit (ends the job).
    Remove(RemoveRequest),
    /// Other worktrees' changes the load left Pending, or counted ones read again after a reload
    /// that kept the snapshot (`WorktreeMsg::ChangesCounted`).
    CountChanges(CountRequest),
    /// `git worktree move` once the target is free; unscoped like RunGit (ends the job).
    Move(MoveRequest),
}

impl EffectMeta for WorktreeEffect {
    fn name(&self) -> &'static str {
        match self {
            WorktreeEffect::CheckFolder(_) => "check-worktree-folder",
            WorktreeEffect::Remove(_) => "remove-worktree",
            WorktreeEffect::CountChanges(_) => "count-worktree-changes",
            WorktreeEffect::Move(_) => "move-worktree",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        match self {
            WorktreeEffect::CountChanges(r) => Some(&r.workdir),
            WorktreeEffect::CheckFolder(_) | WorktreeEffect::Remove(_) | WorktreeEffect::Move(_) => None,
        }
    }

    fn timed(&self) -> bool {
        true
    }

    /// Removing and moving change the repository's worktree list (and maybe its branches).
    fn queue(&self) -> Option<&Path> {
        match self {
            WorktreeEffect::Remove(r) => Some(&r.repo),
            WorktreeEffect::Move(r) => Some(&r.repo),
            WorktreeEffect::CheckFolder(_) | WorktreeEffect::CountChanges(_) => None,
        }
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}
