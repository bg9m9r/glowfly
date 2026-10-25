//! Loading more of the history. Loading more resumes the walk (`LoadKind::More`): while the refs
//! are as loaded, the runner reads only the next commits (`LoadMsg::HistoryExtended`) and they are
//! taken in below the rows on screen; otherwise it is a full refresh at the larger count. A failed
//! load for more is kept for the graph's row and the no-matches pane, tried again by itself twice
//! (`AUTO_RETRY_DELAYS`, driven by `Msg::Tick`), and at once by Try again; trying again is a load
//! for more with `extra_commits` unchanged, and any load that starts meanwhile is that same load.

use crate::effect::Effect;
use crate::state::AppState;
use crate::state::load_more::retry_delay;
use crate::state::refresh::large;
use glowfly_core::repo::HistoryPage;
use std::time::Instant;

/// A load that asked for more history failed with `error` (the snapshot is unchanged): keeps
/// it and schedules the next automatic retry, if any are left. Only the first failure since
/// the last success is announced.
pub(super) fn failed(st: &mut AppState, error: String, now: Instant) {
    if st.refresh.more_retries == 0 {
        st.log(false, error.clone());
    } else {
        st.log_quiet(false, error.clone());
    }
    st.refresh.more_retry_at = retry_delay(st.refresh.more_retries).map(|d| now + d);
    st.refresh.more_failed = Some(error);
}

/// Try again: loads the held failed load again at once, with the same commit count. It isn't
/// counted as an automatic retry.
pub(super) fn retry(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.refresh.more_failed.is_none() || st.snap.is_none() || st.loading {
        return;
    }
    st.refresh.more_retry_at = None;
    super::refresh::more(st, fx);
}

/// The automatic retry, when it is due and the load is still wanted.
pub(super) fn tick(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    if !st.refresh.more_retry_at.is_some_and(|at| now >= at) {
        return;
    }
    st.refresh.more_retry_at = None;
    if st.loading || !st.loading_more() {
        return;
    }
    st.refresh.more_retries += 1;
    super::refresh::more(st, fx);
}

/// A load for more of generation `generation` went on from the walk: takes the next commits in
/// below the rows on screen (the selection, scroll and search cursor stay), or holds the failure.
pub(super) fn extended(st: &mut AppState, generation: u64, result: Result<Box<HistoryPage>, String>, now: Instant, fx: &mut Vec<Effect>) {
    if generation != st.load_gen {
        return; // superseded by a newer load
    }
    st.loading = false;
    let status = std::mem::take(&mut st.refresh.status_after);
    match result {
        Err(e) => failed(st, e, now),
        Ok(page) => {
            st.refresh.more_retries = 0;
            // A worker's layout asked for before the page no longer fits: ask again.
            let pending = st.refresh.relayout.is_some();
            let Some(mut snap) = st.snap.take() else { return };
            let took = snap.extend_history(*page);
            let ok = took.is_ok();
            if let Ok(old) = took
                && large(&snap)
            {
                super::large_graph::free(fx, "commits", Box::new(old));
            }
            st.snap = Some(snap);
            if !ok {
                // Not this snapshot's history (the generation should rule that out): load it all.
                super::refresh::reload_after(st, fx);
                return;
            }
            st.view.ref_rows.reset();
            st.view.highlight = None;
            if pending {
                super::large_graph::relayout(st, fx);
            }
            super::forge::resolve_pr_heads(st);
            super::search::snapshot_changed(st, fx);
        }
    }
    if status {
        super::refresh::refresh_status(st, fx);
    }
    super::refresh::load_finished(st, fx);
}
