//! The "N commits behind" alert: an auto-fetch in the active tab records how far behind the
//! checked-out branch was (`fetched`), the reload after it compares (`loaded`), and the toast's
//! Pull button comes back as `Msg::ToastAction` (`toast_action`). The LFS download that may
//! follow an auto-fetch leaves the watch alone.
//!
//! Desktop notifications: while the setting is on and the window is in the background, behind
//! alerts and finished jobs also go to the OS (`desktop`); turning the setting on sends one to
//! confirm (`set_desktop`), and a refusal comes back to `desktop_failed`.

use crate::effect::{DesktopNote, Effect, LoadKind, NoteKind};
use crate::msg::Msg;
use crate::state::{AppState, ToastAction, notify};
use std::path::Path;
use std::time::Instant;
use trellis_core::paths::same_spelling;
use trellis_core::repo::Snapshot;

/// The background fetch's job label (update/ops.rs `auto_fetch`).
pub(super) const AUTO_FETCH: &str = "Auto-fetch";

/// The LFS download started after a successful auto-fetch (update/lfs.rs).
pub(super) const AUTO_FETCH_LFS: &str = "Auto-fetch LFS files";

/// A job in the active tab ended, before its refresh is requested. A successful auto-fetch
/// records the baseline; any other job clears an older one.
pub(super) fn fetched(st: &mut AppState, label: &str, quiet: bool, ok: bool) {
    // Downloading LFS files moves no refs, so the watch stays for the reload after the fetch.
    if quiet && label == AUTO_FETCH_LFS {
        return;
    }
    let watch = ok && quiet && label == AUTO_FETCH && st.settings.behind_alerts;
    st.behind_watch = watch.then(|| st.snap.as_ref().and_then(|s| notify::baseline(s, st.load_gen))).flatten();
}

/// A repository load landed (before it replaces `st.snap`). The first load started after the
/// fetch ended takes the watch and toasts when the branch fell further behind.
pub(super) fn loaded(st: &mut AppState, snap: &Snapshot, kind: LoadKind, now: Instant, fx: &mut Vec<Effect>) {
    let generation = st.load_gen;
    // A load already in flight when the fetch ended leaves the watch for the queued refresh.
    let Some(w) = st.behind_watch.take_if(|w| generation > w.after_gen) else { return };
    if kind != LoadKind::Refresh || !same_spelling(&w.workdir, &snap.workdir) || !st.settings.behind_alerts {
        return;
    }
    if let Some(text) = notify::alert_text(&w, snap.head_info()) {
        desktop(st, NoteKind::BehindAlert, &text, Some(&snap.workdir), fx);
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

/// Also shows `title` as a desktop notification, with the repository's name as its body, when
/// the setting is on and the window is in the background.
pub(super) fn desktop(st: &AppState, kind: NoteKind, title: &str, workdir: Option<&Path>, fx: &mut Vec<Effect>) {
    if !st.settings.desktop_notifications || st.view.focused {
        return;
    }
    fx.push(Effect::Notify(notify::desktop_note(kind, title, &repo_label(st, workdir))));
}

/// The name of the tab showing `workdir`, else its folder name, else nothing.
fn repo_label(st: &AppState, workdir: Option<&Path>) -> String {
    let Some(w) = workdir else { return String::new() };
    if let Some(t) = st.tabs.iter().find(|t| t.is(w)) {
        return t.name.clone();
    }
    w.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The Desktop notifications checkbox: saves a change, and turning them on sends one
/// notification (whatever the focus) so the user sees whether they arrive.
pub(super) fn set_desktop(st: &mut AppState, on: bool, fx: &mut Vec<Effect>) {
    if st.settings.desktop_notifications == on {
        return;
    }
    st.settings.desktop_notifications = on;
    fx.push(super::prefs::save_settings(st));
    if on {
        fx.push(Effect::Notify(DesktopNote::confirmation()));
    }
}

/// The OS refused a desktop notification. The confirmation's failure is toasted (the user is
/// looking); later ones are only logged, quietly: the user is away and they would repeat.
pub(super) fn desktop_failed(st: &mut AppState, text: String, confirm: bool, now: Instant) {
    let line = format!("Couldn't show a desktop notification: {text}");
    st.log_quiet(false, line.clone());
    if confirm {
        st.toast(false, line, now);
    }
}
