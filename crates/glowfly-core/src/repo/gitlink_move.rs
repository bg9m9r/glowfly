//! What a change to a submodule's gitlink (mode 160000) means, for the diff's card: which way
//! the recorded commit moved (forward, back, to another line of history), the commits it passes
//! over on each side, read from the submodule's own repository, and whether it has uncommitted
//! changes inside. The submodule is opened only where `submodule_inside::open_at` allows; one
//! that can't be read gives an `Unknown` move, never a failed diff.

use super::submodule_inside::{Opened, open_at};
use super::{DiffTarget, rpath};
use crate::commit_text;
use git2::{Diff, DiffDelta, DiffFile, FileMode, Oid, Repository, Sort, SubmoduleIgnore, SubmoduleStatus};
use std::path::Path;

/// The most commits a side of a move lists (the ones nearest its end).
pub const GITLINK_LIST_MAX: usize = 50;
/// The most commits a side of a move counts.
pub const GITLINK_COUNT_MAX: usize = 10_000;

/// The commits on one side of a move.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommitRun {
    /// (id, summary), oldest first: the `GITLINK_LIST_MAX` nearest the end of the move.
    pub commits: Vec<(Oid, String)>,
    /// How many there are, counted up to `GITLINK_COUNT_MAX`.
    pub total: usize,
    /// The count stopped at `GITLINK_COUNT_MAX`: there are more.
    pub capped: bool,
}

/// Why a move's commits can't be listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unknown {
    /// The submodule isn't checked out here (or its repository can't be read).
    NotCheckedOut,
    /// Its repository names a place Glowfly doesn't follow (another network place).
    Refused,
    /// The submodule's repository lacks the old or the new commit (not fetched there).
    Missing { old: bool, new: bool },
    /// A shallow history where no common commit was found.
    Shallow,
}

/// Which way the recorded commit moved.
#[derive(Clone, Debug, PartialEq)]
pub enum MoveKind {
    Added,
    Removed,
    /// The same commit on both sides: only the changes inside differ.
    Same,
    /// The new commit contains the old one.
    Forward,
    /// The old commit contains the new one.
    Backward,
    /// Neither contains the other: `base` is where they part, None for unrelated histories.
    Sideways {
        base: Option<(Oid, String)>,
    },
    Unknown(Unknown),
}

/// A gitlink's change in a diff.
#[derive(Clone, Debug, PartialEq)]
pub struct GitlinkMove {
    /// Repo-relative, with '/': the new path, or the old one for a deletion.
    pub path: String,
    pub old: Option<Oid>,
    pub new: Option<Oid>,
    pub kind: MoveKind,
    /// Commits only the new side has (Forward and Sideways).
    pub ahead: CommitRun,
    /// Commits only the old side has (Backward and Sideways).
    pub behind: CommitRun,
    /// The working tree is on one side and the submodule has uncommitted changes or untracked
    /// files, which the parent doesn't record.
    pub dirty: bool,
    /// One side is a gitlink and the other a file or a link: the diff's rows still show.
    pub type_change: bool,
}

fn is_link(f: &DiffFile<'_>) -> bool {
    f.mode() == FileMode::Commit
}

/// A side's commit: None when that side isn't a gitlink or has no id.
fn side(f: &DiffFile<'_>) -> Option<Oid> {
    (is_link(f) && !f.id().is_zero()).then(|| f.id())
}

/// A side that holds something other than a gitlink (a file, a link or a folder).
fn other(f: &DiffFile<'_>) -> bool {
    !is_link(f) && f.mode() != FileMode::Unreadable
}

/// The gitlink move of the shown path's delta in `diff`, when either side is a gitlink. A type
/// change libgit2 splits into a deletion and an addition is read from the pair. `repo` is the
/// parent's repository, `workdir` its working tree.
pub(super) fn of_diff(repo: &Repository, workdir: &Path, target: &DiffTarget, diff: &Diff<'_>) -> Option<GitlinkMove> {
    let path = target.path();
    let at: Vec<DiffDelta<'_>> = diff.deltas().filter(|d| rpath::touches(d, path)).collect();
    let delta = at.iter().find(|d| is_link(&d.old_file()) || is_link(&d.new_file()))?;
    let (o, n) = (delta.old_file(), delta.new_file());
    let type_change = other(&o) || other(&n) || at.iter().any(|d| other(&d.old_file()) || other(&d.new_file()));
    let shown = String::from_utf8_lossy(rpath::delta_bytes(delta)).into_owned();
    let mut m = between(workdir, &shown, side(&o), side(&n));
    m.type_change = type_change;
    let workdir_side = matches!(target, DiffTarget::Unstaged(_) | DiffTarget::ToWorkdir(..) | DiffTarget::FromWorkdir(..));
    if workdir_side && matches!(open_at(workdir, &shown), Opened::Repo(_)) {
        let inside = SubmoduleStatus::WD_INDEX_MODIFIED | SubmoduleStatus::WD_WD_MODIFIED | SubmoduleStatus::WD_UNTRACKED;
        m.dirty = repo.submodule_status(&shown, SubmoduleIgnore::None).is_ok_and(|s| s.intersects(inside));
    }
    Some(m)
}

