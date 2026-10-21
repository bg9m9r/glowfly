//! What a merge, rebase, reset, cherry-pick or drop would leave, worked out before it runs:
//! the outcome (nothing, a fast-forward, done, or where it stops), the branch's history
//! afterwards as one first-parent lane, and the commits no branch or tag would reach any more.
//!
//! Nothing changes on disk. The replays (`replay.rs`) write their trees and commits into a
//! mempack backend added to the repository's object database with the highest priority, so
//! every new object stays in memory and is freed with the `Repository`; no ref, index or
//! working file is ever written. A real merge is predicted with `git merge-tree` through
//! [`crate::conflict_check::predict`], so the user's merge config and drivers apply there.
//! `ops.rs` holds one function per operation and `shape.rs` the rows they share.

use crate::error::{Error, Result};
use crate::journal::preview::MAX_COUNT;
use git2::{Oid, Repository};
use std::path::Path;

pub use crate::journal::{CommitLine, Count};

mod ops;
mod replay;
mod shape;

#[cfg(test)]
mod drop_tests;
#[cfg(test)]
mod merge_tests;
#[cfg(test)]
mod pick_tests;
#[cfg(test)]
mod rebase_tests;
#[cfg(test)]
mod reset_tests;
#[cfg(test)]
mod tests;

/// The operation to preview, as the menus would run it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreviewOp {
    /// `git merge <theirs>`: `theirs` is the name git is given ("feature", "origin/x"), `oid`
    /// the commit it named when the preview was asked for.
    Merge { theirs: String, oid: Oid },
    /// `git rebase <onto>`: a branch name or a full sha, and the commit it named.
    Rebase { onto: String, oid: Oid },
    /// `git reset <to>` (soft, mixed and hard move the branch the same way).
    Reset { to: Oid },
    /// `git cherry-pick`, oldest first.
    CherryPick(Vec<Oid>),
    /// Dropping these commits from the checked-out branch (`QuickEdit::Drop`).
    Drop(Vec<Oid>),
}

/// A preview to work out: the operation and the HEAD commit the app saw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub op: PreviewOp,
    pub head: Oid,
}

/// What the operation would leave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpPreview {
    pub op: PreviewOp,
    pub head: Oid,
    /// The checked-out branch; None when HEAD is detached.
    pub subject: Option<String>,
    pub outcome: Outcome,
    /// The branch's first-parent history afterwards (or at the stop), newest first.
    pub after: Vec<AfterRow>,
    /// More history follows `after`.
    pub after_more: bool,
    /// The old versions of the commits a rebase or drop rewrites, oldest first.
    pub moved: Vec<CommitLine>,
    /// Commits no branch, tag or worktree would reach afterwards (at most `Limits::rows`).
    pub gone: Vec<Gone>,
    pub gone_count: Count,
    /// How many commits come into the branch.
    pub arriving: Option<Count>,
    /// The rebase didn't look for commits already on the target: too many to compare.
    pub patch_check_skipped: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The branch stays where it is.
    Nothing,
    /// The branch moves forward to the target.
    FastForward,
    /// It runs to the end.
    Done,
    /// It stops for the user.
    Stopped(Stop),
}

/// Where and why an operation stops.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stop {
    pub why: StopWhy,
    /// The commits applied before the stop.
    pub applied: usize,
    /// The commits after it, oldest first.
    pub waiting: Vec<CommitLine>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StopWhy {
    /// Conflicts in these paths (index order, each once).
    Conflicts(Vec<String>),
    /// The commit's changes are already there: it would be an empty commit.
    Empty,
}

/// One row of the history afterwards.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AfterRow {
    /// A commit that stays as it is; `arrives` when the operation brings it into the branch.
    Kept { line: CommitLine, arrives: bool },
    /// A new version of `from` (a rebase or drop), or its copy (a cherry-pick).
    Rewritten { from: CommitLine, copy: bool },
    /// The merge commit a merge makes.
    NewMerge { theirs: String },
    /// Where it stops: at this commit, or (None) at a merge's own commit.
    Stopped { at: Option<CommitLine> },
}

