//! The "N commits behind" alert: an auto-fetch in the active tab records how far behind the
//! checked-out branch was (`fetched`), the reload after it compares (`loaded`), and the toast's
//! Pull button comes back as `Msg::ToastAction` (`toast_action`).

use crate::effect::{Effect, LoadKind};
use crate::msg::Msg;
use crate::state::{AppState, ToastAction, notify};
use std::time::Instant;
use trellis_core::paths::same_spelling;
use trellis_core::repo::Snapshot;

/// The background fetch's job label (update/ops.rs `auto_fetch`).
pub(super) const AUTO_FETCH: &str = "Auto-fetch";

/// A job in the active tab ended, before its refresh is requested. A successful auto-fetch
/// records the baseline; any other job clears an older one.
pub(super) fn fetched(st: &mut AppState, label: &str, quiet: bool, ok: bool) {
    let watch = ok && quiet && label == AUTO_FETCH && st.settings.behind_alerts;
    st.behind_watch = watch.then(|| st.snap.as_ref().and_then(|s| notify::baseline(s, st.load_gen))).flatten();
}

/// A repository load landed (before it replaces `st.snap`). The first load started after the
/// fetch ended takes the watch and toasts when the branch fell further behind.
pub(super) fn loaded(st: &mut AppState, snap: &Snapshot, kind: LoadKind, now: Instant) {
    let generation = st.load_gen;
    // A load already in flight when the fetch ended leaves the watch for the queued refresh.
    let Some(w) = st.behind_watch.take_if(|w| generation > w.after_gen) else { return };
    if kind != LoadKind::Refresh || !same_spelling(&w.workdir, &snap.workdir) || !st.settings.behind_alerts {
        return;
    }
    if let Some(text) = notify::alert_text(&w, snap.head_info()) {
        let msg = Box::new(Msg::git("Pull", &["pull", "--ff"]));
        st.toast_action(true, text, ToastAction { label: "Pull".into(), msg, workdir: Some(snap.workdir.clone()) }, now);
    }
}

/// A current-generation refresh found the refs unchanged: the fetch brought nothing.
pub(super) fn unchanged(st: &mut AppState) {
    let generation = st.load_gen;
    st.behind_watch.take_if(|w| generation > w.after_gen);
}

/// A toast's action button was clicked: removes the toast and runs its message, unless the
/// action is for a repository whose tab is no longer active.
pub(super) fn toast_action(st: &mut AppState, id: u64, now: Instant, fx: &mut Vec<Effect>) {
    let Some(i) = st.toasts.iter().position(|t| t.id == id) else { return };
    let Some(action) = st.toasts.remove(i).action else { return };
    if let Some(dir) = &action.workdir
        && !st.workdir().is_some_and(|w| same_spelling(dir, &w))
    {
        st.toast(false, format!("{} wasn't run: switch back to that repository's tab first.", action.label), now);
        return;
    }
    fx.extend(super::update(st, *action.msg, now));
}
