//! Messages and the effect's job of the pull request panel (state/pull_request.rs,
//! update/pull_request.rs, runner/pull_request.rs, ui/pull_request/). Its files open through
//! `RepoMsg::OpenDiff`, its Fetch button sends `RemoteMsg::Fetch`, and the browser and copy
//! actions are `Msg::OpenUrl` and `Msg::Copy`.

use git2::Oid;
use std::path::PathBuf;
use trellis_core::repo::BranchChanges;

/// What Effect::LoadPrFiles runs: the files pull request `id` changes, from its local tips.
#[derive(Debug)]
pub struct FilesJob {
    pub workdir: PathBuf,
    pub id: u64,
    /// (target, source)
    pub tips: (Oid, Oid),
    pub generation: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PullRequestMsg {
    /// Open the panel for pull request .0 (the sidebar row's double-click and menu).
    Open(u64),
    /// Close the panel (its Close button, Esc).
    Close,
    /// The file list of pull request `id`, compared from `tips` (target, source).
    FilesLoaded { id: u64, tips: (Oid, Oid), generation: u64, result: Result<Option<BranchChanges>, String> },
}

impl PullRequestMsg {
    /// Nothing is kept for an inactive tab: intents are for the active tab, and a dropped
    /// FilesLoaded is asked for again by `reload_pending` when its tab becomes active.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Open(_) | Self::Close | Self::FilesLoaded { .. } => None,
        }
    }
}
