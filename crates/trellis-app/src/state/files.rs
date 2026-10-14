//! The New file and Rename file dialogs' form: the typed path, the last error and whether a
//! request is running.

/// A path field with the outcome of its last submission.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathForm {
    pub text: String,
    /// (the text it was produced for, the message): shown only while the field still holds that
    /// text, so editing the path hides it without anyone clearing it.
    pub error: Option<(String, String)>,
    /// A create or rename is running; submitting again is off until it reports back.
    pub busy: bool,
}

impl PathForm {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), error: None, busy: false }
    }

    /// The error for the text now in the field, if any.
    pub fn current_error(&self) -> Option<&str> {
        self.error.as_ref().filter(|(t, _)| *t == self.text).map(|(_, e)| e.as_str())
    }
}

/// Whether status lists `path` (unstaged or staged) as a file that is still in the working tree,
/// so a new file can't take its place. A path equal to `except` ignoring case (a rename's source)
/// doesn't count. Core checks the disk again; this only catches what the snapshot shows.
pub fn listed(snap: &trellis_core::repo::Snapshot, path: &str, except: Option<&str>) -> bool {
    if except.is_some_and(|e| e.eq_ignore_ascii_case(path)) {
        return false;
    }
    snap.unstaged.iter().chain(&snap.staged).any(|f| f.path == path) && !snap.missing_from_workdir(path)
}
