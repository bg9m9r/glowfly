//! Other worktrees' uncommitted changes, for their WIP rows in the graph. A full load counts
//! only the small ones, within a time budget, and leaves the rest Pending, so one huge linked
//! worktree doesn't hold up the graph of every tab. The app then counts the Pending ones in the
//! background, one worktree at a time, through git when it may run there.

use super::status::ChangeCounts;
use super::status_speed::SlowStatus;
use super::worktrees::WorktreeInfo;
use git2::Repository;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How a worktree's `changes` were read.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ChangesRead {
    /// Not read: the current worktree, a missing one, with the option off, or from `worktrees_at`.
    #[default]
    NotCounted,
    /// `changes` holds its counts.
    Counted,
    /// Left for a background count after the load.
    Pending,
    /// Too large to count in the background; open it in a tab to see its changes.
    TooLarge,
}

/// Indexes with more entries than this are left for the background count.
pub const LOAD_COUNT_INDEX: u32 = 20_000;
/// How long one load spends counting other worktrees before leaving the rest for later.
pub const LOAD_COUNT_BUDGET: Duration = Duration::from_millis(500);
/// How long git may take to read one worktree's status in the background.
pub const COUNT_TIMEOUT: Duration = Duration::from_secs(20);
/// Past this many index entries a worktree where git may not run isn't counted at all: a
/// libgit2 read can't be stopped once started.
pub const NO_GIT_COUNT_INDEX: u32 = 200_000;

/// What a load may spend on counting other worktrees.
pub(super) struct LoadLimits {
    pub max_entries: u32,
    pub budget: Duration,
}

impl LoadLimits {
    pub const DEFAULT: Self = Self { max_entries: LOAD_COUNT_INDEX, budget: LOAD_COUNT_BUDGET };
}

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

/// Counts the uncommitted changes of every listed worktree other than the current one and the
/// missing ones, while they are small and the budget lasts; the rest are left Pending. Read by
/// libgit2 alone, so no hook, fsmonitor or filter runs: safe while the repository isn't trusted.
/// One that can't be opened or read counts as clean.
pub(super) fn count_in_load(list: &mut [WorktreeInfo], limits: &LoadLimits) {
    let started = Instant::now();
    for wt in list.iter_mut().filter(|w| !w.current && !w.missing) {
        let repo = Repository::open(&wt.path).ok();
        let entries = repo.as_ref().and_then(|r| index_entries(&r.path().join("index"))).unwrap_or(0);
        if entries > limits.max_entries || started.elapsed() >= limits.budget {
            wt.read = ChangesRead::Pending;
            wt.changes = ChangeCounts::default();
            continue;
        }
        let lists = repo.and_then(|r| super::status::libgit2_status(&r).ok());
        wt.changes = lists.map(|(unstaged, staged)| ChangeCounts::of(&staged, &unstaged)).unwrap_or_default();
        wt.read = ChangesRead::Counted;
    }
}

/// The result of counting one worktree in the background.
#[derive(Clone, Debug, PartialEq)]
pub struct WorktreeCount {
    pub path: PathBuf,
    /// Counted or TooLarge.
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

/// Counts the worktree at `path`. Git reads it (within `timeout`) only when the repository is
/// `trusted` or nothing in it would run a program, as for a load's own status; otherwise
/// libgit2 does, up to `NO_GIT_COUNT_INDEX` entries. One that can't be opened counts as clean.
pub fn count_worktree(path: &Path, trusted: bool, timeout: Duration) -> WorktreeCount {
    let done = |read, changes, slow| WorktreeCount { path: path.to_path_buf(), read, changes, slow };
    let Ok(repo) = Repository::open(path) else { return done(ChangesRead::Counted, ChangeCounts::default(), None) };
    let entries = index_entries(&repo.path().join("index")).unwrap_or(0);
    let git_ok = trusted || crate::trust::scan(&repo).is_empty();
    // From the config alone: `status::wants_git` would read the whole index to count it.
    let wants_git = repo.config().map(|c| super::status_speed::speedups_of(&c)).is_ok_and(|s| s.fsmonitor || s.untracked_cache);
    let libgit2 = |repo: &Repository| {
        let lists = super::status::libgit2_status(repo).ok();
        done(ChangesRead::Counted, lists.map(|(u, s)| ChangeCounts::of(&s, &u)).unwrap_or_default(), None)
    };
    match plan(entries, git_ok, wants_git) {
        Plan::TooLarge => done(ChangesRead::TooLarge, ChangeCounts::default(), None),
        Plan::Libgit2 => libgit2(&repo),
        Plan::Git => {
            let workdir = repo.workdir().map(Path::to_path_buf).unwrap_or_else(|| path.to_path_buf());
            let started = Instant::now();
            match super::status_cli::status_within(&workdir, timeout) {
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
