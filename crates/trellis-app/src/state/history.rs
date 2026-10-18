//! The open file history: the commits that changed one file, the selected one, and where the
//! walk stopped.

use git2::Oid;
use trellis_core::repo::{DiffTarget, FileHistory, HistoryEntry, Snapshot};

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
    /// Something was at `path` in the working tree when the last batch loaded (or the file was
    /// restored since).
    pub on_disk: bool,
    /// The entry the first batch selects when it lists it (else the first entry).
    pub select: Option<Oid>,
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
            on_disk: false,
            select: None,
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
        self.on_disk = h.on_disk;
        self.loading = false;
    }

    /// The working-tree name to revert `target` in when it is one of these entries under an
    /// older name: the history's own `path`, which Open and Show already use. `None` otherwise.
    pub fn revert_dest(&self, target: &DiffTarget) -> Option<String> {
        let DiffTarget::Commit(oid, p) = target else { return None };
        (*p != self.path && self.entries.iter().any(|e| e.oid == *oid && e.path == *p)).then(|| self.path.clone())
    }

    /// Whether the file is in the working tree under `path`: what status says when it lists the
    /// path, else what the last walk saw on disk.
    pub fn in_workdir(&self, snap: Option<&Snapshot>) -> bool {
        if let Some(s) = snap {
            if s.missing_from_workdir(&self.path) {
                return false;
            }
            if s.unstaged.iter().chain(&s.staged).any(|f| f.path == self.path) {
                return true;
            }
        }
        self.on_disk
    }
}
