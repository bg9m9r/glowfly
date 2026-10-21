//! The commits on each side of two loaded commits, such as a branch and its upstream: those
//! reachable from one but not the other, worked out from the loaded graph alone (no I/O). The
//! toolbar's Push and Pull previews and the undo previews read it.

use super::Snapshot;
use git2::Oid;
use std::collections::BTreeMap;

/// What two commits don't share, newest first on each side.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Divergence {
    /// Commits reachable from `ours` but not `theirs`, at most `limit` of them.
    pub ahead: Vec<Oid>,
    /// Commits reachable from `theirs` but not `ours`, at most `limit` of them.
    pub behind: Vec<Oid>,
    /// How many commits each side has that the other lacks, past `limit` too.
    pub ahead_total: usize,
    pub behind_total: usize,
    /// False when a one-sided commit's parent isn't loaded (the history was cut short), so the
    /// lists and totals may miss commits.
    pub complete: bool,
}

const OURS: u8 = 1;
const THEIRS: u8 = 2;

fn one_sided(flags: u8) -> bool {
    flags == OURS || flags == THEIRS
}

/// Adds `flags` to `row`'s queued flags, keeping `open` (the one-sided queued rows) right.
fn mark(queue: &mut BTreeMap<usize, u8>, open: &mut usize, row: usize, flags: u8) {
    let entry = queue.entry(row).or_insert(0);
    if one_sided(*entry) {
        *open -= 1;
    }
    *entry |= flags;
    if one_sided(*entry) {
        *open += 1;
    }
}

impl Snapshot {
    /// The commits `ours` and `theirs` don't share, from the loaded history, keeping at most
    /// `limit` oids a side; None when either isn't loaded.
    ///
    /// `commits` lists children before parents, so visiting rows in ascending order sees every
    /// child of a row before the row itself: each row's flags (reachable from ours, from theirs)
    /// are final when it is visited. The walk stops once no one-sided row is left to visit.
    pub fn divergence(&self, ours: Oid, theirs: Oid, limit: usize) -> Option<Divergence> {
        let (&ours_row, &theirs_row) = (self.row_of.get(&ours)?, self.row_of.get(&theirs)?);
        let mut queue: BTreeMap<usize, u8> = BTreeMap::new();
        // Queued rows whose flags are one-sided.
        let mut open = 0usize;
        mark(&mut queue, &mut open, ours_row, OURS);
        mark(&mut queue, &mut open, theirs_row, THEIRS);
        let mut d = Divergence { complete: true, ..Divergence::default() };
        while open > 0 {
            let Some((row, flags)) = queue.pop_first() else { break };
            let commit = &self.commits[row];
            if one_sided(flags) {
                open -= 1;
                let (list, total) = if flags == OURS { (&mut d.ahead, &mut d.ahead_total) } else { (&mut d.behind, &mut d.behind_total) };
                *total += 1;
                if list.len() < limit {
                    list.push(commit.oid);
                }
            }
            for parent in &commit.parents {
                match self.row_of.get(parent) {
                    Some(&p) => mark(&mut queue, &mut open, p, flags),
                    None if one_sided(flags) => d.complete = false,
                    None => {}
                }
            }
        }
        Some(d)
    }
}
