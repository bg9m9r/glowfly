//! The activity panel's filters and keyboard cursor: the Failures only toggle, the search text
//! changing, folding a row (an unfolded match can be folded again until the search changes) and
//! the keys of the focused activity list, and the search's step from match to match (Enter and
//! Shift+Enter in the field, the header's buttons). Which rows show is decided by
//! state/activity_filter.rs.

use crate::logs::LogKind;
use crate::state::AppState;
use crate::state::activity_filter::{Filter, is_open, row_ids, step_from};
use crate::state::activity_rows::foldable;
use crate::state::list_keys::{ListKey, ListMove, Place};
use crate::state::logs::EntryId;

/// The filter the panel's toggle and search field make.
fn filter(st: &AppState) -> Filter {
    Filter::new(st.logs.failures_only, &st.view.logs_search)
}

pub(super) fn set_failures_only(st: &mut AppState, on: bool) {
    st.logs.failures_only = on;
    st.view.activity_step = false;
}

/// The search text changed: rows the user folded under the old search may unfold again.
pub(super) fn search_changed(st: &mut AppState) {
    st.logs.search_closed.clear();
    st.view.activity_step = false;
}

/// Folds an activity row (a routine group by its first entry) open, or closed again. A row the
/// search unfolded closes into `search_closed`, not out of `open`, so it stays closed while the
/// search text stays the same and no longer matters once it is cleared.
pub(super) fn fold(st: &mut AppState, id: EntryId) {
    if st.logs.step_opened == Some(id) {
        // Folded by hand: the user took the row over, the next step leaves it as it is.
        st.logs.step_opened = None;
    }
    let f = filter(st);
    let entry = st.log.iter().find(|e| e.id == id);
    let shown = entry.map_or_else(|| st.logs.open.contains(&id), |e| is_open(&st.logs, e, &f));
    if shown {
        st.logs.open.remove(&id);
        if entry.is_some_and(|e| f.unfolds(e)) {
            st.logs.search_closed.insert(id);
        }
    } else {
        st.logs.search_closed.remove(&id);
        st.logs.open.insert(id);
    }
}

/// A key of the focused activity list: Up/Down, Page and Home/End move the cursor over the shown
/// rows; Enter and Space fold the cursor row. Nothing happens unless the Activity tab shows.
pub(super) fn row_key(st: &mut AppState, key: ListKey) {
    if !st.view.show_log || st.logs.tab != LogKind::Activity {
        return;
    }
    let f = filter(st);
    let ids = row_ids(&st.log, &f);
    let at = st.view.activity_focus.and_then(|id| ids.iter().position(|i| *i == id));
    match key {
        ListKey::Move(mv) => {
            let to = match at {
                Some(i) => mv.target(ids.len(), Place::At(i)),
                None if ids.is_empty() => None,
                None => Some(if mv == (ListMove::Edge { end: false }) { 0 } else { ids.len() - 1 }),
            };
            if let Some(to) = to {
                st.view.activity_focus = Some(ids[to]);
                st.view.activity_focus_scroll = true;
            }
        }
        ListKey::Toggle | ListKey::Open => {
            let Some(i) = at else { return };
            if st.log.iter().find(|e| e.id == ids[i]).is_some_and(foldable) {
                fold(st, ids[i]);
            }
        }
        ListKey::Extend(_) | ListKey::Delete | ListKey::Across { .. } => {}
    }
}

/// Moves the keyboard cursor to the next (`back`: previous) shown row, wrapping, scrolls it to
/// the top and opens it. The row the last step opened folds closed again, unless the user took
/// it over since or this step lands on it. Nothing happens unless the Activity tab shows and a
/// filter is on.
pub(super) fn step(st: &mut AppState, back: bool) {
    if !st.view.show_log || st.logs.tab != LogKind::Activity {
        return;
    }
    let f = filter(st);
    let Some(id) = step_from(&st.log, &f, st.view.activity_focus, back) else { return };
    if let Some(prev) = st.logs.step_opened.take() {
        if prev == id {
            st.logs.step_opened = Some(prev);
        } else {
            st.logs.open.remove(&prev);
        }
    }
    if let Some(e) = st.log.iter().find(|e| e.id == id).filter(|e| foldable(e) && !is_open(&st.logs, e, &f)) {
        // `open` alone opens it; a hand fold in `search_closed` stays, so folding back restores it.
        st.logs.open.insert(id);
        st.logs.step_opened = Some(e.id);
    }
    st.view.activity_focus = Some(id);
    st.view.activity_reveal = Some(id);
    st.view.activity_step = true;
}

/// Whether the active filter leaves the entry `id` out of the shown rows (false for an entry no
/// longer in the log).
pub(super) fn hides(st: &AppState, id: EntryId) -> bool {
    let f = filter(st);
    st.log.iter().find(|e| e.id == id).is_some_and(|e| !f.keeps(e))
}

/// Whether the filter lists routine runs one by one, so no row stands for a group.
pub(super) fn lists_runs_singly(st: &AppState) -> bool {
    filter(st).is_active()
}

/// Before Details reveals run `id`: when the active filter would hide it, empties the search
/// (and turns Failures only off for an entry that isn't a failure).
pub(super) fn unhide(st: &mut AppState, id: EntryId) {
    st.logs.search_closed.remove(&id);
    let Some(e) = st.log.iter().find(|e| e.id == id) else { return };
    let f = filter(st);
    if !f.keeps(e) {
        st.view.logs_search.clear();
        st.logs.search_closed.clear();
        if !Filter::new(st.logs.failures_only, "").keeps(e) {
            st.logs.failures_only = false;
        }
    }
}

#[cfg(test)]
#[path = "tests/logs_filter_tests.rs"]
mod tests;
