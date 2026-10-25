//! Effects of the Clone dialog that need more than a git command: a sparse clone, listing the
//! user's repositories on a connected service and checking the parent folder. All are unscoped:
//! the clone ends the active job and the others answer the dialog whichever tab is active.

use super::EffectMeta;
#[cfg(test)]
use super::SandboxRule;
use glowfly_core::forge::Account;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq)]
pub enum CloneEffect {
    /// Runs the clone `cmd` (with `--sparse`) in `parent`, then sets `folders` (cone mode) in the
    /// new repository `dest`. Reported as `CloneMsg::Finished`.
    Sparse { parent: PathBuf, cmd: Vec<String>, dest: PathBuf, folders: Vec<String> },
    /// The repositories the user can reach on `account`, signing in from `cwd` (a folder outside
    /// any repository). Reported as `CloneMsg::ReposLoaded`.
    ListRepos { account: Account, cwd: PathBuf },
    /// Whether the parent folder `parent` exists, checked after `delay` so typing doesn't wait
    /// on the disk. Reported as `CloneMsg::ParentChecked`.
    CheckParent { parent: String, delay: std::time::Duration },
}

impl EffectMeta for CloneEffect {
    fn name(&self) -> &'static str {
        match self {
            CloneEffect::Sparse { .. } => "sparse-clone",
            CloneEffect::ListRepos { .. } => "list-my-repos",
            CloneEffect::CheckParent { .. } => "check-clone-parent",
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

    /// The list of the user's repositories asks a hosting service.
    #[cfg(test)]
    fn sandbox(&self) -> SandboxRule {
        match self {
            CloneEffect::Sparse { .. } | CloneEffect::CheckParent { .. } => SandboxRule::Run,
            CloneEffect::ListRepos { .. } => SandboxRule::Drop,
        }
    }
}
