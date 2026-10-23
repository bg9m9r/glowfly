//! A small pool of open repositories for the worker threads, so the reads that run on every
//! watcher burst and selection (refs fingerprint, status, path status, commit detail, diffs,
//! line counts) don't open the repository, load its config and parse its index each time.
//!
//! A handle is lent (`lend`) to one thread at a time (`git2::Repository` is `Send`, not `Sync`)
//! and comes back to the pool when the `Pooled` lease drops. Handles are keyed by the path as
//! given, never folded, and by how they were opened (`Use`). Before one is lent again it is
//! checked (`fresh.rs`): its config files, `.git` link, git dir and alternates must be as they
//! were when it was opened, and a `Use::Workdir` handle's index is read again when the file
//! changed (libgit2 does so itself only for some calls, not for `Repository::index`).
//!
//! Idle handles go after `IDLE`, the oldest first past `MAX_IDLE` (and `MAX_PER_KEY` per key),
//! and all of a repository's when `forget` is called (a tab closed) or while a `hold` lasts
//! (an effect that changes the repository runs): on Windows an open handle keeps its pack files
//! from being deleted (`git gc`, `git repack`) and its folders from being removed.

use crate::paths::{FolderId, same_spelling};
use git2::Repository;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

mod fresh;
#[cfg(test)]
mod tests;

/// How long an unused handle is kept. Short, for the pack files it keeps open (module docs);
/// long in tests, which check what is kept without a clock.
pub const IDLE: Duration = if cfg!(test) { Duration::from_secs(60) } else { Duration::from_secs(5) };
/// The most idle handles kept, over all repositories. Tests share one pool across threads, so
/// they get more room to keep their own handles.
const MAX_IDLE: usize = if cfg!(test) { 64 } else { 8 };
/// The most idle handles kept for one key (reads of one repository can overlap).
const MAX_PER_KEY: usize = 2;

/// What a handle is for, which decides how it is opened and kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    /// `Repository::open`, its index (once read, by the caller or by libgit2 for attributes)
    /// kept as the file on disk is.
    Workdir,
    /// `Repository::discover` (the path may be inside the repository), for refs, config and
    /// files only: its index is never brought up to date, so never read it through one.
    Refs,
}

/// The repository a handle belongs to, for `forget` and `hold`: its common dir, by identity
/// when the volume gives one (`docs/PLATFORMS.md`), else by spelling.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Group {
    Folder(FolderId),
    Spelling(PathBuf),
}

impl Group {
    fn of(repo: &Repository) -> Group {
        let common = repo.commondir();
        FolderId::of(common).map(Group::Folder).unwrap_or_else(|| Group::Spelling(common.to_owned()))
    }

    fn same(&self, other: &Group) -> bool {
        match (self, other) {
            (Group::Folder(a), Group::Folder(b)) => a == b,
            (Group::Spelling(a), Group::Spelling(b)) => same_spelling(a, b),
            _ => false,
        }
    }
}

/// What `forget` and `hold` name: the handles opened at `path`, and those of its repository
/// (`group`, None when `path` doesn't open).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Target {
    path: PathBuf,
    group: Option<Group>,
}

impl Target {
    fn of(path: &Path) -> Target {
        Target { path: path.to_owned(), group: Repository::discover(path).ok().map(|r| Group::of(&r)) }
    }

    fn covers(&self, key: &Path, group: &Group) -> bool {
        same_spelling(&self.path, key) || self.group.as_ref().is_some_and(|g| g.same(group))
    }
}

/// What the pool knows about a handle besides the handle itself.
struct Slot {
    key: PathBuf,
    how: Use,
    group: Group,
    fresh: fresh::Fresh,
}

struct Idle {
    slot: Slot,
    repo: Repository,
    since: Instant,
}

struct Pool {
    idle: Vec<Idle>,
    /// The repositories being changed (`hold`), one entry per hold.
    held: Vec<Target>,
    /// The handles lent now: id, key, group, and whether a `forget` or `hold` since means it
    /// is dropped when it comes back.
    lent: Vec<(u64, PathBuf, Group, bool)>,
    next_id: u64,
    /// The thread that drops expired handles is running.
    reaper: bool,
    /// How many times each key was opened (tests count reuse with it).
    #[cfg(test)]
    opened: Vec<(PathBuf, Use, usize)>,
}

static POOL: Mutex<Pool> = Mutex::new(Pool {
    idle: Vec::new(),
    held: Vec::new(),
    lent: Vec::new(),
    next_id: 0,
    reaper: false,
    #[cfg(test)]
    opened: Vec::new(),
});

fn pool() -> MutexGuard<'static, Pool> {
    POOL.lock().unwrap_or_else(|e| e.into_inner())
}

/// A repository lent from the pool; it goes back when dropped.
pub struct Pooled {
    repo: Option<Repository>,
    slot: Option<Slot>,
    id: u64,
}

impl Deref for Pooled {
    type Target = Repository;
    fn deref(&self) -> &Repository {
        self.repo.as_ref().expect("a lent repository is held until dropped")
    }
}

impl Drop for Pooled {
    fn drop(&mut self) {
        if let (Some(repo), Some(slot)) = (self.repo.take(), self.slot.take()) {
            check_in(repo, slot, self.id);
        }
    }
}

