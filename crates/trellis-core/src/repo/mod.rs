//! Repository model built on libgit2.
//! Reads (history, refs, status, diffs) and index/working-tree edits happen here; operations
//! that should run hooks or talk to remotes go through `gitcli`.

use crate::graph::Layout;
use git2::{Oid, RepositoryState};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

mod conflict;
mod diff;
mod history;
mod index;
mod lines;
mod load;
mod resolve;
mod restore;
mod revert;

pub use conflict::{Chunk, ConflictContent, ConflictFile, ConflictIds, Operation, Segment, Side, SideLabels, load_conflict};
pub use diff::{commit_detail, commit_stats, compare_files, load_diff, load_diff_with};
use diff::{head_tree, path_opts, workdir_opts};
pub use history::{FileHistory, HistoryEntry, HistoryStart, file_history};
pub use index::{apply_hunk, discard_file, stage_all, stage_file, unstage_paths};
pub use lines::apply_lines;
pub use load::{load, messages_between, status_at};
pub use resolve::{Choice, render_output, resolve_with_side, write_resolution};
pub use restore::{RestoredFrom, restore_file};
pub use revert::revert_hunk;

#[derive(Clone)]
pub struct CommitInfo {
    pub oid: Oid,
    pub summary: String,
    /// Message body flattened to one line (shown dimmed after the summary).
    pub body: String,
    pub author: String,
    pub email: String,
    pub time: i64,
    pub parents: Vec<Oid>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RefKind {
    Head,
    Local,
    Remote,
    Tag,
}

#[derive(Clone)]
pub struct RefLabel {
    pub name: String,
    /// Full ref name, e.g. refs/heads/main (empty for a detached HEAD label).
    pub full: String,
    pub kind: RefKind,
    pub is_head: bool,
}

/// A branch's configured upstream (`branch.<name>.remote` and `.merge`).
#[derive(Clone, Debug, PartialEq)]
pub struct Tracking {
    /// The remote name, or "." when the upstream is a local branch.
    pub remote: String,
    /// The upstream branch name on that remote, without refs/heads/.
    pub merge: String,
}

impl Tracking {
    /// Whether the upstream is on a remote rather than a local branch.
    pub fn is_remote(&self) -> bool {
        self.remote != "."
    }

    /// "remote/merge", or just the branch name for a local upstream.
    pub fn display(&self) -> String {
        if self.is_remote() { format!("{}/{}", self.remote, self.merge) } else { self.merge.clone() }
    }
}

#[derive(Clone)]
pub struct BranchInfo {
    pub name: String,
    pub oid: Option<Oid>,
    pub is_head: bool,
    /// The upstream ref's short name (e.g. origin/x), when it exists.
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    /// The configured upstream, present even when its remote-tracking ref is gone.
    pub tracking: Option<Tracking>,
    /// Full name of the resolved upstream ref, e.g. refs/remotes/origin/x; None when
    /// `upstream` is None.
    pub upstream_ref: Option<String>,
    /// Whether HEAD contains the branch tip, so deleting the branch loses no commits. True for
    /// the checked-out branch; false when HEAD is unborn.
    pub merged: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub struct FileEntry {
    pub path: String,
    /// M, A, D, R, T, ? (untracked) or ! (conflicted: see `load_conflict`).
    pub status: char,
}

#[derive(Clone)]
pub struct StashInfo {
    pub index: usize,
    pub message: String,
    pub oid: Oid,
}

pub struct Snapshot {
    pub workdir: PathBuf,
    pub git_dir: PathBuf,
    pub name: String,
    /// (remote name, fetch URL)
    pub remote_urls: Vec<(String, String)>,
    pub head_branch: Option<String>,
    pub head_oid: Option<Oid>,
    /// HEAD's full commit message (used to pre-fill "amend").
    pub head_message: Option<String>,
    pub state: RepositoryState,
    pub commits: Vec<CommitInfo>,
    pub row_of: HashMap<Oid, usize>,
    pub truncated: bool,
    pub refs: HashMap<Oid, Vec<RefLabel>>,
    pub locals: Vec<BranchInfo>,
    pub remotes: Vec<(String, Vec<(String, Oid)>)>,
    pub tags: Vec<(String, Oid)>,
    pub stashes: Vec<StashInfo>,
    pub unstaged: Vec<FileEntry>,
    pub staged: Vec<FileEntry>,
    pub graph: Layout,
}

impl Snapshot {
    /// Loaded commits reachable from `from` but not from `exclude` (newest first), e.g. the
    /// commits a branch would contribute to its target. Limited to the loaded history.
    pub fn commits_between(&self, from: Oid, exclude: Option<Oid>) -> Vec<&CommitInfo> {
        let reach = |start: Oid| {
            let mut seen = HashSet::new();
            let mut stack = vec![start];
            while let Some(o) = stack.pop() {
                if let Some(&row) = self.row_of.get(&o)
                    && seen.insert(o)
                {
                    stack.extend(self.commits[row].parents.iter().copied());
                }
            }
            seen
        };
        let excluded = exclude.map(reach).unwrap_or_default();
        let wanted = reach(from);
        self.commits.iter().filter(|c| wanted.contains(&c.oid) && !excluded.contains(&c.oid)).collect()
    }

