//! The job of Effect::OpPreview: what an operation would do, worked out without changing the
//! repository (runner/op_preview.rs, glowfly_core::op_preview).

use super::EffectMeta;
#[cfg(test)]
use super::SandboxRule;
use std::path::{Path, PathBuf};

/// One preview of `request` in the repository at `workdir`.
#[derive(Clone, Debug, PartialEq)]
pub struct OpPreviewJob {
    pub workdir: PathBuf,
    pub request: glowfly_core::op_preview::Request,
}

impl OpPreviewJob {
    /// The effect's name, for the worker thread and the debug log.
    pub fn name() -> &'static str {
        "op-preview"
    }
}

impl EffectMeta for OpPreviewJob {
    fn name(&self) -> &'static str {
        "op-preview"
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
