//! Reads about one commit for the details panel (`Effect::Commit`, runner/commit.rs): its
//! detail, and its signature checked with git. Reads only, scoped to the repository's tab.

use super::EffectMeta;
use super::SandboxRule;
use git2::Oid;
use glowfly_core::signing::SigningFormat;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum CommitEffect {
    /// Commit `oid`'s message, author and files; reported as `RepoMsg::DetailLoaded`.
    LoadDetail { workdir: PathBuf, oid: Oid },
    /// Checks commit `oid`'s signature (in `format`) with git, for the details panel's badge;
    /// reported as `RepoMsg::SignatureChecked`.
    VerifySignature { workdir: PathBuf, oid: Oid, format: SigningFormat, generation: u64 },
}

impl EffectMeta for CommitEffect {
    fn name(&self) -> &'static str {
        match self {
            CommitEffect::LoadDetail { .. } => "load-detail",
            CommitEffect::VerifySignature { .. } => "verify-signature",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        match self {
            CommitEffect::LoadDetail { workdir, .. } | CommitEffect::VerifySignature { workdir, .. } => Some(workdir),
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
