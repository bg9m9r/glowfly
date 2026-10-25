//! How the changed-file lists show their files.

use serde::{Deserialize, Serialize};

/// How the changed-file lists show their files (list header toggle and Preferences > UI
/// Customization).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FileListMode {
    /// One row per path, in git's order.
    #[default]
    List,
    /// Files grouped under folder rows.
    Tree,
}

#[cfg(test)]
mod tests {
    use super::FileListMode;
    use crate::settings::Settings;

    #[test]
    fn an_old_settings_file_without_the_field_loads_list() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(s.file_list_mode, FileListMode::List);
        assert_eq!(Settings::default().file_list_mode, FileListMode::List);
    }

    #[test]
    fn tree_round_trips() {
        let s = Settings { file_list_mode: FileListMode::Tree, ..Settings::default() };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"file_list_mode\":\"Tree\""), "{json}");
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.file_list_mode, FileListMode::Tree);
    }
}
