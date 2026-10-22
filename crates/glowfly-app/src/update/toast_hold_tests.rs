//! Headless tests for holding and closing toasts (update/notify.rs): the pointer's hold moves
//! from toast to toast, and the x removes a toast without running its action.

use super::auto_stash_tests::up;
use super::job_errors_tests::two_branches;
use super::tests::{finish, state};
use super::update;
use crate::logs::LogsMsg;
use crate::msg::{Msg, NotifyMsg};
use crate::state::{AppState, ToastAction};
use std::time::Instant;

fn notify(st: &mut AppState, m: NotifyMsg) -> Vec<crate::effect::Effect> {
    update(st, Msg::Notify(m), Instant::now())
}

#[test]
fn hold_and_release_through_messages() {
    let mut st = state();
    let now = Instant::now();
    st.toast(true, "one", now);
    st.toast(true, "two", now);
    let (a, b) = (st.toasts[0].id, st.toasts[1].id);
    assert!(notify(&mut st, NotifyMsg::HoldToast(Some(a))).is_empty());
    assert_eq!(st.held_toast(), Some(a));
    assert!(notify(&mut st, NotifyMsg::HoldToast(Some(b))).is_empty());
    assert_eq!(st.held_toast(), Some(b), "the hold moves");
    assert!(!st.toasts[0].held);
    assert!(notify(&mut st, NotifyMsg::HoldToast(None)).is_empty());
    assert_eq!(st.held_toast(), None, "let go");
    notify(&mut st, NotifyMsg::HoldToast(Some(b + 100)));
    assert_eq!(st.held_toast(), None, "no toast has a stale id");
}

#[test]
fn the_x_removes_the_toast_without_running_its_action() {
    let mut st = state();
    let dir = glowfly_core::paths::temp_dir().join("glowfly-toast-x");
    let now = Instant::now();
    st.toast(true, "Fetch succeeded", now);
    let pull = ToastAction { label: "Pull".into(), msg: Box::new(Msg::git("Pull", &["pull", "--ff"])), workdir: Some(dir) };
    let id = st.toast_action(true, "main is 1 commit behind origin/main", pull, now);
    assert!(notify(&mut st, NotifyMsg::CloseToast(id + 100)).is_empty());
    assert_eq!(st.toasts.len(), 2, "an unknown id changes nothing");
    let fx = notify(&mut st, NotifyMsg::CloseToast(id));
    assert!(fx.is_empty(), "no Pull: {fx:?}");
    assert!(st.toasts.iter().all(|t| t.id != id));
    assert_eq!(st.toasts.len(), 1);
}

#[test]
fn closing_a_failure_keeps_its_fix_in_the_log() {
    let (mut st, dir) = two_branches("toast-x-fix");
    std::fs::write(dir.join("f.txt"), "1\n2\n3\n4\n5\n6\n7\nmine\n").unwrap();
    let fx = up(&mut st, Msg::git("Checkout", &["checkout", "feature", "--"]));
    finish(&mut st, fx);
    let t = st.toasts.last().expect("the failure");
    assert!(!t.ok && t.action.is_some(), "a failure with a fix");
    let Some(Msg::Logs(LogsMsg::Fix(run))) = t.action.as_ref().map(|a| &*a.msg) else { panic!("the fix is kept in the log") };
    let (id, run) = (t.id, *run);
    assert!(st.logs.fix(run).is_some(), "kept in the log before the close");
    let entries = st.log.len();
    assert!(notify(&mut st, NotifyMsg::CloseToast(id)).is_empty());
    assert!(st.toasts.iter().all(|t| t.id != id));
    assert_eq!(st.log.len(), entries);
    let entry = st.log.iter().rev().find(|e| e.text.starts_with("Checkout FAILED")).expect("the job's log entry stays");
    assert!(entry.text.contains("would be overwritten"), "{}", entry.text);
    assert_eq!(st.logs.fix_label(run), Some("Stash and check out"), "the fix stays in the activity log");
    let _ = std::fs::remove_dir_all(&dir);
}
