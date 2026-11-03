//! The update check's preferences (Preferences > General > Updates), which version's release
//! notes were last shown, how a new version tells what's new (`WhatsNew`) and the status bar's
//! What's new chip while it waits to be clicked (`NewsChip`).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdatePrefs {
    /// Check for a newer release once a day.
    pub check: bool,
    /// When the last check succeeded (Unix seconds).
    pub last_checked: Option<i64>,
    /// The version whose release notes were shown (or skipped) last; None in a file from before
    /// release notes existed.
    pub notes_seen: Option<String>,
    /// What the first start after an upgrade does.
    pub whats_new: WhatsNew,
    /// The What's new chip waiting in the status bar, until clicked or it runs out.
    pub news_chip: Option<NewsChip>,
}

impl Default for UpdatePrefs {
    fn default() -> Self {
        Self { check: true, last_checked: None, notes_seen: None, whats_new: WhatsNew::default(), news_chip: None }
    }
}

/// What the first start after an upgrade does with the new version's release notes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WhatsNew {
    /// Opens the release notes by itself.
    Dialog,
    /// Shows a quiet "What's new" chip in the status bar that opens them.
    #[default]
    Chip,
    /// Shows nothing; the notes stay in Help and the palette.
    Off,
}

/// The status bar's What's new chip, recorded so it outlasts a restart.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewsChip {
    /// The version it tells about (the chip is shown only while that version runs).
    pub version: String,
    /// The version seen before, whose later releases the notes mark New.
    pub since: Option<String>,
    /// When it goes away by itself (Unix seconds); None until the first frame that knows the time.
    pub until: Option<i64>,
}
