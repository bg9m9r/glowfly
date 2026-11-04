//! What a start or the recent list reads about repository folders without loading them
//! (`Effect::Session`, runner/session.rs): which folders are gone, and the common dir of each
//! restored tab. Unscoped and read-only.

use super::EffectMeta;
use super::SandboxRule;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum SessionEffect {
    /// Which of these repository folders are gone (not a directory or unreachable), and the
    /// identity of each that is there; reported as `RecentMsg::FoldersChecked`.
    CheckRepoFolders(Vec<PathBuf>),
    /// The common dir of each restored tab's folder, without loading it; reported as
    /// `TabsMsg::GroupsRead`.
    ReadTabGroups(Vec<PathBuf>),
}

impl EffectMeta for SessionEffect {
    fn name(&self) -> &'static str {
        match self {
            SessionEffect::CheckRepoFolders(_) => "check-repo-folders",
            SessionEffect::ReadTabGroups(_) => "read-tab-groups",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    fn timed(&self) -> bool {
        true
    }

    /// Reads only.
    fn queue(&self) -> Option<&Path> {
        None
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}
