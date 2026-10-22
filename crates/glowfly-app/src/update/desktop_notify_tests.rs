//! Headless tests for desktop notifications (update/notify.rs): the setting and its
//! confirmation, finished jobs and behind alerts while the window is in the background, and
//! what a refusal from the OS does.

use super::branch_tests::{remote_commit, with_origin};
use super::tests::{drive, finish_keeping, same_place, scratch_repo, state};
use super::update;
use crate::effect::{DesktopNote, Effect, NoteKind};
use crate::msg::{JobMsg, Msg, PrefsMsg, RepoMsg};
use crate::state::AppState;
use std::path::Path;
use std::time::{Duration, Instant};

fn notes(fx: &[Effect]) -> Vec<&DesktopNote> {
    fx.iter().filter_map(|e| if let Effect::Notify(n) = e { Some(n) } else { None }).collect()
}

fn saves(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::SaveSettings(_))).count()
}

fn set(st: &mut AppState, on: bool) -> Vec<Effect> {
    update(st, Msg::Prefs(PrefsMsg::SetDesktopNotifications(on)), Instant::now())
}

fn tab_name(st: &AppState) -> String {
    st.tabs[st.active_tab].name.clone()
}

/// Ends a Fetch job (its effects are not run) with `ok` and `output`; returns the effects.
fn fetch_ends(st: &mut AppState, ok: bool, output: &str) -> Vec<Effect> {
    let fx = update(st, Msg::git("Fetch", &["fetch", "--all"]), Instant::now());
    assert!(matches!(&fx[..], [Effect::RunGit { .. }]), "{fx:?}");
    update(st, Msg::Job(JobMsg::JobDone { ok, output: output.into(), undo: None }), Instant::now())
}

