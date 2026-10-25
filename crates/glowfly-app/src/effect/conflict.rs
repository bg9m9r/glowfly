//! The conflict effects (`Effect::Conflict`): loading a conflicted file for the conflict view,
//! and counting the conflict markers in working files (runner/conflict_files.rs).

#[cfg(test)]
use super::SandboxRule;
use super::meta::EffectMeta;
use super::repo::ConflictRead;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ConflictEffect {
    /// The conflicted file `path`'s sides, and the picks a file saved with markers holds.
    Load { workdir: PathBuf, path: String, generation: u64 },
    /// Counts the conflict markers in the working files of `paths` (and, for a summary, lists
    /// the resolved files): only reads.
    ReadFiles { workdir: PathBuf, paths: Vec<String>, purpose: ConflictRead },
}

impl EffectMeta for ConflictEffect {
    fn name(&self) -> &'static str {
        match self {
            ConflictEffect::Load { .. } => "load-conflict",
            ConflictEffect::ReadFiles { .. } => "conflict-files",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        match self {
            ConflictEffect::Load { workdir, .. } | ConflictEffect::ReadFiles { workdir, .. } => Some(workdir),
        }
    }

    fn timed(&self) -> bool {
        true
    }

    /// Both only read.
    fn queue(&self) -> Option<&Path> {
        None
    }

    #[cfg(test)]
    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}
