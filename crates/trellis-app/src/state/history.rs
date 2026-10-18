//! The open file or folder history: the commits that changed it, the selected one (and, in a
//! folder history, the selected file), where the walk stopped, and the Diff / File choice with
//! the file's text as of the selected commit.

use git2::Oid;
use std::sync::Arc;
use trellis_core::highlight::Span;
use trellis_core::repo::{DiffTarget, FileHistory, HistoryEntry, Snapshot};

/// What the history follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HistoryKind {
    #[default]
    File,
    /// Every file under a folder (`path` is the folder, without a trailing '/').
    Folder,
}

/// What the right panel shows for the selected entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HistoryPane {
    /// The commit's changes to the file.
    #[default]
    Diff,
    /// The whole file as of the commit.
    File,
}

/// A file's lines as of a commit and their syntax spans (one list per line; `None` to paint
/// plain).
#[derive(Debug)]
pub struct FileTextData {
    pub lines: Vec<String>,
    pub spans: Option<Vec<Vec<Span>>>,
}

impl FileTextData {
    /// The spans of line `line`; empty when plain or out of range.
    pub fn spans(&self, line: usize) -> &[Span] {
        self.spans.as_ref().and_then(|s| s.get(line)).map_or(&[], Vec::as_slice)
    }
}

#[derive(Clone, Debug)]
pub enum TextState {
    Loading,
    Ready(Arc<FileTextData>),
    /// Why it can't be shown (binary, too large, a submodule, missing).
    Failed(String),
}

/// The File view's text: `path` as of commit `oid`.
#[derive(Clone, Debug)]
pub struct FileText {
    pub oid: Oid,
    pub path: String,
    pub state: TextState,
}

#[derive(Clone, Debug)]
pub struct FileHistoryView {
    /// Repo-relative path, as opened (a folder's without its trailing '/').
    pub path: String,
    pub kind: HistoryKind,
    /// The commit it was opened from; `None` for HEAD.
    pub at: Option<Oid>,
    /// `Some(at)` when the walk did not start at HEAD (shown in the header).
    pub as_of: Option<Oid>,
    /// The file's name at HEAD when the walk followed it forwards from `at` to a new name.
    pub head_path: Option<String>,
    pub entries: Vec<HistoryEntry>,
    pub selected: Option<usize>,
    /// A request is running.
    pub loading: bool,
    /// The last commit examined (and its time) when the walk stopped at the commit limit.
    pub searched_until: Option<(Oid, i64)>,
    /// Where "Search further" continues from; empty when the walk is complete.
    pub resume: Vec<(Oid, String)>,
    pub error: Option<String>,
    /// Something was at `today_path()` in the working tree when the last batch loaded (or the
    /// file was restored since).
    pub on_disk: bool,
    /// The entry the first batch (or a re-run) selects when it lists it (else the first entry).
    pub select: Option<Oid>,
    /// Batches loaded: 1 after the first, one more per Search further. A re-run after HEAD
    /// moved examines as many commits again.
    pub batches: usize,
    /// A re-run (HEAD moved) is in flight: its result replaces the list.
    pub rerun: bool,
    /// The selected file of a folder entry (an index into its `files`).
    pub file: usize,
    /// The folder file a re-run selects again when the entry still lists it.
    pub keep_file: Option<String>,
    pub pane: HistoryPane,
    /// The File view's text, when asked for.
    pub text: Option<FileText>,
}

impl FileHistoryView {
    /// A file history waiting for its first batch.
    #[cfg(test)]
    pub fn loading(path: String, at: Option<Oid>) -> Self {
        Self::loading_kind(path, at, HistoryKind::File)
    }

    /// A file or folder history waiting for its first batch.
    pub fn loading_kind(path: String, at: Option<Oid>, kind: HistoryKind) -> Self {
        Self {
            path,
            kind,
            at,
            as_of: None,
            head_path: None,
            entries: Vec::new(),
            selected: None,
            loading: true,
            searched_until: None,
            resume: Vec::new(),
            error: None,
            on_disk: false,
            select: None,
            batches: 0,
            rerun: false,
            file: 0,
            keep_file: None,
            pane: HistoryPane::Diff,
            text: None,
        }
    }

    pub fn selected_entry(&self) -> Option<&HistoryEntry> {
        self.entries.get(self.selected?)
    }

    /// The file's name in the working tree today: its name at HEAD when renamed after `at`.
    pub fn today_path(&self) -> &str {
        self.head_path.as_deref().unwrap_or(&self.path)
    }

    /// The selected commit, file and status the right panel shows: the entry's own in a file
    /// history, the selected file's in a folder history.
    pub fn shown(&self) -> Option<(Oid, &str, char)> {
        let e = self.selected_entry()?;
        match self.kind {
            HistoryKind::File => Some((e.oid, e.path.as_str(), e.status)),
            HistoryKind::Folder => e.files.get(self.file).map(|f| (e.oid, f.path.as_str(), f.status)),
        }
    }

    /// The diff of what `shown` names.
    pub fn target(&self) -> Option<DiffTarget> {
        self.shown().map(|(oid, path, _)| DiffTarget::Commit(oid, path.to_owned()))
    }

    /// The text the File view needs and doesn't have yet (or isn't loading).
    pub fn wanted_text(&self) -> Option<(Oid, String)> {
        if self.pane != HistoryPane::File {
            return None;
        }
        let (oid, path, status) = self.shown()?;
        let have = self.text.as_ref().is_some_and(|t| t.oid == oid && t.path == path);
        (status != 'D' && !have).then(|| (oid, path.to_owned()))
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

    /// Replaces the list with a re-run's results (nothing selected).
    pub fn replace(&mut self, h: FileHistory) {
        self.entries.clear();
        self.selected = None;
        self.append(h);
    }

    /// The working-tree name to revert `target` in when it is one of a file history's entries
    /// under another name: `today_path()`, which Open and Show already use. `None` otherwise.
    pub fn revert_dest(&self, target: &DiffTarget) -> Option<String> {
        let DiffTarget::Commit(oid, p) = target else { return None };
        let today = self.today_path();
        let listed = self.kind == HistoryKind::File && self.entries.iter().any(|e| e.oid == *oid && e.path == *p);
        (*p != today && listed).then(|| today.to_owned())
    }

    /// Whether the file is in the working tree under `today_path()`: what status says when it
    /// lists the path, else what the last walk saw on disk.
    pub fn in_workdir(&self, snap: Option<&Snapshot>) -> bool {
        let path = self.today_path();
        if let Some(s) = snap {
            if s.missing_from_workdir(path) {
                return false;
            }
            if s.unstaged.iter().chain(&s.staged).any(|f| f.path == path) {
                return true;
            }
        }
        self.on_disk
    }
}
