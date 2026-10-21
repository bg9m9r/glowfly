//! Live working-tree updates: keeping the file watcher on the active tab's repository, and
//! refreshing when it reports changes made outside Trellis.

use crate::effect::{Effect, WatchTarget};
use crate::keymap::{BindingId, Keymap};
use crate::msg::WatchMsg;
use crate::state::AppState;
use crate::state::watch::WATCH_SETTLE;
use std::time::Instant;

/// How the user refreshes by hand: "press F5" with Refresh's current key, else "choose Refresh".
pub(super) fn refresh_by_hand(keys: &Keymap) -> String {
    keys.hint(BindingId::Refresh).map_or_else(|| "choose Refresh".to_owned(), |k| format!("press {k}"))
}

/// What Trellis says when it can't watch a repository.
pub(super) fn failed_text(keys: &Keymap) -> String {
    format!(
        "Trellis can't watch this repository for changes, so changes made outside Trellis show up when its window regains focus or you {}.",
        refresh_by_hand(keys)
    )
}

pub(super) fn handle(st: &mut AppState, msg: WatchMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        // Only arrives through `Msg::Scoped`, so it is for the active tab.
        WatchMsg::RepoChanged { full, paths, last_event } => {
            if !st.settings.watch_files || st.snap.is_none() {
                return;
            }
            // Trellis' own job refreshes when it is done.
            let here = st.workdir();
            if let (Some(job), Some(here)) = (st.job.as_ref().and_then(|j| j.workdir.as_deref()), here.as_deref())
                && trellis_core::paths::same_spelling(job, here)
            {
                return;
            }
            if st.watch.quiet_until.is_some_and(|q| last_event <= q) {
                return; // caused by a job or local op that already refreshed
            }
            // A full refresh reads the commands files anyway.
            if !full {
                super::custom_commands::watched(st, paths.as_deref(), fx);
            }
            match (full, paths) {
                (true, _) => super::repo::refresh(st, fx),
                // Only these work-tree paths changed: re-read just their rows.
                (false, Some(paths)) => super::refresh_paths::refresh_paths(st, paths, fx),
                (false, None) => super::repo::refresh_status(st, fx),
            }
        }
        WatchMsg::RemoteVolume { workdir, kind } => {
            if st.watch.remote.insert(workdir.clone(), kind).is_none() {
                let text = remote_text(&workdir, kind, &st.keymap);
                st.toast(true, text.clone(), now);
                st.log_quiet(true, text);
            }
        }
        WatchMsg::WatchFailed { workdir, error } => {
            tracing::warn!("file watching failed for {}: {error}", workdir.display());
            // Reported once per repository; it is retried (silently) when its tab is selected again.
            if st.watch.failed.insert(workdir.clone()) {
                st.toast(false, failed_text(&st.keymap), now);
                // Quiet: the toast above says it, so this shouldn't announce a failure.
                st.log_quiet(false, format!("File watching stopped for {}: {error}", trellis_core::native_path::shown(&workdir)));
            }
        }
    }
}

/// Points the watcher at the active tab's repository, or stops it (no repository, or the
/// preference is off). Does nothing when it already watches the right one.
pub(super) fn sync(st: &mut AppState, fx: &mut Vec<Effect>) {
    let want = if st.settings.watch_files {
        st.snap.as_ref().map(|s| WatchTarget { workdir: s.workdir.clone(), git_dir: s.git_dir.clone() })
    } else {
        None
    };
    let workdir = want.as_ref().map(|t| t.workdir.clone());
    if workdir != st.watch.target {
        st.watch.target = workdir;
        st.watch.quiet_until = None;
        fx.push(Effect::Watch(want));
    }
}

/// What Trellis says about a repository on a network or virtual file system.
pub(super) fn remote_text(workdir: &std::path::Path, kind: &str, keys: &Keymap) -> String {
    let secs = crate::state::watch::REMOTE_POLL.as_secs();
    format!(
        "{} is on {kind}, which may not report changes made from another computer or system. Trellis re-reads it every {secs} seconds while its window is focused, when the window regains focus and when you {}.",
        trellis_core::native_path::shown(workdir),
        refresh_by_hand(keys)
    )
}

/// On the timer `poll_every` sets (a network or virtual file system, or Windows), re-reads the
/// status while the window is focused and nothing else is reading or changing the repository.
pub(super) fn poll(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    let Some(every) = st.watch.poll_interval() else { return };
    if !st.view.was_focused || st.snap.is_none() || st.job.is_some() || st.loading {
        return;
    }
    match st.watch.last_poll {
        Some(last) if now.saturating_duration_since(last) < every => {}
        Some(_) => {
            st.watch.last_poll = Some(now);
            super::repo::refresh_status(st, fx);
        }
        None => st.watch.last_poll = Some(now),
    }
}

/// Trellis just changed the repository itself (and refreshes): events up to now are its own.
pub(super) fn settled(st: &mut AppState, now: Instant) {
    st.watch.quiet_until = Some(now + WATCH_SETTLE);
}
