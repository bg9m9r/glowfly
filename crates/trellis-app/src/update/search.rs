//! The graph's search bar: parsing the query, its matches over the loaded history, stepping
//! through them, filtering the graph to them, and the author picker. All of it scans the
//! loaded commits in memory, so nothing here needs an effect.

use crate::effect::Effect;
use crate::msg::SearchMsg;
use crate::state::search::GraphRows;
use crate::state::{AppState, Selection};
use git2::Oid;
use trellis_core::commit_query::{self, CommitQuery};

pub(super) fn handle(st: &mut AppState, msg: SearchMsg, fx: &mut Vec<Effect>) {
    match msg {
        SearchMsg::QueryChanged => query_changed(st),
        SearchMsg::Next => next(st, fx),
        SearchMsg::ToggleFilter => {
            st.search.filter = !st.search.filter;
            keep_selection_in_view(st);
        }
        SearchMsg::Close => {
            st.view.show_search = false;
            st.view.search.clear();
            st.search.filter = false;
            recompute(st);
        }
        SearchMsg::WantAuthors => {
            if st.search.authors.is_none()
                && let Some(snap) = &st.snap
            {
                st.search.authors = Some(commit_query::authors(&snap.commits));
            }
        }
        SearchMsg::PickAuthor(author) => {
            st.view.search = commit_query::with_author(&st.view.search, author.as_deref());
            st.view.author_filter.clear();
            query_changed(st);
        }
    }
}

fn query_changed(st: &mut AppState) {
    recompute(st);
    // The next Enter lands on the first (newest) match.
    st.search.cursor = st.snap.as_ref().map_or(0, |s| s.commits.len().saturating_sub(1));
    if st.search.filter {
        keep_selection_in_view(st);
    }
}

/// Parses the query and finds its matches in the loaded history.
fn recompute(st: &mut AppState) {
    st.search.query = CommitQuery::parse(&st.view.search);
    st.search.matches = st.snap.as_ref().map(|s| st.search.query.matching_rows(&s.commits)).unwrap_or_default();
}

/// A new history was loaded: the same query is matched against it, and the author list is
/// worked out again when the picker next opens.
pub(super) fn snapshot_changed(st: &mut AppState) {
    recompute(st);
    st.search.authors = None;
    let last = st.snap.as_ref().map_or(0, |s| s.commits.len().saturating_sub(1));
    st.search.cursor = st.search.cursor.min(last);
}

/// Enter: selects the first match after the one Enter last landed on, wrapping to the newest.
fn next(st: &mut AppState, fx: &mut Vec<Effect>) {
    if !st.search.active() {
        return;
    }
    let cursor = st.search.cursor;
    let found = st.search.matches.iter().copied().find(|&m| m > cursor).or_else(|| st.search.matches.first().copied());
    let oid = found.and_then(|i| st.snap.as_ref()?.commits.get(i).map(|c| (i, c.oid)));
    match oid {
        Some((i, oid)) => {
            st.search.cursor = i;
            super::repo::select_commit(st, oid, true, fx);
        }
        None => {
            let q = st.view.search.trim().to_owned();
            st.log(false, format!("No commits match \"{q}\""));
        }
    }
}

/// The selected commit (the focus of a multi-selection).
fn selected_oid(sel: &Selection) -> Option<Oid> {
    match sel {
        Selection::Commit(o) => Some(*o),
        Selection::Commits(m) => Some(m.focus()),
        Selection::Wip | Selection::None => None,
    }
}

/// After the shown rows changed: scrolls so the selected commit stays in view, or to the top
/// when it isn't shown.
fn keep_selection_in_view(st: &mut AppState) {
    let Some(snap) = &st.snap else { return };
    let rows = GraphRows::new(snap, &st.search, st.view.show_search);
    let row = selected_oid(&st.selection).and_then(|o| rows.row_of(o));
    st.view.scroll_to_row = Some(row.map_or(0, |r| r.saturating_sub(4)));
}

/// The graph row to scroll to for showing `oid`. A commit the filter hides turns the filter off
/// (the query stays, so the other rows dim instead).
pub(super) fn reveal(st: &mut AppState, oid: Oid) -> Option<usize> {
    let snap = st.snap.as_ref()?;
    let rows = GraphRows::new(snap, &st.search, st.view.show_search);
    if let Some(row) = rows.row_of(oid) {
        return Some(row);
    }
    if !rows.filtered() || !snap.row_of.contains_key(&oid) {
        return None;
    }
    st.search.filter = false;
    GraphRows::new(snap, &st.search, st.view.show_search).row_of(oid)
}