/// Runs the auto-fetch the next interval tick starts, to completion; returns the skipped effects.
fn auto_fetch(st: &mut AppState) -> Vec<Effect> {
    st.settings.auto_fetch_minutes = 1;
    let tick = st.last_fetch + Duration::from_secs(61);
    let fx = update(st, Msg::Tick(tick), Instant::now());
    finish_keeping(st, fx)
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn turning_on_saves_and_sends_one_confirmation() {
    let mut st = state();
    assert!(st.view.focused);
    let fx = set(&mut st, true);
    assert!(st.settings.desktop_notifications);
    assert_eq!(fx.len(), 2, "{fx:?}");
    assert_eq!(saves(&fx), 1);
    let n = notes(&fx);
    assert_eq!(n.len(), 1, "the confirmation goes out even though the window is focused");
    assert_eq!(n[0].kind, NoteKind::Confirm);
    assert_eq!(*n[0], DesktopNote::confirmation());
    assert_eq!(n[0].workdir, None, "about no repository");
}

#[test]
fn turning_off_or_repeating_sends_no_notification() {
    let mut st = state();
    st.settings.desktop_notifications = true;
    assert!(set(&mut st, true).is_empty(), "already on");
    let fx = set(&mut st, false);
    assert!(!st.settings.desktop_notifications);
    assert_eq!(saves(&fx), 1);
    assert!(notes(&fx).is_empty());
    assert!(set(&mut st, false).is_empty(), "already off");
}

#[test]
fn finished_job_notifies_while_unfocused() {
    let dir = scratch_repo("desktop-job");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    st.settings.desktop_notifications = true;
    st.view.focused = false;
    let name = tab_name(&st);
    assert!(!name.is_empty());

    let fx = fetch_ends(&mut st, true, "$ git fetch --all");
    let n = notes(&fx);
    assert_eq!(n.len(), 1, "{fx:?}");
    assert_eq!((n[0].kind, n[0].title.as_str(), n[0].body.as_str()), (NoteKind::Job, "Fetch succeeded", name.as_str()));
    assert!(n[0].workdir.as_deref().is_some_and(|w| same_place(w, &dir)), "a click shows its repository: {:?}", n[0].workdir);
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Fetch succeeded"), "the toast is still shown");

    let fx = fetch_ends(&mut st, false, "$ git fetch\nfatal: offline");
    let n = notes(&fx);
    assert_eq!(n.len(), 1, "{fx:?}");
    assert_eq!((n[0].kind, n[0].title.as_str(), n[0].body.as_str()), (NoteKind::Job, "Fetch failed: fatal: offline", name.as_str()));
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Fetch failed: fatal: offline"));
    cleanup(&[&dir]);
}

#[test]
fn no_job_notification_when_focused_or_off() {
    let dir = scratch_repo("desktop-quiet");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    st.settings.desktop_notifications = true;
    st.view.focused = true;
    assert!(notes(&fetch_ends(&mut st, true, "")).is_empty(), "focused");
    st.settings.desktop_notifications = false;
    st.view.focused = false;
    assert!(notes(&fetch_ends(&mut st, true, "")).is_empty(), "setting off");
    assert_eq!(st.toasts.len(), 2, "both still toast");
    cleanup(&[&dir]);
}

#[test]
fn quiet_auto_fetch_does_not_notify() {
    let (mut st, work, bare) = with_origin("desktop-autofetch");
    st.settings.desktop_notifications = true;
    st.view.focused = false;
    let kept = auto_fetch(&mut st);
    assert!(notes(&kept).is_empty(), "{kept:?}");
    assert!(st.toasts.is_empty());
    cleanup(&[&work, &bare]);
}

#[test]
fn behind_alert_also_goes_to_the_desktop() {
    let (mut st, work, bare) = with_origin("desktop-behind");
    st.settings.desktop_notifications = true;
    st.view.focused = false;
    let name = tab_name(&st);
    remote_commit(&bare, "main", "r1.txt");
    let kept = auto_fetch(&mut st);
    let n = notes(&kept);
    assert_eq!(n.len(), 1, "{kept:?}");
    assert_eq!(
        (n[0].kind, n[0].title.as_str(), n[0].body.as_str()),
        (NoteKind::BehindAlert, "main is 1 commit behind origin/main", name.as_str())
    );
    assert!(n[0].workdir.as_deref().is_some_and(|w| same_place(w, &work)), "{:?}", n[0].workdir);
    let t = st.toasts.last().expect("the in-app alert");
    assert_eq!(t.text, "main is 1 commit behind origin/main");
    assert!(t.action.is_some(), "still with its Pull button");

    // Focused: only the toast.
    st.view.focused = true;
    remote_commit(&bare, "main", "r2.txt");
    let kept = auto_fetch(&mut st);
    assert!(notes(&kept).is_empty(), "{kept:?}");
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("main is 2 commits behind origin/main"));

    // Behind alerts off: neither.
    st.view.focused = false;
    st.settings.behind_alerts = false;
    let toasts = st.toasts.len();
    remote_commit(&bare, "main", "r3.txt");
    let kept = auto_fetch(&mut st);
    assert!(notes(&kept).is_empty(), "{kept:?}");
    assert_eq!(st.toasts.len(), toasts);
    cleanup(&[&work, &bare]);
}

#[test]
fn notification_failures() {
    let mut st = state();
    st.settings.desktop_notifications = true;
    let failed = |confirm| Msg::Prefs(PrefsMsg::DesktopNotificationFailed { text: "no server".into(), confirm });
    assert!(update(&mut st, failed(true), Instant::now()).is_empty());
    let t = st.toasts.last().expect("the confirmation's failure is toasted");
    assert!(!t.ok);
    assert_eq!(t.text, "Couldn't show a desktop notification: no server");
    assert_eq!(st.log.len(), 1);
    assert!(!st.log[0].ok && st.log[0].text.contains("no server"));
    assert!(!st.view.show_log, "logged quietly");

    assert!(update(&mut st, failed(false), Instant::now()).is_empty());
    assert_eq!(st.toasts.len(), 1, "a later failure isn't toasted");
    assert_eq!(st.log.len(), 2);
    assert!(!st.view.show_log);
    assert!(st.settings.desktop_notifications, "the setting stays on");
}
