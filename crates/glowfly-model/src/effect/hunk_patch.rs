//! Copy hunk as patch's read (`Effect::CopyHunkPatch`, runner/hunk_patch.rs): the hunk read
//! again from the repository, byte for byte, reported as `DiffMsg::HunkPatchRead`. Reads only.

use super::EffectMeta;
use super::SandboxRule;
use glowfly_core::repo::{DiffTarget, Hunk};
use std::path::{Path, PathBuf};

/// Hunk `index` of `target`'s diff as patch text (`repo::hunk_patch`).
#[derive(Debug)]
pub struct HunkPatchRequest {
    pub workdir: PathBuf,
    pub target: DiffTarget,
    pub index: usize,
    /// The hunk as the user saw it: refused as stale unless git produces it again.
    pub expected: Hunk,
    /// The diff was shown with whitespace changes hidden, so its hunks are counted in that one.
    pub ignore_whitespace: bool,
}

impl EffectMeta for HunkPatchRequest {
    fn name(&self) -> &'static str {
        "copy-hunk-patch"
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

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}
