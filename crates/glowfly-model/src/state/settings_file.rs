//! Glowfly's hold on its own settings file: the settings as last saved, and whether saving is
//! held back because the file couldn't be read (update/settings_file.rs). While it is, the
//! settings in use are defaults, and writing them would replace the user's real ones.

use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct SettingsFile {
    /// The settings as last saved (JSON), to detect edits made in Preferences.
    pub saved: String,
    /// The file couldn't be read: no save is made until a try reads it.
    pub unreadable: Option<Unreadable>,
}

/// The settings file Glowfly couldn't read, as the banner shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Unreadable {
    pub file: PathBuf,
    /// What failed, in the OS's words.
    pub detail: String,
    /// A try at reading it again is running.
    pub reading: bool,
}

impl SettingsFile {
    pub fn new(saved: String) -> Self {
        SettingsFile { saved, unreadable: None }
    }

    /// Whether settings may be written now.
    pub fn writable(&self) -> bool {
        self.unreadable.is_none()
    }
}
