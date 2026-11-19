//! Reads about one commit (`Effect::Commit`, runner/commit.rs): its detail and its signature
//! checked with git for the details panel, scoped to the repository's tab, and its whole message
//! for Copy > Message, unscoped. Reads only.

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
    /// Commit `oid`'s whole message as the details panel shows it, for Copy > Message; reported as
    /// `RepoMsg::MessageRead`.
    CopyMessage { workdir: PathBuf, oid: Oid },
}

impl EffectMeta for CommitEffect {
    fn name(&self) -> &'static str {
        match self {
            CommitEffect::LoadDetail { .. } => "load-detail",
            CommitEffect::VerifySignature { .. } => "verify-signature",
            CommitEffect::CopyMessage { .. } => "copy-message",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        match self {
            CommitEffect::LoadDetail { workdir, .. } | CommitEffect::VerifySignature { workdir, .. } => Some(workdir),
            // Unscoped: the clipboard isn't a tab's, so the copy lands whichever tab is active.
            CommitEffect::CopyMessage { .. } => None,
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
