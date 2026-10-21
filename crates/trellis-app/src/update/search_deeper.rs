//! A filtering search that loads further back: while the matches don't fill the graph, older
//! history is loaded in batches that double the commits asked for (so the total walked stays
//! about twice the final size), one load at a time and at most `DEEPER_BATCHES` per query. The
//! next batch is decided here when something changed (a query, a load, a path search), never
//! by the view, and only while the preference to load more by itself is on.

use crate::effect::Effect;
use crate::state::AppState;
use crate::state::load_more::{DEEPER_BATCHES, DeeperSearch};
use crate::state::refresh::can_load_more;

/// Asks for the next batch of older history when the filtering search needs one. Dropping the
/// filter (or its query) drops the search; a load already running finishes and is kept.
pub(super) fn pump(st: &mut AppState, fx: &mut Vec<Effect>) {
    if !st.filtering() {
        st.refresh.deeper = None;
        return;
    }
    // The matches aren't final while path terms are searched, and only one load runs at a time.
    if !st.settings.load_more_on_scroll
        || st.search.searching_files()
        || st.loading_more()
        || st.refresh.more_failed.is_some()
        || st.snap.is_none()
    {
        return;
    }
    let enough = st.search.matches.len() >= st.fill_rows();
    let more = st.snap.as_ref().is_some_and(can_load_more);
    let query = &st.search.query;
    let deeper = match &mut st.refresh.deeper {
        Some(d) if d.query == *query => d,
        // A new query starts a fresh budget; the old query's goes no further.
        slot => slot.insert(DeeperSearch { query: query.clone(), batches: 0 }),
    };
    if enough || !more || deeper.batches >= DEEPER_BATCHES {
        return;
    }
    deeper.batches += 1;
    super::refresh::more_commits(st, st.commits_wanted(), fx);
}

/// Keep searching: a fresh batch budget for the current query. Without the preference, or
/// without a filtering search, it loads more (or tries a failed load again) as Load more does.
pub(super) fn further(st: &mut AppState, fx: &mut Vec<Effect>) {
    if !st.settings.load_more_on_scroll || !st.filtering() {
        if st.refresh.more_failed.is_some() {
            super::load_more::retry(st, fx);
        } else if st.snap.as_ref().is_some_and(can_load_more) {
            super::refresh::more_commits(st, st.settings.commit_limit(), fx);
        }
        return;
    }
    st.refresh.deeper = Some(DeeperSearch { query: st.search.query.clone(), batches: 0 });
    pump(st, fx);
}
