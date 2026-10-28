//! The Git LFS effects (`Effect::Lfs`): whether git-lfs is installed, and the preferences page's
//! status (runner/lfs.rs).

#[cfg(test)]
use super::SandboxRule;
use super::meta::EffectMeta;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum LfsEffect {
    /// Whether git-lfs is installed (`git lfs version`, always run outside every repository, so
    /// no repository's alias.lfs answers). Unscoped: the answer is global.
    Detect,
    /// Git LFS status for the preferences page: installed version, set up or not, tracked patterns.
    Load { workdir: PathBuf, generation: u64 },
}

impl EffectMeta for LfsEffect {
    fn name(&self) -> &'static str {
        match self {
            LfsEffect::Detect => "lfs-detect",
            LfsEffect::Load { .. } => "lfs-status",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        match self {
            LfsEffect::Detect => None,
            LfsEffect::Load { workdir, .. } => Some(workdir),
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
