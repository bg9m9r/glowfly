//! A failed load for more history: kept for the graph's row and the no-matches pane, tried
//! again by itself twice (`AUTO_RETRY_DELAYS`, driven by `Msg::Tick`), and at once by Try
//! again. Loading more re-walks the history with a larger cap, so trying again is a refresh
//! with `extra_commits` unchanged, and any load that starts meanwhile is that same load.

use crate::effect::Effect;
use crate::state::AppState;
use crate::state::load_more::retry_delay;
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
    super::refresh::refresh(st, fx);
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
    super::refresh::refresh(st, fx);
}
