//! The palette's branch entries: Delete branch…, whose sub-list of the local branches opens the
//! Delete branch dialog, offered while there is a local branch besides the checked-out one, and
//! Delete remote branch…, whose sub-list of the remote branches opens the Delete remote branch
//! dialog, offered while a remote has a branch. Both are listed below their match for a typed
//! query.

use super::{Entry, PaletteCommand as C, entry};
use glowfly_core::repo::Snapshot;

/// Whether Delete branch… can be offered: a local branch other than the checked-out one (with a
/// detached HEAD every local branch counts).
pub(crate) fn can_pick(snap: &Snapshot) -> bool {
    snap.locals.iter().any(|b| !b.is_head)
}

/// Whether Delete remote branch… can be offered: some remote has a branch that isn't its HEAD.
pub(crate) fn can_pick_remote(snap: &Snapshot) -> bool {
    snap.remotes.iter().any(|(_, bs)| bs.iter().any(|(n, _)| n != "HEAD"))
}

pub(super) fn branch_entries(snap: &Snapshot, out: &mut Vec<Entry>) {
    if can_pick(snap) {
        out.push(entry(C::ChooseBranchToDelete, "Delete branch…", "remove local drop erase"));
    }
    if can_pick_remote(snap) {
        out.push(entry(C::ChooseRemoteBranchToDelete, "Delete remote branch…", "remove drop erase remote origin"));
    }
}

impl C {
    /// Whether a typed query ranks this command below the others it ties with or trails by a
    /// little, unless the query starts at its label's first letter (`cache::demote`): "branch"
    /// lists creating and switching branches before Delete branch… and Delete remote branch….
    pub(in crate::state) fn ranks_low(&self) -> bool {
        matches!(self, C::ChooseBranchToDelete | C::ChooseRemoteBranchToDelete)
    }
}
