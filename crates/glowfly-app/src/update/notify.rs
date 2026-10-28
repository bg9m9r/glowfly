//! The "N commits behind" alert: an auto-fetch in the active tab records how far behind the
//! checked-out branch was (`fetched`), the reload after it compares (`loaded`), and the toast's
//! Pull button comes back as `Msg::ToastAction` (`toast_action`). The LFS download that may
//! follow an auto-fetch leaves the watch alone.
//!
//! Toasts: the pointer over one holds it (`NotifyMsg::HoldToast`, state/toasts.rs) and its x
//! closes it without running its action (`NotifyMsg::CloseToast`).
//!
//! Desktop notifications: while the setting is on and the window is in the background, behind
//! alerts and finished jobs also go to the OS (`desktop`); turning the setting on sends one to
//! confirm (`set_desktop`), and a refusal comes back to `desktop_failed`. A click on one brings
//! the window forward and shows the repository it was about (`desktop_clicked`).

use crate::effect::{DesktopNote, Effect, LoadKind, NoteKind, UiEffect};
use crate::msg::{Msg, NotifyMsg};
use crate::state::{AppState, ToastAction, notify};
use glowfly_core::paths::same_spelling;
use glowfly_core::repo::Snapshot;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub(super) use crate::state::notify::{AUTO_FETCH, AUTO_FETCH_LFS};

pub(super) fn handle(st: &mut AppState, msg: NotifyMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        NotifyMsg::HoldToast(id) => st.hold_toast(id, now),
        NotifyMsg::CloseToast(id) => st.toasts.retain(|t| t.id != id),
        NotifyMsg::DesktopClicked { workdir } => desktop_clicked(st, workdir, now, fx),
    }
}

/// A desktop notification was clicked: brings the window forward and shows the repository it
/// was about, switching to its tab or opening it again (`tabs::open`). Under a dialog or
/// Preferences the tab doesn't change; a toast offers to show it once that is closed.
fn desktop_clicked(st: &mut AppState, workdir: Option<PathBuf>, now: Instant, fx: &mut Vec<Effect>) {
    fx.push(Effect::Ui(UiEffect::FocusWindow));
    let Some(dir) = workdir else { return };
    if super::deep_link::busy(st) {
        let text = format!("Close the open dialog to show {}.", repo_label(st, Some(&dir)));
        let msg = Box::new(Msg::Notify(NotifyMsg::DesktopClicked { workdir: Some(dir) }));
        st.toast_action(true, text, ToastAction { label: "Show".into(), msg, workdir: None }, now);
        return;
    }
    // Already showing: no reload, only out of the dashboard.
    if st.snap.is_some() && st.tab_showing(&dir) == Some(st.active_tab) {
        return super::dashboard::leave(st);
    }
    super::tabs::open(st, dir, fx);
}

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

/// A toast's action button (or, with `details`, its Details beside a fix) was clicked: removes
/// the toast and runs that button's message, unless it is for a repository whose tab is no
/// longer active.
pub(super) fn toast_action(st: &mut AppState, id: u64, details: bool, now: Instant, fx: &mut Vec<Effect>) {
    let Some(i) = st.toasts.iter().position(|t| t.id == id) else { return };
    let t = st.toasts.remove(i);
    let Some(action) = (if details { t.details } else { t.action }) else { return };
    run_action(st, action, now, fx);
}

/// Whether `action` may run now: it names no repository, or the one the active tab shows.
/// Otherwise a toast says to switch back first.
pub(super) fn may_run(st: &mut AppState, action: &ToastAction, now: Instant) -> bool {
    if let Some(dir) = &action.workdir
        && !st.workdir().is_some_and(|w| same_spelling(dir, &w))
    {
        st.toast(false, format!("{} wasn't run: switch back to that repository's tab first.", action.label), now);
        return false;
    }
    true
}

/// Runs a toast's (or a failed run's) action when `may_run` allows it; returns whether it ran.
pub(super) fn run_action(st: &mut AppState, action: ToastAction, now: Instant, fx: &mut Vec<Effect>) -> bool {
    if !may_run(st, &action, now) {
        return false;
    }
    fx.extend(super::update_sealed(st, *action.msg, now));
    true
}

/// Also shows `title` as a desktop notification, with the repository's name as its body, when
/// the setting is on and the window is in the background.
pub(super) fn desktop(st: &AppState, kind: NoteKind, title: &str, workdir: Option<&Path>, fx: &mut Vec<Effect>) {
    if !st.settings.desktop_notifications || st.view.focused {
        return;
    }
    fx.push(Effect::Notify(notify::desktop_note(kind, title, &repo_label(st, workdir), workdir)));
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
