//! Messages of Trellis's own updates and release notes (update/updates.rs).

use super::Inactive;
use glowfly_core::app_update::{InstallKind, Release};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateMsg {
    /// Sent once by the shell at start: arms the daily check and shows a new version's notes.
    Startup,
    /// Preferences' Check now and the palette's Check for updates.
    CheckNow,
    /// Download the offered release's file (or open its page when there is none for this system).
    Download,
    /// Open the downloaded installer.
    Install,
    /// Show the downloaded archive in the file manager.
    ShowFile,
    /// Open the offered release's page, or the releases page.
    OpenReleasePage,
    /// Open the release notes dialog.
    ShowNotes,
    // ---- from the runner
    /// A check finished: the newest release (None when there is none yet).
    Checked { manual: bool, result: Result<Option<CheckedRelease>, String> },
    /// A download finished: where the verified file was saved.
    Downloaded(Result<PathBuf, String>),
}

/// The newest release, and how this copy was installed (which decides the file it is offered).
#[derive(Clone, Debug, PartialEq)]
pub struct CheckedRelease {
    pub release: Release,
    pub install: InstallKind,
}

impl UpdateMsg {
    /// Updates are unscoped (nothing belongs to a repository), so nothing arrives for an
    /// inactive tab.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Startup
            | Self::CheckNow
            | Self::Download
            | Self::Install
            | Self::ShowFile
            | Self::OpenReleasePage
            | Self::ShowNotes
            | Self::Checked { .. }
            | Self::Downloaded(_) => None,
        }
    }
}
