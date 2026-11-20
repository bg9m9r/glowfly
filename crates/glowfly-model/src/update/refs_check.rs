//! Refreshing after a background fetch: auto-fetch and its LFS download move at most
//! remote-tracking refs, so with the watcher live and nothing held back during the job only
//! `refs_key` is compared (`Depth::Refs`, `LoadMsg::RefsUnchanged`), with no status read or
//! worktree recount; a changed key loads in full, and any other job refreshes as before.

use crate::effect::Effect;
use crate::state::notify::{AUTO_FETCH, AUTO_FETCH_LFS};
use crate::state::{AppState, RunningJob};
use std::time::Instant;

/// A job ended in the active tab: refreshes it, with only a refs check when a background fetch
/// ended while the file watcher was live and reported nothing meanwhile (a fetch writes neither
/// the work tree nor the index, and the watcher would have said if anything else did), else in
/// full.
pub(super) fn after_job(st: &mut AppState, job: &RunningJob, now: Instant, fx: &mut Vec<Effect>) {
    let held = std::mem::take(&mut st.watch.held);
    let live = st.settings.watch_files && st.workdir().is_some_and(|w| st.watch.live_for(&w));
    if fetch_only(job) && live && !held {
        super::watch::settled_refs(st, now);
        super::refresh::check_refs(st, fx);
    } else {
        super::watch::settled(st, now);
        super::repo::refresh(st, fx);
    }
}

/// A quiet background fetch: network only, it never writes the work tree or the index.
fn fetch_only(job: &RunningJob) -> bool {
    job.quiet && (job.label == AUTO_FETCH || job.label == AUTO_FETCH_LFS)
}

/// A refs-only check found refs, HEAD, config and worktrees as the snapshot has them: what a
/// load that reads nothing else finishes with.
pub(super) fn unchanged(st: &mut AppState, generation: u64, now: Instant, fx: &mut Vec<Effect>) {
    if generation != st.load_gen {
        return; // superseded by a newer load
    }
    st.loading = false;
    st.refresh.more_retries = 0;
    super::notify::unchanged(st);
    super::refresh::reask_files(st, fx);
    // The stash list is current: the refs are unchanged.
    super::auto_stash::seen(st, true, now, fx);
    super::trust::maybe_ask(st, false);
    super::forge::refresh(st, false, false, now, fx);
    super::push_pr::loaded(st, fx);
    // A status read this check dropped is read once, whole.
    if std::mem::take(&mut st.refresh.status_after) {
        super::refresh::refresh_status(st, fx);
    }
    super::refresh::load_finished(st, fx);
}
