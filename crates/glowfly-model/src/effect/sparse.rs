//! The sparse checkout effects (`Effect::Sparse`): the preferences page's state and
//! `git sparse-checkout` (runner/sparse.rs).

use super::SandboxRule;
use super::meta::EffectMeta;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum SparseEffect {
    /// Sparse checkout state for the preferences page; its paths only with `run_git` (trust).
    Load { workdir: PathBuf, generation: u64, run_git: bool },
    /// `git sparse-checkout` set, reapply or disable; unscoped like RunGit (ends the job).
    Checkout { cwd: PathBuf, command: glowfly_core::sparse_ops::SparseCommand },
}

impl EffectMeta for SparseEffect {
    fn name(&self) -> &'static str {
        match self {
            SparseEffect::Load { .. } => "sparse-status",
            SparseEffect::Checkout { .. } => "sparse-checkout",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        match self {
            SparseEffect::Load { workdir, .. } => Some(workdir),
            SparseEffect::Checkout { .. } => None,
        }
    }

    fn timed(&self) -> bool {
        true
    }

    /// A checkout changes the working tree; the state is a read.
    fn queue(&self) -> Option<&Path> {
        match self {
            SparseEffect::Load { .. } => None,
            SparseEffect::Checkout { cwd, .. } => Some(cwd),
        }
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}
