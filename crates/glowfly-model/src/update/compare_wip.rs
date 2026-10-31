//! Comparing one commit with the working directory (`Selection::VsWorkdir`): Ctrl-click on the
//! WIP row or a commit, the commit menu's entry (also in the palette and the commit details),
//! Swap direction, reading the file list in the chosen direction (again after every
//! working-tree status read, so it follows edits), and keeping it across refreshes.

use crate::effect::Effect;
use crate::effect::SelectionEffect;
use crate::state::selection_wip::{WorkdirCompare, wip_toggled};
use crate::state::{AppState, Selection};
use git2::Oid;
use glowfly_core::repo::{Comparison, Snapshot};

/// Ctrl-click on the WIP row: one commit gains the working directory, a comparison drops it,
/// otherwise the WIP row is selected. Refused (logged) with several commits selected.
pub(super) fn toggle(st: &mut AppState, fx: &mut Vec<Effect>) {
    match wip_toggled(&st.session.selection) {
        // Only graph rows are compared (a stash selected in the sidebar isn't one).
        Ok(Selection::VsWorkdir(w)) if !st.snap.as_ref().is_some_and(|s| s.row_of.contains_key(&w.oid)) => {
            super::selection::apply(st, Selection::Wip, fx)
        }
        Ok(new) => super::selection::apply(st, new, fx),
        Err(why) => st.log(false, why),
    }
}

/// The commit menu's Compare with working directory, for graph row `oid`.
pub(super) fn compare(st: &mut AppState, oid: Oid, fx: &mut Vec<Effect>) {
    if !st.snap.as_ref().is_some_and(|s| s.row_of.contains_key(&oid))
        || matches!(&st.session.selection, Selection::VsWorkdir(w) if w.oid == oid)
    {
        return;
    }
    super::selection::apply(st, Selection::VsWorkdir(WorkdirCompare::new(oid)), fx);
}

/// Swap direction: the comparison runs the other way (working tree to commit, or back). The
/// list is read again ("Loading" until it arrives; the new generation drops a read of the old
/// direction still on its way), and an open diff of this comparison closes.
pub(super) fn swap(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Selection::VsWorkdir(w) = &mut st.session.selection else { return };
    w.swapped = !w.swapped;
    w.changes = None;
    let oid = w.oid;
    if st.session.diff.view.as_ref().is_some_and(|(t, _)| t.workdir_compared() == Some(oid)) {
        super::repo::close_diff(st);
    }
    request(st, fx);
}

/// Reads the compared commit's file list again, in its direction. The list on screen stays
/// until the new one arrives, so it doesn't flicker.
pub(super) fn request(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    let Selection::VsWorkdir(w) = &mut st.session.selection else { return };
    w.generation += 1;
    fx.push(Effect::Selection(SelectionEffect::WorkdirCompare { workdir, oid: w.oid, generation: w.generation, swapped: w.swapped }));
}

/// A file list read: kept when it is the latest read for the compared commit.
pub(super) fn loaded(st: &mut AppState, oid: Oid, generation: u64, result: Result<Comparison, String>) {
    match &mut st.session.selection {
        Selection::VsWorkdir(w) if w.oid == oid && w.generation == generation => {
            w.changes = Some(result.clone());
            if let Err(e) = result {
                st.log(false, e);
            }
        }
        _ => tracing::debug!("dropped a stale working-tree comparison of {oid} (read {generation})"),
    }
}

/// The working tree's status was read again: so is the comparison's file list.
pub(super) fn status_read(st: &mut AppState, fx: &mut Vec<Effect>) {
    if matches!(st.session.selection, Selection::VsWorkdir(_)) {
        request(st, fx);
    }
}

/// After a refresh (`snap` is the new snapshot, not stored yet): the comparison is read again
/// while its commit is still a graph row; otherwise the WIP row (when dirty) or nothing is
/// selected, and its diff closes.
pub(super) fn refreshed(st: &mut AppState, snap: &Snapshot, fx: &mut Vec<Effect>) {
    let Selection::VsWorkdir(w) = &st.session.selection else { return };
    if snap.row_of.contains_key(&w.oid) {
        request(st, fx);
        return;
    }
    st.session.selection = if snap.is_dirty() { Selection::Wip } else { Selection::None };
    st.session.detail = None;
    if st.session.diff.view.as_ref().is_some_and(|(t, _)| t.workdir_compared().is_some()) {
        super::repo::close_diff(st);
    }
}
