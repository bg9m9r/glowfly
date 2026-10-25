//! What a history walk takes from an earlier one: the commits a snapshot holds (`Prior`), read
//! again by oid instead of from the object database, and where a cut walk stopped (`Frontier`),
//! for a load for more to go on from.
//!
//! A commit's oid names its content, so a loaded `CommitInfo` stays right for as long as the
//! commit exists. Two things can change what libgit2 reads for an unchanged oid: a shallow
//! clone's `shallow` file (fetching deeper gives a former root its parents), so every commit
//! named in the old or the new list is read again; and `info/grafts`, which turns the reuse off.

use super::{Authors, CommitInfo, Snapshot};
use git2::{Oid, Repository};
use std::collections::HashMap;
use std::sync::Arc;

/// Where a history walk cut short stopped: the commits still queued, as (committer time, push
/// place, oid), and the push counter. With the commits it loaded, all a resumed walk needs
/// (`walk_order::TimeQueue::resume`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Frontier {
    queued: Vec<(i64, u64, Oid)>,
    order: u64,
}

impl Frontier {
    pub(crate) fn new(queued: Vec<(i64, u64, Oid)>, order: u64) -> Frontier {
        Frontier { queued, order }
    }

    pub(crate) fn queued(&self) -> &[(i64, u64, Oid)] {
        &self.queued
    }

    pub(crate) fn order(&self) -> u64 {
        self.order
    }

    /// How many commits are queued.
    pub fn len(&self) -> usize {
        self.queued.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queued.is_empty()
    }

    /// The queued entries in a fixed order, for comparing two frontiers in tests.
    #[cfg(test)]
    pub(crate) fn sorted(&self) -> (Vec<(i64, u64, Oid)>, u64) {
        let mut q = self.queued.clone();
        q.sort_unstable();
        (q, self.order)
    }
}

/// The history a snapshot on screen holds, for the next load of its repository: commits it may
/// take instead of reading them again, and where its walk stopped.
#[derive(Clone)]
pub struct Prior {
    pub commits: Arc<[CommitInfo]>,
    /// The shallow roots it was loaded with (`Snapshot::shallow_roots`).
    pub shallow_roots: Vec<Oid>,
    pub frontier: Option<Arc<Frontier>>,
}

impl std::fmt::Debug for Prior {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let frontier = if self.frontier.is_some() { "yes" } else { "no" };
        write!(f, "Prior({} commits, frontier: {frontier})", self.commits.len())
    }
}

impl Snapshot {
    /// This snapshot's history, for a later load to build on (the commits shared).
    pub fn prior(&self) -> Prior {
        Prior { commits: Arc::clone(&self.commits), shallow_roots: self.shallow_roots.clone(), frontier: self.frontier.clone() }
    }
}

/// The commits of a `Prior` by oid, without those whose parents may read differently now.
pub(super) struct CommitCache {
    commits: Arc<[CommitInfo]>,
    rows: HashMap<Oid, u32>,
}

impl CommitCache {
    /// The commits of `prior` a walk in `repo` (whose shallow roots are now `shallow_now`) may
    /// take as they are; None when the repository has `info/grafts`.
    pub(super) fn new(prior: &Prior, repo: &Repository, shallow_now: &[Oid]) -> Option<CommitCache> {
        if repo.commondir().join("info").join("grafts").exists() {
            return None;
        }
        let mut rows: HashMap<Oid, u32> = prior.commits.iter().enumerate().map(|(i, c)| (c.oid, i as u32)).collect();
        for oid in prior.shallow_roots.iter().chain(shallow_now) {
            rows.remove(oid);
        }
        Some(CommitCache { commits: Arc::clone(&prior.commits), rows })
    }

    pub(super) fn get(&self, oid: Oid) -> Option<&CommitInfo> {
        self.rows.get(&oid).map(|&i| &self.commits[i as usize])
    }

    /// An authors table holding every author of the cached commits, so a new commit by one of
    /// them shares the copy the reused commits hold.
    pub(super) fn authors(&self) -> Authors {
        authors_of(&self.commits)
    }
}

/// An authors table seeded with the authors of `commits`.
pub(super) fn authors_of(commits: &[CommitInfo]) -> Authors {
    let mut authors = Authors::default();
    for c in commits {
        authors.seed(&c.author);
    }
    authors
}
