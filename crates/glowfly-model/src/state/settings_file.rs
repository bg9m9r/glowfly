//! Glowfly's hold on its own settings file: the settings as last saved, and whether saving is
//! held back because the file couldn't be read (update/settings_file.rs). While it is, the
//! settings in use are defaults, and writing them would replace the user's real ones.
//!
//! Some settings Glowfly records by itself, never as a choice the user makes (`user_view`
//! leaves them out): which release notes were seen, when the update check last worked, and
//! which one-time notices were shown. A read after a hold keeps the newer of those
//! (`carry_records`) and doesn't count them as edits the read drops.

use glowfly_core::app_update::Version;
use glowfly_core::settings::{LoadProblem, Settings};
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

    /// Holds saving back when the startup load says the file stays unread (`problem.locked`).
    /// The shell calls this before any startup step runs, since some of them save settings;
    /// `PrefsMsg::SettingsUnreadable` then tells the user.
    pub fn hold(&mut self, problem: Option<&LoadProblem>) {
        if let Some(LoadProblem { locked: Some(file), detail, .. }) = problem {
            self.unreadable = Some(Unreadable { file: file.clone(), detail: detail.clone(), reading: false });
        }
    }

    /// Whether settings may be written now.
    pub fn writable(&self) -> bool {
        self.unreadable.is_none()
    }
}

/// `s` as JSON without what Glowfly records by itself: what the user set.
pub fn user_view(s: &Settings) -> String {
    let mut s = s.clone();
    s.updates.notes_seen = None;
    s.updates.last_checked = None;
    s.detached_head_warned = false;
    s.status_tip_shown.clear();
    serde_json::to_string(&s).unwrap_or_default()
}

/// Carries what Glowfly recorded by itself in `ours` into `file` (just read) where ours is
/// newer: the later release notes seen and update check, and notices already shown. Returns
/// whether anything was carried, so `file` needs saving.
pub fn carry_records(ours: &Settings, file: &mut Settings) -> bool {
    let mut carried = false;
    let version = |v: &Option<String>| v.as_deref().and_then(Version::parse);
    if let Some(v) = version(&ours.updates.notes_seen)
        && version(&file.updates.notes_seen).is_none_or(|f| v > f)
    {
        file.updates.notes_seen.clone_from(&ours.updates.notes_seen);
        carried = true;
    }
    if let Some(t) = ours.updates.last_checked
        && file.updates.last_checked.is_none_or(|f| t > f)
    {
        file.updates.last_checked = Some(t);
        carried = true;
    }
    if ours.detached_head_warned && !file.detached_head_warned {
        file.detached_head_warned = true;
        carried = true;
    }
    for (repo, shown) in &ours.status_tip_shown {
        if *shown && file.status_tip_shown.get(repo) != Some(&true) {
            file.status_tip_shown.insert(repo.clone(), true);
            carried = true;
        }
    }
    carried
}
