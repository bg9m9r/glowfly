//! The branch menus' background reads (`Effect::Branch`, runner/branches.rs): counting the
//! commits deleting a branch would leave on no branch, for the Delete dialog.

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
}

impl EffectMeta for BranchEffect {
    fn name(&self) -> &'static str {
        match self {
            BranchEffect::CountUnmerged { .. } => "count-unmerged",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        match self {
            BranchEffect::CountUnmerged { workdir, .. } => Some(workdir),
        }
    }

    fn timed(&self) -> bool {
        true
    }

    /// It only reads.
    fn queue(&self) -> Option<&Path> {
        None
    }

    #[cfg(test)]
    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}
