//! Desktop (OS) notifications: what `update` asks the runner to show (runner/notify.rs).

use std::path::PathBuf;

/// What kind of event a desktop notification reports: decides what a failure to show it does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NoteKind {
    /// The one sent when the setting is turned on; a failure is toasted.
    Confirm,
    /// The "N commits behind" alert after a background fetch.
    BehindAlert,
    /// A user-started job ended.
    Job,
}

/// One OS notification (runner/notify.rs).
#[derive(Clone, Debug, PartialEq)]
pub struct DesktopNote {
    pub kind: NoteKind,
    pub title: String,
    pub body: String,
    /// The repository the note is about, which a click on it shows.
    pub workdir: Option<PathBuf>,
}

impl DesktopNote {
    /// The notification sent on turning desktop notifications on, so the user sees they arrive.
    pub fn confirmation() -> Self {
        Self {
            kind: NoteKind::Confirm,
            title: "Desktop notifications are on".into(),
            body: "Glowfly will show finished jobs and behind alerts here while its window is in the background.".into(),
            workdir: None,
        }
    }
}
