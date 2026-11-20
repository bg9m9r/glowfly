//! An update's shortcut: spotting where the rest of its walk is the previous index's tail, and
//! appending that tail in bulk instead of walking it.
//!
//! The previous index's walk (the base) is a sequence `e_0..e_{N-1}` from the base's starts. Let
//! `P_p` be its first `p` commits and `F_p` its queue after `p` pops. Each queued commit keeps a
//! push counter; on equal committer times the lower counter pops first. Suppose the new walk
//! reaches a state where
//!
//! 1. the base commits it has popped are exactly `P_p` (positions `0..p`),
//! 2. no commit outside the base is queued, and
//! 3. its queue holds exactly `F_p`, and sorting both queues by (time descending, push counter
//!    ascending) gives the same sequence of commits.
//!
//! Then the rest of the new walk pops `e_p..e_{N-1}`. Every walked parent of a base commit is in
//! the base (a base is complete). A base commit counts as seen in the new walk exactly when it is
//! in `P_p` or `F_p`, as in the base walk. Later pushes get counters above every current one in
//! both walks, in the same sequence, and commits outside the base that were popped already never
//! appear as parents of base commits, so they change nothing. So both walks pop and push the same
//! commits in the same order, by induction.
//!
//! When a base commit is never reached by the new walk (a force-push, a ref moved back, a deleted
//! branch with commits nothing else reaches) it stays in `F_p` for every `p` up to its position
//! and is missing from the popped set after it: conditions 1 and 3 never hold together and the
//! walk runs to the end as a fresh one would.

use super::{Builder, CommitIndex, Entry};
use crate::repo::walk_order::TimeQueue;
use git2::Oid;
use std::cmp::Reverse;
use std::collections::HashMap;

/// A commit's place in a set fingerprint: the first bytes of its id.
fn fp(oid: Oid) -> u64 {
    let mut b = [0u8; 8];
    let src = oid.as_bytes();
    let n = b.len().min(src.len());
    b[..n].copy_from_slice(&src[..n]);
    u64::from_le_bytes(b)
}

/// Watches an update's walk for the point where the rest of it is the base walk's tail.
pub(super) struct Splice<'b> {
    base: &'b CommitIndex,
    /// Gave up: the base turned out inconsistent (should not happen).
    off: bool,
    /// Commits in the new queue.
    queued: usize,
    /// Wrapping sum of `fp` over the new queue.
    qhash: u64,
    /// Queued commits the base lacks.
    foreign: usize,
    /// Base commits popped so far.
    copied: usize,
    /// The highest base position popped.
    max_pos: Option<u32>,
    /// No full check runs again before this many base commits were popped.
    retry_at: usize,
    /// The base walk replayed lazily up to position `p`: its queue then, as commit to push counter.
    p: usize,
    frontier: HashMap<Oid, u64>,
    /// The base's push counter.
    order: u64,
    /// Wrapping sum of `fp` over `frontier`.
    fhash: u64,
}

