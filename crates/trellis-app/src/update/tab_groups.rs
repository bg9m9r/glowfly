//! Worktree tab groups (Preferences > UI Customization > Group worktree tabs): where a new tab
//! opens, how a dropped tab moves, and pulling a repository's tabs together once their common
//! dirs are known (from a load, or the startup read of restored tabs). The rules themselves are
//! in `state/tab_groups.rs`.

use crate::effect::Effect;
use crate::state::tab_groups::{blocks, grouped_order, is_identity, move_order, new_index, same_group};
use crate::state::tabs::Place;
use crate::state::{AppState, Tab};
use std::path::{Path, PathBuf};
use trellis_core::repo::CommonDir;

/// Where a new tab for `path` goes: right after the last tab of its group when a loaded or
/// parked snapshot lists `path` as one of its worktrees, otherwise (or with grouping off) at
/// the end.
pub(super) fn slot_for(st: &AppState, path: &Path) -> usize {
    let n = st.tabs.len();
    if !st.settings.group_worktree_tabs {
        return n;
    }
    let id = st.known.id_of(path);
    let lists = |snap: &trellis_core::repo::Snapshot| {
        snap.worktrees.iter().any(|wt| Place { paths: vec![wt.path.clone()], folder: wt.folder }.is(path, id))
    };
    let mut snaps =
        (0..n).filter_map(|j| if j == st.active_tab { st.snap.as_ref() } else { st.tabs[j].parked.as_ref().and_then(|p| p.snap.as_ref()) });
    let Some(group) = snaps.find(|s| lists(s)).map(|s| &s.common) else { return n };
    st.tabs.iter().rposition(|t| t.group.as_ref().is_some_and(|g| g.same(group))).map_or(n, |last| last + 1)
}

/// Inserts `tab` at index `at`, keeping the active tab and the tab to return to on the same tabs.
pub(super) fn insert_tab(st: &mut AppState, at: usize, tab: Tab) {
    st.tabs.insert(at, tab);
    if st.active_tab != usize::MAX && st.active_tab >= at {
        st.active_tab += 1;
    }
    st.tab_return = st.tab_return.map(|r| if r >= at { r + 1 } else { r });
}

/// Puts the tabs in `order` (`order[new] == old`). The active tab stays active and keeps its
/// data in `AppState`; every other tab's parked data moves with it. Returns whether anything
/// moved.
pub(super) fn apply_order(st: &mut AppState, order: &[usize]) -> bool {
    if is_identity(order) || order.len() != st.tabs.len() {
        return false;
    }
    let mut old: Vec<Option<Tab>> = std::mem::take(&mut st.tabs).into_iter().map(Some).collect();
    st.tabs = order.iter().filter_map(|&i| old.get_mut(i).and_then(Option::take)).collect();
    st.active_tab = new_index(order, st.active_tab);
    st.tab_return = st.tab_return.map(|r| new_index(order, r));
    true
}

/// Tab `from` was dropped on slot `to` (`tab_groups::move_order`).
pub(super) fn moved(st: &mut AppState, from: usize, to: usize, fx: &mut Vec<Effect>) {
    let n = st.tabs.len();
    if from >= n || to >= n {
        return;
    }
    let order = move_order(&blocks(&st.tabs, st.settings.group_worktree_tabs), from, to);
    if apply_order(st, &order) {
        fx.push(super::tabs::save(st));
    }
}

/// The active tab's repository just opened: records its group and, with grouping on, moves the
/// tab next to the other tabs of that group (when it opened where its group wasn't known yet).
pub(super) fn loaded(st: &mut AppState) {
    let i = st.active_tab;
    let (Some(snap), Some(tab)) = (&st.snap, st.tabs.get_mut(i)) else { return };
    tab.group = Some(snap.common.clone());
    if !st.settings.group_worktree_tabs {
        return;
    }
    let members: Vec<usize> = (0..st.tabs.len()).filter(|&j| j != i && same_group(&st.tabs[i], &st.tabs[j])).collect();
    let (Some(&first), Some(&last)) = (members.first(), members.last()) else { return };
    let (lo, hi) = (first.min(i), last.max(i));
    if hi - lo == members.len() {
        return; // already side by side
    }
    let to = if i < last { last } else { last + 1 };
    let mut order: Vec<usize> = (0..st.tabs.len()).collect();
    let tab = order.remove(i);
    order.insert(to, tab);
    apply_order(st, &order);
}

/// `Effect::ReadTabGroups` answered: records each restored tab's group (one found by a load
/// wins), then pulls the groups together.
pub(super) fn read(st: &mut AppState, results: Vec<(PathBuf, Option<CommonDir>)>, fx: &mut Vec<Effect>) {
    for (path, common) in results {
        let Some(common) = common else { continue };
        for t in st.tabs.iter_mut().filter(|t| t.group.is_none() && t.is(&path)) {
            t.group = Some(common.clone());
        }
    }
    regroup(st, fx);
}

/// With grouping on, brings every group's tabs together (each where its leftmost tab is) and
/// saves the session. Does nothing when they already are.
pub(super) fn regroup(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.settings.group_worktree_tabs && apply_order(st, &grouped_order(&st.tabs)) {
        fx.push(super::tabs::save(st));
    }
}

/// The last session's tabs were restored: asks for the group of each not loaded yet.
pub(super) fn restored(st: &AppState, fx: &mut Vec<Effect>) {
    let paths: Vec<PathBuf> = st.tabs.iter().filter(|t| t.group.is_none()).filter_map(|t| t.path.clone()).collect();
    if !paths.is_empty() {
        fx.push(Effect::ReadTabGroups(paths));
    }
}
