//! Messages for finding repositories in project folders, and fetching or pulling many listed
//! repositories at once (update/repos.rs).

use super::Inactive;
use crate::state::repo_scan::FoundRepo;
use crate::state::repo_sync::{SyncKind, SyncScope};
use std::path::PathBuf;
use trellis_core::repo_sync::Outcome;

/// The Find repositories dialog and its scan (update/repo_scan.rs), and fetch or pull across
/// repositories (update/repo_sync.rs).
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
    /// Fetch or pull every repository in `scope`, a few at a time, and show how each went. While
    /// one runs, shows it again instead.
    StartSync { kind: SyncKind, scope: SyncScope },
    /// Result of `ReposEffect::Sync` for row `index` of run `generation`.
    Synced { generation: u64, index: usize, outcome: Outcome },
    /// Show the last fetch or pull across repositories again (its toast's action).
    ShowSync,
    /// Stop starting repositories in the running fetch or pull; those running finish.
    StopSync,
    /// Run row `index` of run `generation` again (a failed or stopped row).
    RetrySync { generation: u64, index: usize },
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
            Self::ShowFinder
            | Self::AddFolder
            | Self::RemoveFolder(_)
            | Self::Rescan
            | Self::Scanned { .. }
            | Self::StartSync { .. }
            | Self::Synced { .. }
            | Self::ShowSync
            | Self::StopSync
            | Self::RetrySync { .. } => None,
        }
    }
}
