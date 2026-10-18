//! Messages of the Git LFS preferences page (state/lfs.rs, update/lfs.rs, ui/prefs/lfs.rs) and
//! of the file menu's Track in Git LFS submenu (ui/file_menu.rs), the main window's "git-lfs
//! missing" banner (ui/lfs_banner.rs) and the diff's LFS summary (ui/diff/lfs_note.rs).

use trellis_core::lfs::{LfsCommand, LfsStatus, TrackTarget};

pub enum LfsMsg {
    /// The navigation row: switch Preferences to the page and read the status.
    ShowPage,
    /// Read the status again ("Check again").
    Reload,
    /// `git lfs install --local` in the active repository.
    Initialize,
    /// The palette: open the page and initialize once the status shows that it can.
    InitializeWhenReady,
    /// The typed pattern (checked in update).
    Track(String),
    /// A pattern's raw text.
    Untrack(String),
    /// An LFS command; Prune asks first.
    Run {
        command: LfsCommand,
        confirmed: bool,
    },
    /// Check whether git-lfs is installed (at startup).
    Detect,
    /// Track a file row's file, extension or folder (file context menu).
    TrackPath(TrackTarget),
    /// The main window's "git-lfs missing" banner was dismissed for this workdir (until restart).
    DismissBanner(std::path::PathBuf),
    /// The open diff shows its pointer text (true) or the LFS summary (false).
    ShowPointer(bool),
    // ---- from the runner
    /// git-lfs's version, None when it isn't installed.
    Detected(Option<String>),
    Loaded {
        generation: u64,
        result: Result<LfsStatus, String>,
    },
}

impl LfsMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::ShowPage
            | Self::Reload
            | Self::Initialize
            | Self::InitializeWhenReady
            | Self::Track(..)
            | Self::Untrack(..)
            | Self::Run { .. }
            | Self::Detect
            | Self::TrackPath(..)
            | Self::DismissBanner(..)
            | Self::ShowPointer(..)
            | Self::Detected(..)
            | Self::Loaded { .. } => None,
        }
    }
}