/// A commit that would become unreachable, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gone {
    pub line: CommitLine,
    pub why: GoneWhy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoneWhy {
    Dropped,
    /// Its change is already on the target (a rebase skips it).
    AlreadyApplied,
    /// A merge a rebase leaves out (the result is linear).
    MergeLeftOut,
    /// Only the old tip reached it (a reset).
    LeftBehind,
}

/// How much a preview walks and shows.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    /// Rows of the history afterwards and of the gone list.
    pub rows: usize,
    /// The most commits replayed.
    pub replay: usize,
    /// The most target commits compared by patch for a rebase.
    pub patch: usize,
    /// The most commits a count walks.
    pub count: usize,
}

impl Limits {
    pub const DEFAULT: Limits = Limits { rows: 12, replay: 500, patch: 1000, count: MAX_COUNT };
}

pub const STALE: &str = "The branch changed since you asked for this preview. Preview again.";
pub const PICK_MERGE: &str = "Preview doesn't cover cherry-picking a merge commit yet.";
pub const REBASE_MERGES: &str = "rebase.rebaseMerges is set, and the preview doesn't cover recreating merges yet.";
pub const OCTOPUS: &str = "The preview doesn't cover merges with more than two parents yet.";
pub const STEPS: &str = "The preview doesn't cover exec or break steps.";
pub const FF_ONLY: &str = "Git will refuse this merge: fast-forward only merges are set (merge.ff or the branch's merge options), and the branches have diverged.";
pub const UNRELATED: &str = "Git will refuse this merge: the two branches share no history.";
pub const UNBORN: &str = "The branch has no commits yet.";

/// Why a preview of a merge or rebase onto `name` is out of date.
pub fn ref_moved(name: &str) -> String {
    format!("{name} moved since you asked for this preview. Preview again.")
}

/// Why a replay of `n` commits isn't previewed.
pub fn too_many(n: usize) -> String {
    format!("The preview covers up to 500 commits; this would replay {n}.")
}

/// What `req` would do in the repository at `workdir`. Refused (`Error::Unsupported`, with a
/// message for the user) while an operation is in progress, on an unborn HEAD, when HEAD or the
/// named ref moved since the request, and for the cases the preview doesn't cover.
pub fn preview(workdir: &Path, req: &Request) -> Result<OpPreview> {
    preview_with(workdir, req, &Limits::DEFAULT)
}

pub(crate) fn preview_with(workdir: &Path, req: &Request, lim: &Limits) -> Result<OpPreview> {
    let repo = Repository::open(workdir)?;
    if let Some(text) = crate::rebase::in_progress_text(repo.state()) {
        return Err(Error::Unsupported(text));
    }
    let head = repo.head().ok().and_then(|h| h.target()).ok_or_else(|| Error::Unsupported(UNBORN.into()))?;
    if repo.find_commit(head).is_err() {
        return Err(Error::Unsupported(UNBORN.into()));
    }
    if head != req.head {
        return Err(Error::Unsupported(STALE.into()));
    }
    // Every object written from here on goes to memory only and is freed with `repo`.
    let odb = repo.odb()?;
    let _mem = odb.add_new_mempack_backend(1000)?;
    let cx = ops::Cx { repo: &repo, workdir, head, subject: subject(&repo), lim };
    match &req.op {
        PreviewOp::Merge { theirs, oid } => ops::merge(&cx, theirs, *oid),
        PreviewOp::Rebase { onto, oid } => ops::rebase(&cx, onto, *oid),
        PreviewOp::Reset { to } => ops::reset(&cx, *to),
        PreviewOp::CherryPick(oids) => ops::cherry_pick(&cx, oids),
        PreviewOp::Drop(oids) => ops::drop(&cx, oids),
    }
    .map(|mut p| {
        p.op = req.op.clone();
        p
    })
}

/// The checked-out branch's name, or None when HEAD is detached.
fn subject(repo: &Repository) -> Option<String> {
    if repo.head_detached().unwrap_or(true) {
        return None;
    }
    repo.head().ok()?.shorthand().map(str::to_owned)
}