/// How the submodule at `path` (in the parent working tree `workdir`) moved from `old` to
/// `new`, with the commits on each side read from its repository. Never fails: what can't be
/// read is an `Unknown` move.
pub(crate) fn between(workdir: &Path, path: &str, old: Option<Oid>, new: Option<Oid>) -> GitlinkMove {
    let mut m = GitlinkMove {
        path: path.to_owned(),
        old,
        new,
        kind: MoveKind::Same,
        ahead: CommitRun::default(),
        behind: CommitRun::default(),
        dirty: false,
        type_change: false,
    };
    let (o, n) = match (old, new) {
        (None, Some(_)) | (None, None) => return MoveKind::Added.set(m),
        (Some(_), None) => return MoveKind::Removed.set(m),
        (Some(o), Some(n)) if o == n => return m,
        (Some(o), Some(n)) => (o, n),
    };
    let repo = match open_at(workdir, path) {
        Opened::Repo(r) => r,
        Opened::NotCheckedOut => return MoveKind::Unknown(Unknown::NotCheckedOut).set(m),
        Opened::Refused => return MoveKind::Unknown(Unknown::Refused).set(m),
    };
    let (has_old, has_new) = (repo.find_commit(o).is_ok(), repo.find_commit(n).is_ok());
    if !has_old || !has_new {
        return MoveKind::Unknown(Unknown::Missing { old: !has_old, new: !has_new }).set(m);
    }
    match walk(&repo, o, n) {
        Ok((kind, ahead, behind)) => {
            m.ahead = ahead;
            m.behind = behind;
            kind.set(m)
        }
        Err(_) => MoveKind::Unknown(Unknown::NotCheckedOut).set(m),
    }
}

impl MoveKind {
    fn set(self, mut m: GitlinkMove) -> GitlinkMove {
        m.kind = self;
        m
    }
}

/// The direction of the move from `o` to `n` (both present in `repo`), and the commits only
/// `n` has and only `o` has.
fn walk(repo: &Repository, o: Oid, n: Oid) -> Result<(MoveKind, CommitRun, CommitRun), git2::Error> {
    if repo.graph_descendant_of(n, o)? {
        return Ok((MoveKind::Forward, run(repo, n, o)?, CommitRun::default()));
    }
    if repo.graph_descendant_of(o, n)? {
        return Ok((MoveKind::Backward, CommitRun::default(), run(repo, o, n)?));
    }
    let base = match repo.merge_base(o, n) {
        Ok(b) => Some((b, commit_text::summary(&repo.find_commit(b)?).into_owned())),
        Err(e) if e.code() == git2::ErrorCode::NotFound => None,
        Err(e) => return Err(e),
    };
    if base.is_none() && repo.is_shallow() {
        return Ok((MoveKind::Unknown(Unknown::Shallow), CommitRun::default(), CommitRun::default()));
    }
    Ok((MoveKind::Sideways { base }, run(repo, n, o)?, run(repo, o, n)?))
}

/// The commits `push` reaches and `hide` doesn't: counted up to `GITLINK_COUNT_MAX`, the newest
/// `GITLINK_LIST_MAX` kept, oldest first.
fn run(repo: &Repository, push: Oid, hide: Oid) -> Result<CommitRun, git2::Error> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
    walk.push(push)?;
    walk.hide(hide)?;
    let mut kept = Vec::new();
    let (mut total, mut capped) = (0, false);
    for id in walk {
        let id = id?;
        if total == GITLINK_COUNT_MAX {
            capped = true;
            break;
        }
        total += 1;
        if kept.len() < GITLINK_LIST_MAX {
            kept.push(id);
        }
    }
    kept.reverse();
    let commits = kept
        .into_iter()
        .map(|id| Ok((id, commit_text::summary(&repo.find_commit(id)?).into_owned())))
        .collect::<Result<Vec<_>, git2::Error>>()?;
    Ok(CommitRun { commits, total, capped })
}

#[cfg(test)]
#[path = "gitlink_move_tests.rs"]
mod tests;
