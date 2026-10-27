//! What was read from the active tab's repository and is kept for it: commit stats, line counts,
//! signature checks and the last conflict check. Parking keeps the results and drops the
//! requests in flight (`parked`); opening another repository in the tab forgets them (`clear`).

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
    /// For parking: keeps the results and drops what was on its way. Replies in flight are
    /// dropped, and the tab asks again on return.
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
