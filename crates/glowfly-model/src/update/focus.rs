//! Returning to the window (`RepoMsg::WindowFocused`): picking up what changed outside Glowfly
//! without reading everything again. The status is read again unless the watcher on the
//! repository is trusted and the last whole read ended under `FOCUS_FRESH` ago (its reports kept
//! the lists current); other worktrees are counted again by their usual throttle; the identity
//! is read again only when a config file it comes from changed (the runner compares
//! `ConfigStamp`s); recent folders and git-lfs as before.

use crate::effect::Effect;
use crate::state::AppState;
use crate::state::refresh::FOCUS_FRESH;
use std::time::Instant;

/// Picks up changes made outside the app.
pub(super) fn window_focused(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    // While a job runs its end refreshes.
    if st.job.is_none() {
        if lists_current(st, now) {
            tracing::debug!("window focused: the watcher kept the status current");
            // Not forced: the interval since the last count applies, and one that is due waits
            // for the tick.
            super::worktree_counts::recount(st, now, fx);
        } else {
            super::refresh::refresh(st, fx);
        }
    }
    super::recent::check_folders(st, fx);
    super::profile_rules::check_identity(st, fx);
    super::lfs::window_focused(st, now, fx);
}

/// Whether the active repository's lists are current at `now` without a read: a repository is
/// open with its file watcher on, trusted (`WatchState::trusted_for`) and reporting since the last
/// whole status read ended less than `FOCUS_FRESH` ago, and its history wasn't freed while the
/// tab was parked.
pub(super) fn lists_current(st: &AppState, now: Instant) -> bool {
    let Some(snap) = st.snap.as_ref() else { return false };
    st.settings.watch_files
        && st.session.tab_view.history_freed.is_none()
        && st.watch.trusted_for(&snap.workdir)
        && st.refresh.status_read_at.is_some_and(|at| now.saturating_duration_since(at) < FOCUS_FRESH)
}

/// A whole status read of the active tab ended at `now` (`ok`: it was read; a failure makes the
/// next focus read again).
pub(super) fn whole_read(st: &mut AppState, ok: bool, now: Instant) {
    st.refresh.status_read_at = ok.then_some(now);
}
