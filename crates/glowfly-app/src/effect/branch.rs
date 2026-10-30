//! The branch menus' background work (`Effect::Branch`, runner/branches.rs): counting the
//! commits deleting a branch would leave on no branch, for the Delete dialog, and the Push to
//! all remotes job.

#[cfg(test)]
use super::SandboxRule;
use super::meta::EffectMeta;
use git2::Oid;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum BranchEffect {
    /// Counts the commits local branch `name`'s `tip` reaches that no other branch does
    /// (`repo::unmerged_count`); reports `BranchMsg::UnmergedCounted`, scoped to the tab that asked.
    CountUnmerged { workdir: PathBuf, name: String, tip: Oid },
    /// Pushes local `branch` to every remote in turn (`push_all::run`); reports
    /// `BranchMsg::PushedAll`. A job, unscoped like RunGit.
    PushAll { cwd: PathBuf, branch: String, pushes: Vec<glowfly_core::push_all::RemotePush> },
}

impl EffectMeta for BranchEffect {
    fn name(&self) -> &'static str {
        match self {
            BranchEffect::CountUnmerged { .. } => "count-unmerged",
            BranchEffect::PushAll { .. } => "push-all",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        match self {
            BranchEffect::CountUnmerged { workdir, .. } => Some(workdir),
            BranchEffect::PushAll { .. } => None,
        }
    }

    fn timed(&self) -> bool {
        true
    }

    /// The count only reads; pushes update remote-tracking refs at most, like a push RunGit.
    fn queue(&self) -> Option<&Path> {
        None
    }

    /// Journeys push to local bare repositories.
    #[cfg(test)]
    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}
