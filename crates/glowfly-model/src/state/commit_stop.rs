//! Where a stopped rebase leaves the commit form: whether it stopped (no push after committing
//! then), whether it stopped to edit a commit (the form commits and continues), and whether git
//! would amend HEAD there, which decides whether Amend is offered.

use super::amend_blocked;
use git2::RepositoryState;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;

/// Where a rebase stopped to edit a commit, if it did: there the commit form amends it and
/// continues the rebase in one go. Which stop it is: the step and the commit.
pub fn edit_stop(snap: &Snapshot) -> Option<EditStop> {
    snap.rebase.as_ref().filter(|p| p.stop.kind == glowfly_core::rebase::StopKind::Edit).map(|p| (p.step, p.stop.oid))
}

/// A rebase's stop to edit: its step and the commit (`edit_stop`).
pub type EditStop = (usize, Option<git2::Oid>);

/// At a stop to edit, whether git would amend HEAD: it stopped cleanly and HEAD is still the
/// commit being edited (not at an `edit` step that conflicted, nor after a reset to split it).
pub fn amends_at_stop(snap: &Snapshot) -> bool {
    edit_stop(snap).is_some() && snap.rebase.as_ref().is_some_and(|p| p.amends)
}

/// Why Amend can't be used now: `amend_blocked` for the operation in progress, and at a stop to
/// edit where HEAD isn't the commit being edited, since amending would rewrite the commit before it.
pub fn amend_blocked_in(t: Lang, snap: &Snapshot) -> Option<&'static str> {
    amend_blocked(t, snap.state)
        .or_else(|| (edit_stop(snap).is_some() && !amends_at_stop(snap)).then(|| t.tr("commit.blocked.amend_not_edited")))
}

/// A rebase is stopped (to edit, at a break, on a conflict or a failed command).
pub fn rebase_stopped(snap: &Snapshot) -> bool {
    use RepositoryState::*;
    snap.rebase.is_some() || matches!(snap.state, Rebase | RebaseMerge | RebaseInteractive)
}
