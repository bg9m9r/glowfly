//! The update check's preferences (Preferences > General > Updates) and which version's
//! release notes were last shown.

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
}

impl Default for UpdatePrefs {
    fn default() -> Self {
        Self { check: true, last_checked: None, notes_seen: None }
    }
}
