//! Other worktrees' uncommitted changes, for their WIP rows in the graph. A full load reads none
//! of them: it marks each other worktree it may open Pending, or Carried with the counts the
//! snapshot before it had for the same folder at the same HEAD, so their rows stay put through a
//! reload; the app then counts those in the background, one count per tab at a time, through git
//! when it may run there.

use super::status::ChangeCounts;
use super::status_speed::SlowStatus;
use super::worktrees::WorktreeInfo;
use crate::settings::{RepoTrust, SavedTrust};
use git2::Repository;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How a worktree's `changes` were read.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ChangesRead {
    /// Not read, and never asked for by a recount: the current worktree, a missing one, with
    /// the option off, from `worktrees_at`, or one whose folder or git files are on another
    /// network place than the repository it is listed from.
    #[default]
    NotCounted,
    /// `changes` holds its counts.
    Counted,
    /// Not read yet: left for the background count right after the load.
    Pending,
    /// `changes` holds the counts the snapshot before this load had for the same folder at the
    /// same HEAD (`Snapshot::prior`); shown as counted, and counted again in the background right
    /// after the load.
    Carried,
    /// Too large to count in the background; open it in a tab to see its changes.
    TooLarge,
    /// A count couldn't read it for now: the worktree's own files held the read
    /// (`tree_files::check`), or its folder isn't a repository at the moment. Asked again with
    /// the counted ones, so it is counted once it can be read.
    Held,
}

/// How long git may take to read one worktree's status in the background.
pub const COUNT_TIMEOUT: Duration = Duration::from_secs(20);
/// Past this many index entries a worktree where git may not run isn't counted at all: a
/// libgit2 read can't be stopped once started.
pub const NO_GIT_COUNT_INDEX: u32 = 200_000;

/// The entry count in the header of the index file at `index` ("DIRC", a version, then a
/// big-endian count). None when it is missing or not an index. Reads only the header.
pub(crate) fn index_entries(index: &Path) -> Option<u32> {
    let mut head = [0u8; 12];
    std::fs::File::open(index).ok()?.read_exact(&mut head).ok()?;
    if &head[..4] != b"DIRC" {
        return None;
    }
    Some(u32::from_be_bytes([head[8], head[9], head[10], head[11]]))
}

/// Another worktree's counts as the snapshot on screen holds them, for the next load to show
/// until its background count lands (`Snapshot::prior`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct KnownCount {
    path: PathBuf,
    head: Option<git2::Oid>,
    changes: ChangeCounts,
}

/// What a later load may carry over from `list`: other worktrees, not missing, whose read is
/// Counted, Held or Carried.
pub(crate) fn known_counts(list: &[WorktreeInfo]) -> Vec<KnownCount> {
    list.iter()
        .filter(|w| !w.current && !w.missing && matches!(w.read, ChangesRead::Counted | ChangesRead::Held | ChangesRead::Carried))
        .map(|w| KnownCount { path: w.path.clone(), head: w.head, changes: w.changes })
        .collect()
}

/// Marks every listed worktree but the current and the missing ones for the background count,
/// reading none of them: Carried with its `known` counts when the snapshot before had them for
/// the same folder (`paths::same_spelling`) at the same HEAD, else Pending with none. One whose
/// folder, or a file libgit2 would follow in opening it (its `.git` file's `gitdir:`,
/// `commondir`, config), is on another network place than `home` (the common dir) stays
/// NotCounted, as `count_worktree` leaves it.
pub(super) fn leave_for_later(list: &mut [WorktreeInfo], home: &Path, known: &[KnownCount]) {
    for wt in list.iter_mut().filter(|w| !w.current && !w.missing && openable(&w.path, home)) {
        let same = known.iter().find(|k| k.head == wt.head && crate::paths::same_spelling(&k.path, &wt.path));
        (wt.read, wt.changes) = match same {
            Some(k) => (ChangesRead::Carried, k.changes),
            None => (ChangesRead::Pending, ChangeCounts::default()),
        };
    }
}

/// What a libgit2 status read (`lists`; None for a worktree that can't be opened) counts as: its
/// counts, clean when it failed, and not counted when the worktree's own `.gitattributes` or
/// `.gitignore` held the read (`tree_files::check`).
fn counted(lists: Option<crate::error::Result<(Vec<super::FileEntry>, Vec<super::FileEntry>)>>) -> (ChangesRead, ChangeCounts) {
    match lists {
        Some(Err(crate::error::Error::LinkedFile(_))) => (ChangesRead::Held, ChangeCounts::default()),
        Some(Ok((unstaged, staged))) => (ChangesRead::Counted, ChangeCounts::of(&staged, &unstaged)),
        _ => (ChangesRead::Counted, ChangeCounts::default()),
    }
}

