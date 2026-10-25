//! The order the history walk visits commits in, shared by the graph's walk (`walk.rs`) and the
//! search index (`commit_index`): newest by committer time first, ties in the order the commits
//! were found, each commit once. Both must agree exactly, so a commit's place in the index's walk
//! says how many commits the graph has to load to show it.
//!
//! A walk cut short can be resumed: its queue is kept as a `Frontier` (the queued commits with
//! their places, and the push counter) and `TimeQueue::resume` rebuilds it, so the resumed walk
//! pops exactly what a longer walk from the same tips would have popped next. Nothing is lost in
//! between, as every commit pushed is either loaded or still queued.

use super::walk_reuse::Frontier;
use git2::Oid;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};

/// The commits still to visit, newest committer time first; a commit pushed twice is visited once.
#[derive(Default)]
pub(crate) struct TimeQueue {
    queue: BinaryHeap<(i64, Reverse<u64>, Oid)>,
    seen: HashSet<Oid>,
    /// Counts pushes, so ties in time come out in the order they were found (a tip pushed first
    /// comes first).
    order: u64,
}

impl TimeQueue {
    /// Queues `oid` with committer time `time` (seconds), unless it was queued before.
    pub(crate) fn push(&mut self, oid: Oid, time: i64) {
        if self.seen.insert(oid) {
            self.order += 1;
            self.queue.push((time, Reverse(self.order), oid));
        }
    }

    /// The next commit to visit.
    pub(crate) fn pop(&mut self) -> Option<Oid> {
        self.queue.pop().map(|(_, _, oid)| oid)
    }

    /// Whether every commit found has been visited.
    pub(crate) fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// The commits still queued, with their places, and the push counter: what `resume` needs.
    pub(crate) fn frontier(&self) -> Frontier {
        Frontier::new(self.queue.iter().map(|(t, Reverse(o), oid)| (*t, *o, *oid)).collect(), self.order)
    }

    /// The queue a walk stopped with at `f`, having visited `loaded`: each of them and each
    /// queued commit counts as seen, so none is queued again.
    pub(crate) fn resume(f: &Frontier, loaded: impl Iterator<Item = Oid>) -> TimeQueue {
        let mut seen: HashSet<Oid> = loaded.collect();
        seen.extend(f.queued().iter().map(|e| e.2));
        let queue = f.queued().iter().map(|&(t, o, oid)| (t, Reverse(o), oid)).collect();
        TimeQueue { queue, seen, order: f.order() }
    }
}

/// The parents the walk follows from `c`, with their committer times, in parent order. These are
/// git2's `parents()`, which ends at the first parent that can't be read (a missing object in a
/// broken or partial repository), unlike `parent_ids()`; every walk goes through this so they stop
/// at the same place.
pub(crate) fn walk_parents<'r>(c: &git2::Commit<'r>) -> impl Iterator<Item = (Oid, i64)> {
    c.parents().map(|p| (p.id(), p.time().seconds()))
}
