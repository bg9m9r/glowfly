//! The success voice of pushes and pulls (update/sync_voice.rs): one short sentence that says
//! what was sent or brought in, as a brief success toast, and none while Confirm pushes and pulls
//! is off (the activity log keeps the sentence).

use super::failure_headline_tests::{opened, running};
use super::update;
use crate::msg::{JobMsg, Msg};
use crate::state::AppState;
use glowfly_core::settings::ToastTime;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const UPDATED: &str = "To https://example.com/r.git\n   1234567..89abcde  main -> main";
const NEW_BRANCH: &str = "To https://example.com/r.git\n * [new branch]      main -> main\nbranch 'main' set up to track 'origin/main'.";

/// A repository on `main` with an origin remote, its branch `ahead` and `behind` its upstream
/// (none when `tracked` is false).
fn repo(name: &str, tracked: bool, ahead: usize, behind: usize) -> (AppState, PathBuf) {
    let (mut st, dir) = opened(name);
    let snap = st.snap.as_mut().expect("opened");
    snap.remotes = vec![("origin".into(), Vec::new())];
    let head = snap.locals.iter_mut().find(|b| b.is_head).expect("a checked-out branch");
    head.name = "main".into();
    head.upstream = tracked.then(|| "origin/main".to_owned());
    (head.ahead, head.behind) = (ahead, behind);
    snap.head_branch = Some("main".into());
    (st, dir)
}

fn done(st: &mut AppState, dir: &std::path::Path, label: &str, output: &str) {
    running(st, dir, label);
    update(st, Msg::Job(JobMsg::JobDone { ok: true, output: format!("$ git x\n{output}"), undo: None }), Instant::now());
}

fn said(st: &AppState) -> &str {
    st.toasts.last().map(|t| t.text.as_str()).expect("a toast")
}

#[test]
fn a_push_says_how_many_commits_went_where() {
    let (mut st, dir) = repo("voice-push", true, 2, 0);
    done(&mut st, &dir, "Push", UPDATED);
    assert_eq!(said(&st), "Pushed 2 commits to origin/main.");
    let toast = st.toasts.last().unwrap();
    assert!(toast.ok && toast.brief && toast.action.is_none());
    assert_eq!(toast.lifetime(ToastTime::Standard), Some(Duration::from_secs(3)), "a brief success");
    assert_eq!(toast.lifetime(ToastTime::Longer), ToastTime::Longer.lifetime(false), "Longer still stays longer");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn one_commit_is_singular() {
    let (mut st, dir) = repo("voice-push-one", true, 1, 0);
    done(&mut st, &dir, "Push", UPDATED);
    assert_eq!(said(&st), "Pushed 1 commit to origin/main.");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_new_branch_is_published() {
    let (mut st, dir) = repo("voice-publish", false, 0, 0);
    done(&mut st, &dir, "Push", NEW_BRANCH);
    assert_eq!(said(&st), "Published main to origin.");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_push_with_nothing_to_send_is_up_to_date() {
    let (mut st, dir) = repo("voice-push-none", true, 0, 0);
    done(&mut st, &dir, "Push", "Everything up-to-date");
    assert_eq!(said(&st), "main is already up to date.");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_pull_says_how_many_commits_came_in() {
    let (mut st, dir) = repo("voice-pull", true, 0, 3);
    done(&mut st, &dir, "Pull", "Updating 1234567..89abcde\nFast-forward");
    assert_eq!(said(&st), "Pulled 3 commits into main.");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_pull_with_nothing_new_is_up_to_date() {
    let (mut st, dir) = repo("voice-pull-none", true, 0, 0);
    done(&mut st, &dir, "Pull (rebase)", "Already up to date.");
    assert_eq!(said(&st), "main is already up to date.");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_pull_whose_fetch_moved_the_upstream_doesnt_guess_a_count() {
    let (mut st, dir) = repo("voice-pull-moved", true, 0, 1);
    let output = "From https://example.com/r\n   1234567..89abcde  main       -> origin/main\nUpdating 1234567..89abcde";
    done(&mut st, &dir, "Pull", output);
    assert_eq!(said(&st), "Pulled new commits into main.");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn turned_off_there_is_no_toast_but_the_log_keeps_it() {
    let (mut st, dir) = repo("voice-off", true, 2, 0);
    st.settings.confirm_push_pull = false;
    let before = st.toasts.len();
    done(&mut st, &dir, "Push", UPDATED);
    assert_eq!(st.toasts.len(), before, "no toast");
    let run = st.log.iter().rev().find_map(|l| l.run.as_ref()).expect("the push is logged as a run");
    assert_eq!(run.summary, "Pushed 2 commits to origin/main.");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn other_jobs_keep_their_own_headline() {
    let (mut st, dir) = repo("voice-other", true, 2, 0);
    done(&mut st, &dir, "Push tag v1 to origin", UPDATED);
    assert!(!said(&st).starts_with("Pushed"), "{}", said(&st));
    assert!(!st.toasts.last().unwrap().brief);
    let _ = std::fs::remove_dir_all(&dir);
}
