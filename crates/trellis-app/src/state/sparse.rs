//! The Sparse Checkout preferences page: the status read in the background, the form (mode and
//! paths) and what it was filled with, and the texts the page and its job toasts show.

use trellis_core::sparse_ops::{SparseMode, SparseStatus};

/// Job labels.
pub const ENABLE_LABEL: &str = "Turn on sparse checkout";
pub const SAVE_LABEL: &str = "Save sparse paths";
pub const REAPPLY_LABEL: &str = "Reapply sparse paths";
pub const DISABLE_LABEL: &str = "Turn off sparse checkout";

#[derive(Default)]
pub struct SparsePage {
    /// None while the status is read, or when reading it failed (`error`).
    pub status: Option<SparseStatus>,
    pub error: Option<String>,
    /// Bumped by each read; an older read's result is ignored.
    pub generation: u64,
    /// The Folders / Patterns field (a view text buffer), one path per line.
    pub paths: String,
    /// The Mode radio buttons (a form field).
    pub mode: SparseMode,
    /// What the form was last filled with, to tell whether the user edited it.
    pub filled: (String, SparseMode),
    /// Why the last Turn on / Save paths was refused.
    pub paths_error: Option<String>,
    /// A sparse job succeeded: fill the form from the next status even if it was edited.
    pub refill: bool,
    /// The last read ran no git (git held), so the paths are unknown.
    pub held: bool,
}

impl SparsePage {
    /// Fills the form from the status: its paths and mode when on, an empty folders list when off.
    pub fn fill(&mut self) {
        let (paths, mode) = match &self.status {
            Some(s) if s.enabled => (s.paths.join("\n"), s.mode),
            _ => (String::new(), SparseMode::Cone),
        };
        self.paths = paths.clone();
        self.mode = mode;
        self.filled = (paths, mode);
        self.paths_error = None;
    }

    /// Whether the form differs from what it was filled with.
    pub fn edited(&self) -> bool {
        self.paths != self.filled.0 || self.mode != self.filled.1
    }

    /// The primary button and its job: Turn on when off, Save paths when on.
    pub fn apply_label(&self) -> &'static str {
        if self.status.as_ref().is_some_and(|s| s.enabled) { SAVE_LABEL } else { ENABLE_LABEL }
    }
}

/// The Status row: on or off, the mode, and how many tracked files are checked out.
pub fn summary(s: &SparseStatus) -> String {
    if !s.enabled {
        return "Off. Every tracked file is checked out.".into();
    }
    let mode = match s.mode {
        SparseMode::Cone => "On, folders (cone) mode.",
        SparseMode::Patterns => "On, patterns mode.",
    };
    if s.sparse_index {
        return mode.into();
    }
    let n = s.tracked.saturating_sub(s.skipped);
    let files = if s.tracked == 1 { "file" } else { "files" };
    let verb = if n == 1 { "is" } else { "are" };
    format!("{mode} {n} of {} tracked {files} {verb} checked out.", s.tracked)
}

/// The success toast when git kept `n` changed files outside the new paths.
pub fn kept_headline(label: &str, n: usize) -> String {
    if n == 1 {
        format!("{label}: 1 changed file outside the paths was kept. Commit or discard it, then Reapply.")
    } else {
        format!("{label}: {n} changed files outside the paths were kept. Commit or discard them, then Reapply.")
    }
}
