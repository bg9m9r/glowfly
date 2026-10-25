//! The blame view's request (`Effect::LoadBlame`).

use super::EffectMeta;
#[cfg(test)]
use super::SandboxRule;
use git2::Oid;
use glowfly_core::repo::BlameOptions;
use std::path::{Path, PathBuf};

/// Blame `path` of `workdir` at HEAD (`at: None`) or as of commit `at`.
#[derive(Debug)]
pub struct BlameRequest {
    pub workdir: PathBuf,
    pub path: String,
    pub at: Option<Oid>,
    /// Whitespace, moves and skipped commits (the header's toggles and skipped chip).
    pub opts: BlameOptions,
    /// Highlight the lines too (Preferences > Diff viewer syntax highlighting).
    pub highlight: bool,
    pub generation: u64,
    /// For a HEAD blame re-run after HEAD moved: the HEAD it was last shown at; when `path` is
    /// gone from HEAD, blame the name renames since then lead to.
    pub follow: Option<Oid>,
}

impl EffectMeta for BlameRequest {
    fn name(&self) -> &'static str {
        "blame"
    }

    fn scope(&self) -> Option<&PathBuf> {
        Some(&self.workdir)
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
