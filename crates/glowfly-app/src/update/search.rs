//! The graph's search bar: parsing the query, its matches over the loaded history, stepping
//! through them, filtering the graph to them, and the author picker. Text and author terms
//! scan the loaded commits in memory (a large history off the UI thread, update/search_loaded.rs);
//! path terms need each commit's changed files, which update/search_paths.rs asks the runner for.

use crate::effect::Effect;
use crate::msg::SearchMsg;
use crate::state::graph_rows::GraphRows;
use crate::state::search::PathSearch;
use crate::state::{AppState, Selection};
use git2::Oid;
use glowfly_core::commit_query::{self, CommitQuery};
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: SearchMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        SearchMsg::Open => {
            if st.session.diff.view.is_some() {
                super::repo::close_diff(st);
            }
            st.view.show_search = true;
            st.view.focus_search = true;
        }
        SearchMsg::QueryChanged => super::search_loaded::typed(st, now, fx),
        SearchMsg::Next => {
            super::search_loaded::flush(st, fx);
            next(st, fx);
        }
        SearchMsg::ToggleFilter => {
            st.session.search.filter = !st.session.search.filter;
            keep_selection_in_view(st);
            super::search_deeper::pump(st, fx);
        }
        SearchMsg::Close => {
            st.view.show_search = false;
            st.session.tab_view.search.clear();
            st.session.search.loaded.due = None;
            st.session.search.filter = false;
            st.session.search.paths = PathSearch { requests: st.session.search.paths.requests, ..Default::default() };
            st.session.search.index.drop_answer();
            recompute(st);
            super::search_deeper::pump(st, fx);
        }
        SearchMsg::Clear => {
            st.session.tab_view.search.clear();
            st.session.search.loaded.due = None;
            st.view.author_filter.clear();
            st.session.search.paths = PathSearch { requests: st.session.search.paths.requests, ..Default::default() };
            st.session.search.index.drop_answer();
            recompute(st);
            super::search_deeper::pump(st, fx);
            st.view.focus_search = true;
        }
        SearchMsg::WantAuthors => {
            if st.session.search.authors.is_none()
                && let Some(snap) = &st.snap
            {
                st.session.search.authors = Some(commit_query::authors(&snap.commits));
            }
        }
        SearchMsg::PickAuthor(author) => {
            st.session.tab_view.search = commit_query::with_author(&st.session.tab_view.search, author.as_deref());
            st.view.author_filter.clear();
            query_changed(st, false, fx);
        }
        SearchMsg::PathsSearched { request, result } => super::search_paths::searched(st, request, result, fx),
        SearchMsg::IndexProgress { build, indexed, estimate } => super::history_index::progress(st, build, indexed, estimate),
        SearchMsg::IndexBuilt { build, result } => super::history_index::built(st, build, result, fx),
        SearchMsg::IndexQueried { request, result } => super::search_index::queried(st, request, result, fx),
        SearchMsg::LoadedMatched { request, result } => super::search_loaded::matched(st, request, result, fx),
    }
}

/// The query changed: its matches (on the query worker for a large history unless `now`), the
/// path and index answers it needs, and the next Enter's starting point.
pub(super) fn query_changed(st: &mut AppState, now: bool, fx: &mut Vec<Effect>) {
    st.session.search.query = CommitQuery::parse(&st.session.tab_view.search);
    let asked = !now && super::search_loaded::ask(st, fx);
    if !asked {
        recompute(st);
    }
    super::search_paths::want(st, fx);
    super::search_index::want(st, fx);
    // The next Enter lands on the first (newest) match.
    st.session.search.cursor = st.snap.as_ref().map_or(0, |s| s.commits.len().saturating_sub(1));
    st.session.search.stepped = false;
    st.session.search.index.jump = None;
    if !asked {
        matches_changed(st, fx);
    }
}

/// The matches changed: a filtered graph keeps the selection in view and may load further back.
pub(super) fn matches_changed(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.session.search.filter {
        keep_selection_in_view(st);
    }
    super::search_deeper::pump(st, fx);
}

