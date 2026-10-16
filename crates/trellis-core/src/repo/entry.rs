//! A row of a file list (status, a commit's or a comparison's files), and why Trellis may
//! refuse to act on it.

#[derive(Clone, PartialEq, Debug)]
pub struct FileEntry {
    /// The repository-relative path, '/'-separated. Names no file when `hazard` is `NotUtf8`.
    pub path: String,
    /// M, A, D, R, T, ? (untracked) or ! (conflicted: see `load_conflict`).
    pub status: char,
    /// Why acting on this row could reach the wrong file; `None` for almost every row.
    pub hazard: Option<Hazard>,
}

impl FileEntry {
    /// A row for a path with nothing unusual about it.
    pub fn new(path: impl Into<String>, status: char) -> Self {
        Self { path: path.into(), status, hazard: None }
    }
}

/// Why Trellis won't act on a file row (see `FileEntry::hazard`).
#[derive(Clone, PartialEq, Debug)]
pub enum Hazard {
    /// The name isn't UTF-8 (these are its bytes): `path` shows each byte it can't show as
    /// `\xHH`, so it names another file or none.
    NotUtf8(Vec<u8>),
    /// Other index paths that this file system, or git with `core.ignorecase`, treats as the
    /// same file (they differ only in case or Unicode normalization): one file on disk stands
    /// for all of them, so discarding, deleting, renaming or restoring one changes the others.
    Twins(Vec<String>),
}

impl Hazard {
    /// Why actions on the row are off, for its tooltip and refusals.
    pub fn reason(&self) -> String {
        match self {
            Self::NotUtf8(_) => "This file's name isn't UTF-8, so Trellis can't act on it safely; use git on the command line.".into(),
            Self::Twins(others) => format!(
                "This file system can't tell this path from {}: they are one file on disk. Rename one of them in git first.",
                others.join(", ")
            ),
        }
    }

    /// A few words shown after the path.
    pub fn label(&self) -> &'static str {
        match self {
            Self::NotUtf8(_) => "name isn't UTF-8",
            Self::Twins(_) => "same file as another path",
        }
    }
}
