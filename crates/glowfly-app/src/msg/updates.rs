//! Messages of Glowfly's own updates and release notes (update/updates.rs).

use super::Inactive;
use glowfly_core::app_update::{InstallKind, LastInstall, Progress, Release, Saved, SavedProblem};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateMsg {
    /// Sent once by the shell at start: arms the daily check and shows a new version's notes.
    Startup,
    /// Preferences' Check now and the palette's Check for updates.
    CheckNow,
    /// Download the offered release's file (or open its page when there is none for this system),
    /// again when the one downloaded before is gone or changed.
    Download,
    /// Preferences' Cancel while the download runs: stops it and removes the partial file.
    CancelDownload,
    /// Install the downloaded update and restart, or open its installer and quit, after the
    /// unsaved-work checks.
    InstallAndRestart,
    /// The restart confirmation's button: as `InstallAndRestart`, without asking about the
    /// tabs' unsaved work again.
    InstallAndRestartConfirmed,
    /// Show the downloaded archive in the file manager.
    ShowFile,
    /// Open the offered release's page, or the releases page.
    OpenReleasePage,
    /// Open the release notes dialog.
    ShowNotes,
    /// Show the update helper's log in the file manager.
    ShowUpdateLog(PathBuf),
    // ---- from the runner
    /// A check finished: the newest release (None when there is none yet).
    Checked { manual: bool, result: Result<Option<CheckedRelease>, String> },
    /// How far the running download is (a few times a second).
    DownloadProgress(Progress),
    /// A download finished: where the verified file was saved, and its checksum.
    Downloaded(Result<Saved, String>),
    /// The download stopped for Cancel; its partial file is removed.
    DownloadCancelled,
    /// The downloaded `file` was read again: None when it still matches, else what is wrong.
    SavedChecked { file: PathBuf, problem: Option<SavedProblem> },
    /// Install and restart was prepared (the window closes now) or failed (nothing changed).
    RestartReady(Result<(), String>),
    /// At the start: the last installer run failed or was cancelled (read from the update log).
    LastInstall(LastInstall),
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
            | Self::CancelDownload
            | Self::InstallAndRestart
            | Self::InstallAndRestartConfirmed
            | Self::ShowFile
            | Self::OpenReleasePage
            | Self::ShowNotes
            | Self::Checked { .. }
            | Self::DownloadProgress(_)
            | Self::Downloaded(_)
            | Self::DownloadCancelled
            | Self::SavedChecked { .. }
            | Self::ShowUpdateLog(_)
            | Self::RestartReady(_)
            | Self::LastInstall(_) => None,
        }
    }
}
