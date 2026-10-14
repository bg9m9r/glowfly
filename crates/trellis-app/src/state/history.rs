//! The open file history: the commits that changed one file, the selected one, and where the
//! walk stopped.

use git2::Oid;
use trellis_core::repo::{FileHistory, HistoryEntry};

#[derive(Clone, Debug)]
pub struct FileHistoryView {
    /// Repo-relative path, as opened.
    pub path: String,
    /// The commit it was opened from; `None` for HEAD.
    pub at: Option<Oid>,
    /// `Some(at)` when the walk did not start at HEAD (shown in the header).
    pub as_of: Option<Oid>,
    pub entries: Vec<HistoryEntry>,
    pub selected: Option<usize>,
    /// A request is running.
    pub loading: bool,
    /// The last commit examined (and its time) when the walk stopped at the commit limit.
    pub searched_until: Option<(Oid, i64)>,
    /// Where "Search further" continues from; empty when the walk is complete.
    pub resume: Vec<(Oid, String)>,
    pub error: Option<String>,
}

impl FileHistoryView {
    pub fn loading(path: String, at: Option<Oid>) -> Self {
        Self {
            path,
            at,
            as_of: None,
            entries: Vec::new(),
            selected: None,
            loading: true,
            searched_until: None,
            resume: Vec::new(),
            error: None,
        }
    }

    pub fn selected_entry(&self) -> Option<&HistoryEntry> {
        self.entries.get(self.selected?)
    }

    /// The walk reached the file's first commit.
    pub fn complete(&self) -> bool {
        !self.loading && self.error.is_none() && self.resume.is_empty()
    }

    /// Adds a batch of results, skipping commits already listed.
    pub fn append(&mut self, h: FileHistory) {
        for e in h.entries {
            if !self.entries.iter().any(|x| x.oid == e.oid) {
                self.entries.push(e);
            }
        }
        self.resume = h.resume;
        self.searched_until = h.searched_until;
        self.loading = false;
    }
}