    pub fn is_dirty(&self) -> bool {
        !self.unstaged.is_empty() || !self.staged.is_empty()
    }

    pub fn head_info(&self) -> Option<&BranchInfo> {
        self.locals.iter().find(|b| b.is_head)
    }

    /// Whether status says `path` is gone from the working tree: deleted there (unstaged 'D'),
    /// or deleted in the index ('D' staged) with nothing at the path since.
    pub fn missing_from_workdir(&self, path: &str) -> bool {
        if let Some(u) = self.unstaged.iter().find(|f| f.path == path) {
            return u.status == 'D';
        }
        self.staged.iter().any(|f| f.path == path && f.status == 'D')
    }
}

pub struct CommitDetail {
    pub oid: Oid,
    pub message: String,
    pub author: String,
    pub email: String,
    pub committer: String,
    pub time: i64,
    pub parents: Vec<Oid>,
    pub files: Vec<FileEntry>,
    /// Paths of `files` missing from the working tree when the detail was loaded (deleted,
    /// renamed or never checked out). Views use it to disable Open / Show in folder.
    pub absent: HashSet<String>,
}

impl CommitDetail {
    /// Whether `path` was in the working tree when the detail was loaded.
    pub fn in_workdir(&self, path: &str) -> bool {
        !self.absent.contains(path)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum DiffTarget {
    Commit(Oid, String),
    Unstaged(String),
    Staged(String),
    /// `path` as it differs from commit `.0` (older) to commit `.1` (newer), tree to tree with
    /// renames detected.
    Between(Oid, Oid, String),
}

impl DiffTarget {
    pub fn path(&self) -> &str {
        match self {
            DiffTarget::Commit(_, p) | DiffTarget::Unstaged(p) | DiffTarget::Staged(p) | DiffTarget::Between(_, _, p) => p,
        }
    }

    /// A diff of the working tree or index. Only these targets support hunk and line operations.
    pub fn is_working_tree(&self) -> bool {
        matches!(self, DiffTarget::Unstaged(_) | DiffTarget::Staged(_))
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
    /// Filled in by `highlight::highlight_diff`; `None` means plain.
    pub highlight: Option<crate::highlight::DiffHighlight>,
    /// Filled in by `intraline::intraline`; empty means no word marks.
    pub intraline: crate::intraline::Intraline,
    /// With LoadDiffOptions::whole_file: every line of the file and its change blocks. None
    /// otherwise, or when it couldn't be mapped onto the hunks.
    pub whole: Option<Box<crate::whole_file::WholeFile>>,
}

/// How `load_diff_with` builds a file diff.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoadDiffOptions {
    /// Hide changes in the amount of whitespace and at line ends (git `-b --ignore-space-at-eol`).
    /// Hunk indexes then no longer match `apply_hunk`, `apply_lines` or `revert_hunk`.
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

/// Which history the graph shows.
#[derive(Clone, Debug, Default)]
pub struct LoadOptions {
    pub max_commits: usize,
    /// Only the checked-out branch, its upstream, the default branch(es) and `extra_tips`.
    pub smart: bool,
    /// Branch names (local or remote, e.g. PR source branches) kept visible in smart mode.
    pub extra_tips: Vec<String>,
    /// Full ref names to leave out of the graph.
    pub hidden: Vec<String>,
    /// When set, show only these full ref names (plus HEAD).
    pub solo: Vec<String>,
}

impl LoadOptions {
    pub fn all(max_commits: usize) -> Self {
        Self { max_commits, ..Self::default() }
    }
}

#[cfg(test)]
mod compare_tests;
#[cfg(test)]
mod conflict_tests;
#[cfg(test)]
mod diff_tests;
#[cfg(test)]
mod history_tests;
#[cfg(test)]
mod lines_tests;
#[cfg(test)]
mod load_filter_tests;
#[cfg(test)]
mod resolve_tests;
#[cfg(test)]
mod restore_tests;
#[cfg(test)]
mod revert_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod whole_tests;
