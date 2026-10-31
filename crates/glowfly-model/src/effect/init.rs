//! Effects of the Init dialog: checking its folder, reading the name to suggest for a license,
//! and the init job. All unscoped: the dialog and the job live outside any repository's tab.

use super::EffectMeta;
use super::SandboxRule;
use glowfly_core::init_ops::InitPlan;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, PartialEq)]
pub enum InitEffect {
    /// What the folder text `folder` names, checked after `delay` so typing doesn't wait on the
    /// disk. Reported as `InitMsg::FolderChecked`.
    CheckFolder { folder: String, delay: Duration },
    /// git's user.name (global, or `cwd`'s repository's). Reported as `InitMsg::AuthorRead`.
    ReadAuthor { cwd: PathBuf },
    /// Creates the repository in `folder` and adds `plan`'s files. It ends the active job
    /// (`InitMsg::Finished`).
    Run { folder: PathBuf, plan: InitPlan },
}

impl EffectMeta for InitEffect {
    fn name(&self) -> &'static str {
        match self {
            InitEffect::CheckFolder { .. } => "check-init-folder",
            InitEffect::ReadAuthor { .. } => "read-author",
            InitEffect::Run { .. } => "init",
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

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}
