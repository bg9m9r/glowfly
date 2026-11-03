//! Screen-reader announcements (Preferences > Accessibility > Announcements): each toast's text
//! and the status bar's job are AccessKit live regions, polite but assertive for a failure, under
//! keys that stay put while other toasts close or a success is shown again (a failure shown again is read again); a quiet job (auto-fetch) never is; with Announcements off they are plain labels.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_partial::job;
use crate::msg::Msg;
use crate::state::AppState;
use crate::ui::{status, toasts};
use eframe::egui::Id;
use eframe::egui::accesskit::{Live, NodeId};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use std::time::Instant;

type H = Harness<'static, (AppState, Vec<Msg>)>;

const DONE: &str = "Fetch succeeded";
const FAILED: &str = "Push failed: the remote rejected the update.";
const JOB: &str = "Fetch…";

/// The repository with a success toast, a failure toast and a Fetch running, Announcements `on`.
fn busy(name: &str, on: bool) -> (H, std::path::PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    st.settings.a11y.announce_jobs = on;
    let now = Instant::now();
    st.toast(true, DONE, now);
    st.toast(false, FAILED, now);
    st.job = Some(job("Fetch"));
    let mut h = harness(st, true);
    h.run_steps(4);
    (h, dir)
}

/// The AccessKit node of key `id`.
fn node(id: Id) -> NodeId {
    NodeId(id.value())
}

#[test]
fn toasts_and_the_job_are_live_regions() {
    let (mut h, dir) = busy("announce-on", true);
    assert_eq!(h.get_by_label(DONE).live(), Live::Polite, "a success waits its turn");
    assert_eq!(h.get_by_label(FAILED).live(), Live::Assertive, "a failure interrupts");
    assert_eq!(h.get_by_label(JOB).live(), Live::Polite);
    let failure = toasts::text_key(&h.state().0.toasts[1]);
    assert_eq!(h.get_by_label(FAILED).id(), node(failure));
    assert_eq!(h.get_by_label(JOB).id(), node(status::job_key()));
    // The toast above closes: the failure keeps its node, so it isn't announced again.
    h.state_mut().0.toasts.remove(0);
    h.run_steps(2);
    assert!(h.query_by_label(DONE).is_none());
    assert_eq!(h.get_by_label(FAILED).id(), node(failure), "the same node");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn a_failure_shown_again_is_announced_again_and_a_success_is_not() {
    let (mut h, dir) = busy("announce-again", true);
    let (done, failure) = (toasts::text_key(&h.state().0.toasts[0]), toasts::text_key(&h.state().0.toasts[1]));
    // A retry that fails the same way, and a success said again: one toast each still.
    h.state_mut().0.toast(false, FAILED, Instant::now());
    h.state_mut().0.toast(true, DONE, Instant::now());
    h.run_steps(2);
    assert_eq!(h.state().0.toasts.len(), 2, "shown again, not copied");
    let again = h.get_by_label(FAILED);
    assert_ne!(again.id(), node(failure), "a new node: the failure is read out again");
    assert_eq!(again.live(), Live::Assertive);
    assert_eq!(h.get_by_label(DONE).id(), node(done), "the success keeps its node, so it stays quiet");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn a_quiet_job_is_not_announced() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "announce-quiet");
    st.job = Some(crate::state::RunningJob { quiet: true, ..job("Fetch") });
    assert!(st.settings.a11y.announce_jobs);
    let mut h = harness(st, true);
    h.run_steps(4);
    assert_eq!(h.get_by_label(JOB).live(), Live::Off, "auto-fetch runs on a timer: not read every time");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn nothing_is_live_with_announcements_off() {
    let (h, dir) = busy("announce-off", false);
    for text in [DONE, FAILED, JOB] {
        assert_eq!(h.get_by_label(text).live(), Live::Off, "{text} is a plain label");
    }
    drop(h);
    remove_fixture(&dir);
}
