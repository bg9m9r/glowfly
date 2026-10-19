//! Per-file line counts of the active tab's commits and of its working tree: a small cache by
//! commit, and the working tree's latest counts with at most one request running at a time.

use git2::Oid;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use trellis_core::repo::{FileLineCounts, WipLineCounts};

/// The most commits whose counts are kept; the oldest stored goes first.
pub const MAX_CACHED_COMMITS: usize = 64;

/// Line counts of commits and of the working tree (parked with the tab).
#[derive(Default)]
pub struct LineCounts {
    commits: HashMap<Oid, Arc<FileLineCounts>>,
    /// `commits`' keys in the order they were stored.
    order: VecDeque<Oid>,
    pending: HashSet<Oid>,
    wip: Option<WipLineCounts>,
    /// The generation of the latest working-tree request: replies to older ones are ignored.
    wip_gen: u64,
    /// A working-tree request is running.
    wip_busy: bool,
    /// A status read came while one was running: ask again when it ends.
    wip_queued: bool,
}

impl LineCounts {
    pub fn commit(&self, oid: Oid) -> Option<&FileLineCounts> {
        self.commits.get(&oid).map(|c| &**c)
    }

    /// Neither stored nor on its way.
    pub fn wants_commit(&self, oid: Oid) -> bool {
        !self.commits.contains_key(&oid) && !self.pending.contains(&oid)
    }

    pub fn start_commit(&mut self, oid: Oid) {
        self.pending.insert(oid);
    }

    /// Stores `oid`'s counts, dropping the oldest stored past `MAX_CACHED_COMMITS`.
    pub fn finish_commit(&mut self, oid: Oid, counts: Arc<FileLineCounts>) {
        self.pending.remove(&oid);
        if self.commits.insert(oid, counts).is_none() {
            self.order.push_back(oid);
        }
        while self.order.len() > MAX_CACHED_COMMITS {
            if let Some(old) = self.order.pop_front() {
                self.commits.remove(&old);
            }
        }
    }

    /// The count failed: nothing is stored (the rows show no counts), and it can be asked again.
    pub fn commit_failed(&mut self, oid: Oid) {
        self.pending.remove(&oid);
    }

    /// Drops `oid`'s counts (they were worked out while its parents were missing).
    pub fn forget_commit(&mut self, oid: Oid) {
        if self.commits.remove(&oid).is_some() {
            self.order.retain(|o| *o != oid);
        }
        self.pending.remove(&oid);
    }

    /// The working tree's latest counts (the previous ones while a newer request runs).
    pub fn wip(&self) -> Option<&WipLineCounts> {
        self.wip.as_ref()
    }

    /// Starts a working-tree request: its generation, or None when one is running (it is then
    /// asked for again when that one ends).
    pub fn start_wip(&mut self) -> Option<u64> {
        if self.wip_busy {
            self.wip_queued = true;
            return None;
        }
        self.wip_busy = true;
        self.wip_gen += 1;
        Some(self.wip_gen)
    }

    /// Stores the reply to request `generation` (`None`: it failed, so no counts are shown).
    /// Returns whether a queued request must be sent now. A stale reply changes nothing.
    pub fn finish_wip(&mut self, generation: u64, result: Option<WipLineCounts>) -> bool {
        if generation != self.wip_gen || !self.wip_busy {
            return false;
        }
        self.wip_busy = false;
        self.wip = result;
        std::mem::take(&mut self.wip_queued)
    }

    /// The working tree is clean: nothing to count. A request still running is ignored.
    pub fn clear_wip(&mut self) {
        self.wip = Some(WipLineCounts::default());
        self.wip_gen += 1;
        self.wip_busy = false;
        self.wip_queued = false;
    }

    /// Forgets everything (another repository was opened); replies in flight are ignored.
    pub fn clear(&mut self) {
        let wip_gen = self.wip_gen + 1;
        *self = LineCounts { wip_gen, ..Default::default() };
    }

    /// For parking: keeps the stored counts, drops what was on its way. Both sides get a newer
    /// generation, so a reply in flight is ignored.
    pub fn parked(&mut self) -> LineCounts {
        self.wip_gen += 1;
        let parked = LineCounts {
            commits: std::mem::take(&mut self.commits),
            order: std::mem::take(&mut self.order),
            wip: self.wip.take(),
            wip_gen: self.wip_gen,
            ..Default::default()
        };
        self.pending.clear();
        self.wip_busy = false;
        self.wip_queued = false;
        parked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(n: u8) -> Oid {
        Oid::from_bytes(&[n; 20]).unwrap()
    }

    #[test]
    fn commits_are_evicted_oldest_first() {
        let mut c = LineCounts::default();
        for n in 0..=MAX_CACHED_COMMITS as u8 {
            c.start_commit(oid(n));
            c.finish_commit(oid(n), Arc::default());
        }
        assert!(c.wants_commit(oid(0)), "the oldest is gone");
        assert!(c.commit(oid(1)).is_some() && c.commit(oid(MAX_CACHED_COMMITS as u8)).is_some());
        c.start_commit(oid(200));
        assert!(!c.wants_commit(oid(200)));
        c.commit_failed(oid(200));
        assert!(c.wants_commit(oid(200)), "a failure can be asked again");
    }

    #[test]
    fn parking_keeps_results_and_ignores_replies_in_flight() {
        let mut c = LineCounts::default();
        c.finish_commit(oid(1), Arc::default());
        c.start_commit(oid(2));
        let g = c.start_wip().unwrap();
        let mut parked = c.parked();
        assert!(parked.commit(oid(1)).is_some() && parked.wants_commit(oid(2)));
        assert!(!parked.finish_wip(g, Some(WipLineCounts::default())));
        assert!(parked.wip().is_none(), "the old reply is ignored");
        assert!(parked.start_wip().is_some(), "not busy after the return");
    }
}
