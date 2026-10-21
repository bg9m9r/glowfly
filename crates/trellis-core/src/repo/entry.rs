//! A row of a file list (status, a commit's or a comparison's files), and why Trellis may
//! refuse to act on it.

use crate::i18n::Lang;

#[derive(Clone, PartialEq, Debug)]
pub struct FileEntry {
    /// The repository-relative path, '/'-separated. Names no file when `hazard` is `NotUtf8`.
    pub path: String,
    /// M, A, D, R, T, ? (untracked) or ! (conflicted: see `load_conflict`).
    pub status: char,
    /// Why acting on this row could reach the wrong file; `None` for almost every row.
    pub hazard: Option<Hazard>,
    /// The row's stored content (index, commit, or the working copy where git has none) is a
    /// Git LFS pointer (`lfs_marks`).
    pub lfs: bool,
    /// For a staged rename in a status read (an 'R' row of the staged list): its path in HEAD,
    /// when that is UTF-8. None everywhere else, commit and comparison lists included.
    pub renamed_from: Option<String>,
}

impl FileEntry {
    /// A row for a path with nothing unusual about it.
    pub fn new(path: impl Into<String>, status: char) -> Self {
        Self { path: path.into(), status, hazard: None, lfs: false, renamed_from: None }
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
    pub fn reason(&self, t: Lang) -> String {
        match self {
            Self::NotUtf8(_) => t.tr("files.hazard.not_utf8.reason").to_owned(),
            Self::Twins(others) => t.trf("files.hazard.twins.reason", &[("paths", &others.join(", "))]),
        }
    }

    /// A few words shown after the path.
    pub fn label(&self, t: Lang) -> &'static str {
        match self {
            Self::NotUtf8(_) => t.tr("files.hazard.not_utf8.label"),
            Self::Twins(_) => t.tr("files.hazard.twins.label"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hazard_texts_follow_the_language() {
        let bytes = Hazard::NotUtf8(b"n\xffme".to_vec());
        let twins = Hazard::Twins(vec!["A.txt".into(), "a.TXT".into()]);
        assert_eq!(bytes.reason(Lang::En), "This file's name isn't UTF-8, so Trellis can't act on it safely; use git on the command line.");
        assert_eq!(
            twins.reason(Lang::En),
            "This file system can't tell this path from A.txt, a.TXT: they are one file on disk. Rename one of them in git first."
        );
        assert_eq!((bytes.label(Lang::En), twins.label(Lang::En)), ("name isn't UTF-8", "same file as another path"));
        let de = twins.reason(Lang::De);
        assert!(de.contains("A.txt, a.TXT") && de != twins.reason(Lang::En), "{de}");
        assert_ne!(bytes.reason(Lang::De), bytes.reason(Lang::En));
        assert_ne!(bytes.label(Lang::De), bytes.label(Lang::En));
        assert_ne!(twins.label(Lang::De), twins.label(Lang::En));
    }
}