/// The repository at `path` for `how`: an idle handle that is still fresh, else a new one.
pub fn lend(path: &Path, how: Use) -> Result<Pooled, git2::Error> {
    let found = {
        let mut p = pool();
        let now = Instant::now();
        let at = p.idle.iter().rposition(|i| i.slot.how == how && i.slot.key == path && now - i.since < IDLE);
        at.map(|k| p.idle.remove(k))
    };
    if let Some(Idle { mut slot, repo, .. }) = found
        && slot.fresh.still(&repo, how == Use::Workdir)
    {
        return Ok(lent(repo, slot));
    }
    let repo = match how {
        Use::Workdir => Repository::open(path)?,
        Use::Refs => Repository::discover(path)?,
    };
    #[cfg(test)]
    count_open(path, how);
    let fresh = fresh::Fresh::taken(&repo, how == Use::Workdir);
    let slot = Slot { key: path.to_owned(), how, group: Group::of(&repo), fresh };
    Ok(lent(repo, slot))
}

fn lent(repo: Repository, slot: Slot) -> Pooled {
    let mut p = pool();
    let id = p.next_id;
    p.next_id += 1;
    p.lent.push((id, slot.key.clone(), slot.group.clone(), false));
    Pooled { repo: Some(repo), slot: Some(slot), id }
}

/// Drops the idle handles of the repository at `path` (every worktree of it) and of any
/// opened at that spelling. Handles lent now are dropped when they come back.
pub fn forget(path: &Path) {
    let target = Target::of(path);
    let gone = {
        let mut p = pool();
        take_covered(&mut p, &target)
    };
    drop(gone);
}

/// Keeps the repository at `path` out of the pool until the returned guard drops: its idle
/// handles are dropped now, and handles lent meanwhile are dropped when they come back.
#[must_use = "the repository is kept out of the pool only while the guard lives"]
pub fn hold(path: &Path) -> Hold {
    let target = Target::of(path);
    let gone = {
        let mut p = pool();
        p.held.push(target.clone());
        take_covered(&mut p, &target)
    };
    drop(gone);
    Hold(target)
}

/// See `hold`.
pub struct Hold(Target);

impl Drop for Hold {
    fn drop(&mut self) {
        let mut p = pool();
        if let Some(k) = p.held.iter().position(|t| *t == self.0) {
            p.held.remove(k);
        }
    }
}

/// Takes out the idle handles `target` covers, and marks the lent ones to be dropped.
fn take_covered(p: &mut Pool, target: &Target) -> Vec<Idle> {
    for (_, key, group, doomed) in &mut p.lent {
        *doomed |= target.covers(key, group);
    }
    let (gone, kept) = std::mem::take(&mut p.idle).into_iter().partition(|i| target.covers(&i.slot.key, &i.slot.group));
    p.idle = kept;
    gone
}

fn check_in(repo: Repository, slot: Slot, id: u64) {
    let mut gone = Vec::new();
    let mut p = pool();
    let doomed = match p.lent.iter().position(|l| l.0 == id) {
        Some(k) => p.lent.swap_remove(k).3,
        None => true,
    };
    // A read that panicked may have left the handle part way through something.
    if doomed || std::thread::panicking() || p.held.iter().any(|t| t.covers(&slot.key, &slot.group)) {
        drop(p);
        return;
    }
    let same = |i: &Idle| i.slot.key == slot.key && i.slot.how == slot.how;
    if p.idle.iter().filter(|i| same(i)).count() >= MAX_PER_KEY
        && let Some(k) = p.idle.iter().position(same)
    {
        gone.push(p.idle.remove(k));
    }
    if p.idle.len() >= MAX_IDLE {
        gone.push(p.idle.remove(0));
    }
    p.idle.push(Idle { slot, repo, since: Instant::now() });
    if !p.reaper {
        p.reaper = std::thread::Builder::new().name("glowfly-repo-pool".into()).spawn(reap).is_ok();
        if !p.reaper {
            gone.append(&mut p.idle);
        }
    }
    drop(p);
    drop(gone);
}

/// Drops handles as they expire; ends when none is left.
fn reap() {
    loop {
        let (gone, next) = {
            let mut p = pool();
            let gone = expire(&mut p, Instant::now());
            let next = p.idle.iter().map(|i| i.since + IDLE).min();
            p.reaper = next.is_some();
            (gone, next)
        };
        drop(gone);
        match next {
            Some(at) => std::thread::sleep(at.saturating_duration_since(Instant::now()) + Duration::from_millis(10)),
            None => return,
        }
    }
}

/// Takes out the handles idle for `IDLE` or longer at `now`.
fn expire(p: &mut Pool, now: Instant) -> Vec<Idle> {
    let (gone, kept) = std::mem::take(&mut p.idle).into_iter().partition(|i| now.saturating_duration_since(i.since) >= IDLE);
    p.idle = kept;
    gone
}

/// How many idle handles the pool keeps for `path` (as spelled when they were lent), for tests.
#[doc(hidden)]
pub fn kept(path: &Path) -> usize {
    pool().idle.iter().filter(|i| i.slot.key == path).count()
}

#[cfg(test)]
fn count_open(path: &Path, how: Use) {
    let mut p = pool();
    match p.opened.iter_mut().find(|(k, h, _)| k == path && *h == how) {
        Some((_, _, n)) => *n += 1,
        None => p.opened.push((path.to_owned(), how, 1)),
    }
}
