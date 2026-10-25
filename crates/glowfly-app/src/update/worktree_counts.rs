//! Counting other worktrees' changes after a load. A full load counts only the small ones and
//! leaves the rest Pending, so one huge linked worktree doesn't hold up the graph; right after it
//! one background effect (`WorktreeEffect::CountChanges`) counts those, and the result
//! (`WorktreeMsg::ChangesCounted`) fills in their counts and lays the graph out again. One count
//! runs per tab at a time, and it outlives refreshes that find the repository unchanged.

use crate::effect::Effect;
use crate::effect::WorktreeEffect;
use crate::state::AppState;
use crate::worktree::CountRequest;
use glowfly_core::paths::same_spelling;
use glowfly_core::repo::{ChangesRead, WorktreeCount, worktree_wip_heads};
use std::time::Instant;

/// Asks for the background count of the snapshot's Pending worktrees, unless one runs already
/// or the option is off.
pub(super) fn want(st: &mut AppState, fx: &mut Vec<Effect>) {
    if !st.settings.worktree_wips || st.refresh.counting.is_some() {
        return;
    }
    let Some(snap) = st.snap.as_ref() else { return };
    let pending: Vec<_> = snap.worktrees.iter().filter(|w| w.read == ChangesRead::Pending).collect();
    if pending.is_empty() {
        return;
    }
    // Each worktree by its own decision, never the tab's: a linked worktree has its own config
    // and submodules, and opening it in a tab would hold git there too.
    let targets = pending.iter().map(|w| (w.path.clone(), st.saved_trust_for(&w.path))).collect();
    let request = CountRequest { workdir: snap.workdir.clone(), refs_key: snap.refs_key, targets };
    st.refresh.counting = Some(snap.refs_key);
    fx.push(Effect::Worktree(WorktreeEffect::CountChanges(request)));
}

/// The background count for the snapshot with `refs_key` is done: fills in the worktrees still
/// Pending when the snapshot is still that one, then asks again for a newer snapshot's.
pub(super) fn counted(st: &mut AppState, refs_key: u64, counts: Vec<WorktreeCount>, now: Instant, fx: &mut Vec<Effect>) {
    if st.refresh.counting == Some(refs_key) {
        st.refresh.counting = None;
    }
    let (mut slow, mut relayout) = (None, false);
    if let Some(snap) = st.snap.as_mut().filter(|s| s.refs_key == refs_key) {
        let rows_before = worktree_wip_heads(&snap.worktrees);
        for wt in snap.worktrees.iter_mut().filter(|w| w.read == ChangesRead::Pending) {
            match counts.iter().find(|c| same_spelling(&c.path, &wt.path)) {
                Some(c) => {
                    wt.read = c.read;
                    wt.changes = c.changes;
                }
                // Left out of the result: not asked again, so Pending can't loop.
                None => wt.read = ChangesRead::TooLarge,
            }
        }
        relayout = worktree_wip_heads(&snap.worktrees) != rows_before;
        slow = counts.iter().find_map(|c| c.slow.clone());
    }
    if relayout {
        super::large_graph::relayout(st, fx);
    }
    super::status_tip::check(st, slow.as_ref(), now, fx);
    want(st, fx);
}
