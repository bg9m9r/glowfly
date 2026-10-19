//! The file history's choice between a commit's changes and the whole file, and between a
//! Markdown file's text and its rendered preview.

use serde::{Deserialize, Serialize};

/// What a file or folder history shows for the selected commit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoryPane {
    /// The commit's changes to the file.
    #[default]
    Diff,
    /// The whole file as of the commit.
    File,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryPrefs {
    /// The pane every history opens on (the header's Diff / File toggle sets it).
    pub pane: HistoryPane,
    /// Markdown files open rendered in the File view (its toolbar's Text / Preview switch sets
    /// it).
    pub preview: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn old_files_open_on_the_diff() {
        let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
        assert_eq!(s.history.pane, HistoryPane::Diff);
        assert!(!s.history.preview, "old files show Markdown as text");
    }

    #[test]
    fn history_prefs_round_trip() {
        let on = Settings { history: HistoryPrefs { pane: HistoryPane::File, preview: true }, ..Settings::default() };
        let back: Settings = serde_json::from_str(&serde_json::to_string(&on).unwrap()).unwrap();
        assert_eq!(back.history.pane, HistoryPane::File);
        assert!(back.history.preview);
        let partial: HistoryPrefs = serde_json::from_str("{}").unwrap();
        assert_eq!(partial, HistoryPrefs::default());
    }
}
