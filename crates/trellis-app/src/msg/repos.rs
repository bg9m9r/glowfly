//! Messages for finding repositories in project folders (update/repos.rs).

use super::Inactive;
use crate::state::repo_scan::FoundRepo;
use std::path::PathBuf;

/// The Find repositories dialog and its scan (update/repo_scan.rs).
pub enum ReposMsg {
    /// Open the dialog (and scan the project folders, when there are any).
    ShowFinder,
    /// Pick a folder to add to the project folders.
    AddFolder,
    /// Stop looking in this project folder.
    RemoveFolder(PathBuf),
    /// Scan the project folders again (Look again).
    Rescan,
    /// Result of `ReposEffect::Scan`.
    Scanned { generation: u64, report: Box<ScanFound> },
}

/// What a scan found, with each repository's identity (runner/repos.rs).
#[derive(Debug, Default)]
pub struct ScanFound {
    pub found: Vec<FoundRepo>,
    pub unreadable: Vec<(PathBuf, String)>,
    pub truncated: bool,
}

impl ReposMsg {
    /// What this message still needs when its tab is no longer active: nothing, as none of
    /// these is ever sent scoped.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::ShowFinder | Self::AddFolder | Self::RemoveFolder(_) | Self::Rescan | Self::Scanned { .. } => None,
        }
    }
}