/// Whether libgit2 may open the worktree at `path`: neither it nor what its files name is on
/// another network place than `home` (`git_file::other_repo`).
fn openable(path: &Path, home: &Path) -> bool {
    crate::net_path::may_follow(path, home) && matches!(crate::git_file::other_repo(path, home), crate::git_file::Found::Repo(_))
}

/// The result of counting one worktree in the background.
#[derive(Clone, Debug, PartialEq)]
pub struct WorktreeCount {
    pub path: PathBuf,
    /// Counted, TooLarge, or NotCounted when the worktree's own `.gitattributes` or `.gitignore`
    /// held the read.
    pub read: ChangesRead,
    pub changes: ChangeCounts,
    /// Some when git's read was slow enough to suggest a speed-up.
    pub slow: Option<SlowStatus>,
}

/// How a background count reads a worktree.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Plan {
    Git,
    Libgit2,
    TooLarge,
}

/// How to count a worktree of `entries` index entries, when git may run there (`git_ok`) and
/// its config speeds git's read up (`wants_git`).
fn plan(entries: u32, git_ok: bool, wants_git: bool) -> Plan {
    if git_ok && (wants_git || entries as usize > super::status::LARGE_INDEX) {
        Plan::Git
    } else if entries > NO_GIT_COUNT_INDEX {
        Plan::TooLarge
    } else {
        Plan::Libgit2
    }
}

/// Counts the worktree at `path`. Git reads it (within `timeout`) only when the worktree's own
/// saved `decision` trusts it and its print covers what the worktree runs now, or when there is
/// none and nothing in it would run a program, as opening it in a tab would allow
/// (`trust::allows`); otherwise libgit2 does, up to `NO_GIT_COUNT_INDEX` entries. One that
/// libgit2 can't open counts as clean, and one whose folder holds no repository for now is
/// Held (counted by a later recount). One whose folder, or a file libgit2 would follow in
/// opening it, is on another network place than `home` (the work tree it is listed from) is
/// never opened and is NotCounted, as the load leaves it.
pub fn count_worktree(path: &Path, home: &Path, decision: Option<&SavedTrust>, timeout: Duration) -> WorktreeCount {
    let done = |read, changes, slow| WorktreeCount { path: path.to_path_buf(), read, changes, slow };
    if !crate::net_path::may_follow(path, home) {
        return done(ChangesRead::NotCounted, ChangeCounts::default(), None);
    }
    match crate::git_file::other_repo(path, home) {
        crate::git_file::Found::Repo(_) => {}
        crate::git_file::Found::Refused(_) => return done(ChangesRead::NotCounted, ChangeCounts::default(), None),
        // No repository there for now (being made again, say): asked for again.
        crate::git_file::Found::None => return done(ChangesRead::Held, ChangeCounts::default(), None),
    }
    let Some(repo) = Repository::open(path).ok() else { return done(ChangesRead::Counted, ChangeCounts::default(), None) };
    let entries = index_entries(&repo.path().join("index")).unwrap_or(0);
    let git_ok = match decision {
        Some(d) if d.trust == RepoTrust::NotTrusted => false,
        d => crate::trust::allows(d, &crate::trust::scan(&repo)),
    };
    // From the config alone: `status::wants_git` would read the whole index to count it.
    let wants_git = repo.config().map(|c| super::status_speed::speedups_of(&c)).is_ok_and(|s| s.fsmonitor || s.untracked_cache);
    let libgit2 = |repo: &Repository| {
        let (read, changes) = counted(Some(super::status::libgit2_status(repo)));
        done(read, changes, None)
    };
    match plan(entries, git_ok, wants_git) {
        Plan::TooLarge => done(ChangesRead::TooLarge, ChangeCounts::default(), None),
        Plan::Libgit2 => libgit2(&repo),
        Plan::Git => {
            let workdir = repo.workdir().map(Path::to_path_buf).unwrap_or_else(|| path.to_path_buf());
            let started = Instant::now();
            match super::status_cli::status_within(&repo, &workdir, timeout) {
                Ok((unstaged, staged)) => {
                    let slow = super::status_speed::slow(started.elapsed(), &workdir);
                    done(ChangesRead::Counted, ChangeCounts::of(&staged, &unstaged), slow)
                }
                Err(false) if entries <= NO_GIT_COUNT_INDEX => libgit2(&repo),
                Err(_) => done(ChangesRead::TooLarge, ChangeCounts::default(), None),
            }
        }
    }
}

#[cfg(test)]
#[path = "worktree_counts_tests.rs"]
mod tests;
