//! Messages of the Git LFS preferences page (state/lfs.rs, update/lfs.rs, ui/prefs/lfs.rs) and
//! of the file menu's Track in Git LFS submenu (ui/file_menu.rs).

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
    // ---- from the runner
    /// git-lfs's version, None when it isn't installed.
    Detected(Option<String>),
    Loaded {
        generation: u64,
        result: Result<LfsStatus, String>,
    },
}
