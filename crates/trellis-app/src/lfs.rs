//! Messages of the Git LFS preferences page (state/lfs.rs, update/lfs.rs, ui/prefs/lfs.rs).

use trellis_core::lfs::{LfsCommand, LfsStatus};

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
    // ---- from the runner
    Loaded {
        generation: u64,
        result: Result<LfsStatus, String>,
    },
}
