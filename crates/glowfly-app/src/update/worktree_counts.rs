//! Counting other worktrees' changes after a load. A full load counts only the small ones and
//! leaves the rest Pending, so one huge linked worktree doesn't hold up the graph; right after it
//! one background effect (`WorktreeEffect::CountChanges`) counts those, and the result
//! (`WorktreeMsg::ChangesCounted`) fills in their counts and lays the graph out again. One count
//! runs per tab at a time, and it outlives refreshes that find the repository unchanged.
//!
//! A reload that keeps the snapshot (the watcher's, a status-only one, returning to the tab or
//! the window) counts the counted ones again through the same effect (`recount`), throttled by
//! `RefreshState::recount` so many worktrees aren't read on every file event.

use crate::effect::Effect;
use crate::effect::WorktreeEffect;
use crate::state::AppState;
use crate::worktree::CountRequest;
use glowfly_core::paths::same_spelling;
use glowfly_core::repo::{ChangesRead, WorktreeCount, WorktreeInfo, worktree_wip_heads};
use std::time::Instant;

/// A full load has just counted the other worktrees (`now`): asks for the Pending ones, and the
/// next recount waits its interval from here.
pub(super) fn loaded(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    st.refresh.recount.started(now);
    want(st, fx);
}

/// Asks for the background count of the snapshot's Pending worktrees, unless one runs already
/// or the option is off.
pub(super) fn want(st: &mut AppState, fx: &mut Vec<Effect>) {
    ask(st, |w| w.read == ChangesRead::Pending, fx);
}

/// Asks for one background count of the snapshot's worktrees `pick` chooses; false when none
/// was asked for (one runs already, the option is off, or there are none).
fn ask(st: &mut AppState, pick: impl Fn(&WorktreeInfo) -> bool, fx: &mut Vec<Effect>) -> bool {
    if !st.settings.worktree_wips || st.refresh.counting.is_some() {
        return false;
    }
    let Some(snap) = st.snap.as_ref() else { return false };
    let picked: Vec<_> = snap.worktrees.iter().filter(|w| pick(w)).collect();
    if picked.is_empty() {
        return false;
    }
    // Each worktree by its own decision, never the tab's: a linked worktree has its own config
    // and submodules, and opening it in a tab would hold git there too.
    let targets = picked.iter().map(|w| (w.path.clone(), st.saved_trust_for(&w.path))).collect();
    let request = CountRequest { workdir: snap.workdir.clone(), refs_key: snap.refs_key, targets };
    st.refresh.counting = Some(snap.refs_key);
    fx.push(Effect::Worktree(WorktreeEffect::CountChanges(request)));
    true
}

/// Other worktrees counted before (not the current one, a missing one, or one too large to
/// count in the background): what a recount reads again.
fn recountable(w: &WorktreeInfo) -> bool {
    !w.current && !w.missing && w.read == ChangesRead::Counted
}

/// Counts the other worktrees again after a reload that kept the snapshot, at `now`: at once
/// when the last count is `RECOUNT_EVERY` old, else once it is (on a tick, or when a running
/// count ends).
pub(super) fn recount(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    let wanted = st.settings.worktree_wips && st.snap.as_ref().is_some_and(|s| s.worktrees.iter().any(recountable));
    if !wanted {
        return;
    }
    if st.refresh.counting.is_some() {
        st.refresh.recount.wait(now);
    } else if st.refresh.recount.ask(now) && ask(st, recountable, fx) {
        st.refresh.recount.started(now);
    }
}

/// Runs the recount kept for later once it is due and no count runs.
pub(super) fn tick(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    if st.refresh.counting.is_none() && st.refresh.recount.is_due(now) {
        recount(st, now, fx);
    }
}

/// The background count for the snapshot with `refs_key` is done: fills in the worktrees it
/// counted (Pending ones, or counted ones read again) when the snapshot is still that one, then
/// asks again for a newer snapshot's, or runs a recount that came due meanwhile.
pub(super) fn counted(st: &mut AppState, refs_key: u64, counts: Vec<WorktreeCount>, now: Instant, fx: &mut Vec<Effect>) {
    if st.refresh.counting == Some(refs_key) {
        st.refresh.counting = None;
    }
    let (mut slow, mut relayout) = (None, false);
    if let Some(snap) = st.snap.as_mut().filter(|s| s.refs_key == refs_key) {
        let rows_before = worktree_wip_heads(&snap.worktrees);
        for wt in snap.worktrees.iter_mut().filter(|w| !w.current) {
            match (counts.iter().find(|c| same_spelling(&c.path, &wt.path)), wt.read) {
                (Some(c), ChangesRead::Pending | ChangesRead::Counted) => {
                    wt.read = c.read;
                    wt.changes = c.changes;
                }
                // Left out of the result: not asked again, so Pending can't loop.
                (None, ChangesRead::Pending) => wt.read = ChangesRead::TooLarge,
                _ => {}
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
    tick(st, now, fx);
}
