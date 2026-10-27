//! What was read from the active tab's repository and is kept for it: commit stats, line counts,
//! signature checks and the last conflict check. Parking splits the results, which go with the
//! tab, from the counters, which stay behind moved past every request in flight (`parked`); the
//! counters join `RepoSession::counters` so they rise past every tab's. Opening another
//! repository in the tab forgets the results (`clear`).

use git2::Oid;
use std::collections::{HashMap, HashSet};
use std::mem::take;

/// The repository reads of a tab (`RepoSession::reads`).
#[derive(Default)]
pub struct RepoReads {
    /// Per-commit (files, added, deleted) for the CHANGES column.
    pub stats: HashMap<Oid, (usize, usize, usize)>,
    /// Commits whose stats are on their way.
    pub stats_pending: HashSet<Oid>,
    /// Per-file line counts of commits and of the working tree.
    pub line_counts: super::line_counts::LineCounts,
    /// Signature checks of the tab's commits.
    pub signatures: super::signatures::Signatures,
    /// Predicted merge conflicts of the checked-out branch (one in flight is asked for again on
    /// return).
    pub conflict_checks: super::conflict_check::ConflictChecks,
}

impl RepoReads {
    /// The request counters (working-tree line counts, signatures, conflict checks), for
    /// `RepoSession::counters`.
    pub(super) fn counters(&mut self) -> [&mut u64; 3] {
        [self.line_counts.counter(), self.signatures.counter(), self.conflict_checks.counter()]
    }

    /// Splits the results from the counters: returns the results (requests in flight dropped,
    /// asked for again on return); `self` keeps no results and the same counters, moved past
    /// every request in flight. `RepoSession::park` leaves `self` behind, so the next tab's
    /// counters move past it.
    pub fn parked(&mut self) -> RepoReads {
        self.stats_pending.clear();
        RepoReads {
            stats: take(&mut self.stats),
            stats_pending: HashSet::new(),
            line_counts: self.line_counts.parked(),
            signatures: self.signatures.parked(),
            conflict_checks: self.conflict_checks.parked(),
        }
    }

    /// Forgets everything (another repository was opened); replies in flight are ignored.
    pub fn clear(&mut self) {
        self.stats.clear();
        self.stats_pending.clear();
        self.line_counts.clear();
        self.signatures.clear();
        self.conflict_checks.clear();
    }
}

#[cfg(test)]
#[path = "session_reads_tests.rs"]
mod tests;
