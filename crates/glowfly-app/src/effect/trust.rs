//! What repository trust asks the runner (runner/trust.rs): reading repositories Glowfly just
//! made from a trusted one, so they can be trusted too (update/trust_inherit.rs).

use super::EffectMeta;
#[cfg(test)]
use super::SandboxRule;
use crate::state::trust::InheritKind;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq)]
pub enum TrustEffect {
    /// Resolves and scans each of `paths` (a new worktree, newly checked-out submodules) made
    /// from the trusted repository `from` (its real path). Reads only; then `open` opens.
    Inherit { kind: InheritKind, from: PathBuf, paths: Vec<PathBuf>, open: Option<PathBuf> },
}

impl EffectMeta for TrustEffect {
    fn name(&self) -> &'static str {
        match self {
            TrustEffect::Inherit { .. } => "inherit-trust",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        None
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