/// Parses the query and finds its matches in the loaded history (path terms as far as found).
pub(super) fn recompute(st: &mut AppState) {
    st.session.search.loaded.pending = None;
    st.session.search.query = CommitQuery::parse(&st.session.tab_view.search);
    let touching = &st.session.search.paths.touching;
    st.session.search.matches = st.snap.as_ref().map(|s| st.session.search.query.matching_rows(&s.commits, touching)).unwrap_or_default();
}

/// A new history was loaded: the same query is matched against it, path answers are kept for
/// the commits still loaded (the new ones are asked for), the author list is worked out again
/// when the picker next opens, and a filtering search may load further back.
pub(super) fn snapshot_changed(st: &mut AppState, fx: &mut Vec<Effect>) {
    super::search_paths::history_changed(st);
    recompute(st);
    super::search_paths::want(st, fx);
    st.session.search.authors = None;
    let last = st.snap.as_ref().map_or(0, |s| s.commits.len().saturating_sub(1));
    st.session.search.cursor = st.session.search.cursor.min(last);
    // The history may need its index built or updated, an older match Enter asked for may be
    // loaded now, and the index is asked for the query if it changed.
    super::history_index::want_build(st, fx);
    super::search_index::landed(st, fx);
    super::search_index::want(st, fx);
    // Loaded with `st.loading` already false, so a filtering search's next batch starts at once.
    super::search_deeper::pump(st, fx);
}

/// Enter: selects the first match after the one Enter last landed on. Past the last loaded
/// match, the whole-history index (when it answered) loads the next older one; otherwise Enter
/// wraps to the newest.
fn next(st: &mut AppState, fx: &mut Vec<Effect>) {
    // While files are still searched, Enter steps through the matches found so far.
    if !st.session.search.active() || (st.session.search.searching_files() && st.session.search.matches.is_empty()) {
        return;
    }

    let cursor = st.session.search.cursor;
    let after = st.session.search.matches.iter().copied().find(|&m| m > cursor);
    // The first Enter after a query change lands on the newest match, loaded or not.
    let beyond = after.is_none() && (st.session.search.stepped || st.session.search.matches.is_empty());
    if beyond && super::search_index::next_beyond(st, fx) {
        return;
    }
    let found = after.or_else(|| st.session.search.matches.first().copied());
    let oid = found.and_then(|i| st.snap.as_ref()?.commits.get(i).map(|c| (i, c.oid)));
    match oid {
        Some((i, oid)) => {
            st.session.search.cursor = i;
            st.session.search.stepped = true;
            super::repo::select_commit(st, oid, true, fx);
        }
        None => {
            let q = st.session.tab_view.search.trim().to_owned();
            st.log(false, format!("No commits match \"{q}\""));
        }
    }
}

/// The selected commit (the focus of a multi-selection).
pub(super) fn selected_oid(sel: &Selection) -> Option<Oid> {
    match sel {
        Selection::Commit(o) => Some(*o),
        Selection::VsWorkdir(w) => Some(w.oid),
        Selection::Commits(m) => Some(m.focus()),
        Selection::Wip | Selection::None => None,
    }
}

/// After the shown rows changed: scrolls so the selected commit stays in view, or to the top
/// when it isn't shown.
fn keep_selection_in_view(st: &mut AppState) {
    let Some(snap) = &st.snap else { return };
    let rows = GraphRows::new(snap, &st.session.search, st.view.show_search);
    let row = selected_oid(&st.session.selection).and_then(|o| rows.row_of(o));
    st.view.scroll_to_row = Some(row.map_or(0, |r| r.saturating_sub(4)));
}

/// The graph row to scroll to for showing `oid`. A commit the filter hides turns the filter off
/// (the query stays, so the other rows dim instead).
pub(super) fn reveal(st: &mut AppState, oid: Oid) -> Option<usize> {
    let snap = st.snap.as_ref()?;
    let rows = GraphRows::new(snap, &st.session.search, st.view.show_search);
    if let Some(row) = rows.row_of(oid) {
        return Some(row);
    }
    if !rows.filtered() || !snap.row_of.contains_key(&oid) {
        return None;
    }
    st.session.search.filter = false;
    GraphRows::new(snap, &st.session.search, st.view.show_search).row_of(oid)
}
