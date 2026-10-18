//! What a file diff is made of, which file it is of, and how to load it.

use git2::Oid;

#[derive(Clone, PartialEq, Debug)]
pub enum DiffTarget {
    Commit(Oid, String),
    Unstaged(String),
    Staged(String),
    /// `path` as it differs from commit `.0` (older) to commit `.1` (newer), tree to tree with
    /// renames detected.
    Between(Oid, Oid, String),
    /// `path` as it differs from commit `.0` to the working tree (staged and unstaged changes
    /// and untracked files; renames detected).
    ToWorkdir(Oid, String),
}

impl DiffTarget {
    pub fn path(&self) -> &str {
        match self {
            DiffTarget::Commit(_, p)
            | DiffTarget::Unstaged(p)
            | DiffTarget::Staged(p)
            | DiffTarget::Between(_, _, p)
            | DiffTarget::ToWorkdir(_, p) => p,
        }
    }

    /// A diff of the working tree or index. Only these targets support hunk and line operations.
    pub fn is_working_tree(&self) -> bool {
        matches!(self, DiffTarget::Unstaged(_) | DiffTarget::Staged(_))
    }

    /// A diff with the working tree on one side, so it is re-read when the working tree's
    /// status is (the working-tree diffs and a commit compared with the working tree).
    pub fn follows_workdir(&self) -> bool {
        self.is_working_tree() || matches!(self, DiffTarget::ToWorkdir(..))
    }

    /// A diff between stored versions, whose hunks can be reverted in the working tree.
    pub fn reverts_hunks(&self) -> bool {
        matches!(self, DiffTarget::Commit(..) | DiffTarget::Between(..))
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct DiffLine {
    pub origin: char,
    pub old: Option<u32>,
    pub new: Option<u32>,
    /// The line without its terminator; tabs are kept and expanded when drawn (`tabs::expand`).
    pub text: String,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Hunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct FileDiff {
    pub binary: bool,
    pub hunks: Vec<Hunk>,
    /// Filled in by `highlight::highlight_diff`; `None` means plain. A pending highlight is
    /// finished by Effect::HighlightDiff (`highlight::finish`).
    pub highlight: Option<crate::highlight::DiffHighlight>,
    /// Filled in by `intraline::intraline`: the word marks, and the similarity line pairs that
    /// the split layout and the marks share. Empty means no marks and pairing by position.
    pub intraline: crate::intraline::Intraline,
    /// With LoadDiffOptions::whole_file: every line of the file and its change blocks. None
    /// otherwise, or when it couldn't be mapped onto the hunks.
    pub whole: Option<Box<crate::whole_file::WholeFile>>,
}

/// How `load_diff_with` builds a file diff.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoadDiffOptions {
    /// Hide changes in the amount of whitespace and at line ends (git `-b --ignore-space-at-eol`).
    /// Hunk indexes then no longer match `apply_hunk` or `apply_lines`: act on such a diff with
    /// `apply_shown`, or `revert` with `RevertRequest::ignore_whitespace`.
    pub ignore_whitespace: bool,
    /// Also load the file with unlimited context into FileDiff::whole (the whole-file view).
    /// The normal hunks are loaded as always, so hunk indexes still match apply_hunk.
    pub whole_file: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum HunkOp {
    Stage,
    Unstage,
    Discard,
}