impl<'b> Splice<'b> {
    /// A watch on a walk updating `base`, with the base's start pushes replayed.
    pub(super) fn new(base: &'b CommitIndex) -> Splice<'b> {
        let mut s = Splice {
            base,
            off: false,
            queued: 0,
            qhash: 0,
            foreign: 0,
            copied: 0,
            max_pos: None,
            retry_at: 0,
            p: 0,
            frontier: HashMap::new(),
            order: 0,
            fhash: 0,
        };
        for &start in &base.key().starts {
            if base.position(start).is_none() {
                s.off = true;
            }
            s.enqueue(start);
        }
        s
    }

    /// The base walk queues `oid` unless it has seen it.
    fn enqueue(&mut self, oid: Oid) {
        if !self.frontier.contains_key(&oid) {
            self.order += 1;
            self.frontier.insert(oid, self.order);
            self.fhash = self.fhash.wrapping_add(fp(oid));
        }
    }

    fn give_up(&mut self) -> Option<usize> {
        self.off = true;
        self.frontier = HashMap::new();
        None
    }

    /// The new walk queued `oid` now (`TimeQueue::push` said so); `in_base` when the base holds it.
    pub(super) fn pushed(&mut self, oid: Oid, in_base: bool) {
        self.queued += 1;
        self.qhash = self.qhash.wrapping_add(fp(oid));
        if !in_base {
            self.foreign += 1;
        }
    }

    /// The new walk popped `oid`, at base position `pos` when the base holds it.
    pub(super) fn popped(&mut self, oid: Oid, pos: Option<u32>) {
        self.queued = self.queued.saturating_sub(1);
        self.qhash = self.qhash.wrapping_sub(fp(oid));
        match pos {
            Some(pos) => {
                self.copied += 1;
                self.max_pos = Some(self.max_pos.map_or(pos, |m| m.max(pos)));
            }
            None => self.foreign = self.foreign.saturating_sub(1),
        }
    }

    /// Called before every pop: the base position `p` at which the rest of the walk is the
    /// base's tail, when `queue` is the base's queue there (see the module docs).
    pub(super) fn ready(&mut self, queue: &TimeQueue) -> Option<usize> {
        let exact = self.max_pos.map_or(0, |m| m as usize + 1);
        if self.off || self.foreign > 0 || self.copied != exact || self.copied < self.retry_at {
            return None;
        }
        if !self.advance_to(self.copied) {
            return self.give_up();
        }
        if self.frontier.len() != self.queued || self.fhash != self.qhash {
            return None;
        }
        if self.same_order(queue) {
            return Some(self.p);
        }
        self.retry_at = self.copied + self.queued.max(1);
        None
    }

    /// Replays the base walk to `to` pops; false when the base doesn't hold together.
    fn advance_to(&mut self, to: usize) -> bool {
        while self.p < to {
            let e = self.base.entries[self.p];
            if self.frontier.remove(&e.oid).is_none() {
                return false;
            }
            self.fhash = self.fhash.wrapping_sub(fp(e.oid));
            self.p += 1;
            let at = e.parents_at as usize;
            for &q in &self.base.parents[at..at + e.parents_len as usize] {
                match self.base.position(q) {
                    None => return false,
                    Some(pos) if (pos as usize) < self.p => {}
                    Some(_) => self.enqueue(q),
                }
            }
        }
        true
    }

    /// Whether the new queue and the base's at `p` pop their commits in the same order.
    fn same_order(&self, queue: &TimeQueue) -> bool {
        let mut new: Vec<(i64, u64, Oid)> = queue.frontier().queued().to_vec();
        new.sort_by_key(|&(t, c, _)| (Reverse(t), c));
        let mut old: Vec<(i64, u64, Oid)> = Vec::with_capacity(self.frontier.len());
        for (&oid, &c) in &self.frontier {
            match self.base.time_of(oid) {
                Some(t) => old.push((t, c, oid)),
                None => return false,
            }
        }
        old.sort_by_key(|&(t, c, _)| (Reverse(t), c));
        new.len() == old.len() && new.iter().zip(&old).all(|(a, b)| a.2 == b.2)
    }
}

impl Builder {
    /// Appends `count` of the base's entries from position `from`, as they are: the texts and
    /// authors are shared (`Builder::new` took the base's), and their walked parents are one run
    /// of the base's parents, copied whole.
    pub(super) fn copy_tail(&mut self, base: &CommitIndex, from: usize, count: usize) {
        if count == 0 {
            return;
        }
        let tail: &[Entry] = &base.entries[from..from + count];
        let (first, last) = (tail[0], tail[count - 1]);
        let run = first.parents_at as usize..(last.parents_at + last.parents_len) as usize;
        let shift = self.ix.parents.len() as i64 - first.parents_at as i64;
        self.ix.parents.extend_from_slice(&base.parents[run]);
        self.ix.entries.extend(tail.iter().map(|e| Entry { parents_at: (e.parents_at as i64 + shift) as u32, ..*e }));
    }

    /// Fills `by_oid` for the `count` entries copied from the base's position `from` once `at`
    /// entries were in (see `copy_tail`).
    pub(super) fn index_tail(&mut self, base: &CommitIndex, from: usize, at: usize, count: usize) {
        if from + count < base.len() {
            for i in 0..count {
                let oid = self.ix.entries[at + i].oid;
                self.ix.by_oid.insert(oid, (at + i) as u32);
            }
            return;
        }
        // The whole tail: the base's positions moved along, then the new prefix's over them (every
        // base commit before `from` is in it) and the commits the update read.
        let mut map = base.by_oid.clone();
        for v in map.values_mut() {
            if *v as usize >= from {
                *v = (*v as usize + at - from) as u32;
            }
        }
        map.extend(self.ix.by_oid.drain());
        self.ix.by_oid = map;
    }
}
