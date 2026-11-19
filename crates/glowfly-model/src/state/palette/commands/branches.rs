//! The palette's branch entries: Delete branch…, whose sub-list of the local branches opens the
//! Delete branch dialog, offered while there is a local branch besides the checked-out one, and
//! listed below its match for a typed query.

use super::{Entry, PaletteCommand as C, entry};
use glowfly_core::repo::Snapshot;

/// Whether Delete branch… can be offered: a local branch other than the checked-out one (with a
/// detached HEAD every local branch counts).
pub(crate) fn can_pick(snap: &Snapshot) -> bool {
    snap.locals.iter().any(|b| !b.is_head)
}

pub(super) fn branch_entries(snap: &Snapshot, out: &mut Vec<Entry>) {
    if can_pick(snap) {
        out.push(entry(C::ChooseBranchToDelete, "Delete branch…", "remove local drop erase"));
    }
}

impl C {
    /// Whether a typed query ranks this command below the others it ties with or trails by a
    /// little, unless the query starts at its label's first letter (`cache::demote`): "branch"
    /// lists creating and switching branches before Delete branch….
    pub(in crate::state) fn ranks_low(&self) -> bool {
        matches!(self, C::ChooseBranchToDelete)
    }
}
