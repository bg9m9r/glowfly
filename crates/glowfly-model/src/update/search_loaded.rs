//! Matching a typed query against a large loaded history without holding up frames: typing waits
//! for a pause (`TYPING_PAUSE`, driven by `Msg::Tick`) before matching, and the match runs on the
//! query worker (`IndexEffect::MatchLoaded`). Its answer is applied only while it is the newest
//! request: a newer query, a sync match (`search::recompute`) or a new history drops it. Smaller
//! histories, and queries with path terms, are matched at once on the UI thread.

use crate::effect::{Effect, IndexEffect, Shared};
use crate::state::AppState;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How long typing must pause before a large history is matched.
pub const TYPING_PAUSE: Duration = Duration::from_millis(80);

/// From `LARGE_HISTORY` loaded commits a typed query waits for a pause and is matched off the UI
/// thread.
fn large(st: &AppState) -> bool {
    st.snap.as_ref().is_some_and(crate::state::refresh::large)
}

/// The view edited the query: a large history is matched after a pause, a small one at once.
pub(super) fn typed(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    if large(st) {
        st.session.search.loaded.due = Some(now + TYPING_PAUSE);
    } else {
        super::search::query_changed(st, false, fx);
    }
}

/// A frame at `t`: matches a typed query whose pause is over.
pub(super) fn tick(st: &mut AppState, t: Instant, fx: &mut Vec<Effect>) {
    if st.session.search.loaded.due.is_some_and(|due| t >= due) {
        st.session.search.loaded.due = None;
        super::search::query_changed(st, false, fx);
    }
}

/// Before Enter steps through the matches: a query still waiting for its pause or its answer is
/// matched now, so Enter lands on one of its matches.
pub(super) fn flush(st: &mut AppState, fx: &mut Vec<Effect>) {
    let due = st.session.search.loaded.due.take().is_some();
    if due || st.session.search.loaded.pending.is_some() {
        super::search::query_changed(st, true, fx);
    }
}

/// Asks the query worker for the parsed query's matches in a large loaded history; false (match
/// on the UI thread) for a small history, an empty query or one with path terms.
pub(super) fn ask(st: &mut AppState, fx: &mut Vec<Effect>) -> bool {
    let q = &st.session.search.query;
    let Some(snap) = st.snap.as_ref().filter(|_| large(st) && !q.is_empty() && !q.has_paths()) else { return false };
    let l = &mut st.session.search.loaded;
    l.requests += 1;
    l.pending = Some(l.requests);
    let commits = Shared(Arc::clone(&snap.commits));
    fx.push(Effect::Index(IndexEffect::MatchLoaded { workdir: snap.workdir.clone(), request: l.requests, commits, query: q.clone() }));
    true
}

/// The tab was shown again: a match asked for while it was active had its answer dropped, so it
/// is asked for again.
pub(super) fn resume(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.session.search.loaded.pending.is_some() && !ask(st, fx) {
        super::search::recompute(st);
    }
}

/// The worker's answer to request `request`: the matches when it is still the one asked for.
pub(super) fn matched(st: &mut AppState, request: u64, result: Result<Vec<usize>, String>, fx: &mut Vec<Effect>) {
    if st.session.search.loaded.pending != Some(request) {
        return;
    }
    match result {
        Ok(matches) => {
            st.session.search.loaded.pending = None;
            st.session.search.matches = matches;
        }
        Err(e) => {
            tracing::warn!("matching the loaded history off the UI thread failed: {e}");
            super::search::recompute(st);
        }
    }
    super::search::matches_changed(st, fx);
}
