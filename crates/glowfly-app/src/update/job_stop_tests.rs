//! Headless tests for cancelling the active job from the status bar: only a job that just
//! fetches can be, a stopped one ends as cancelled, and one that finished first ends as usual.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::effect::partial::StopKey;
use crate::msg::{JobMsg, Msg, RepoMsg};
use crate::state::AppState;
use std::path::PathBuf;
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// A repository with an origin (another repository on this computer), open in a fresh state.
fn with_origin(name: &str) -> (AppState, PathBuf, PathBuf) {
    let origin = scratch_repo(&format!("{name}-origin"));
    let dir = scratch_repo(name);
    git(&dir, &["remote", "add", "origin", &origin.display().to_string()]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir, origin)
}

fn stoppable(fx: &[Effect]) -> Option<bool> {
    fx.iter().find_map(|e| match e {
        Effect::RunGit { stoppable, .. } => Some(*stoppable),
        _ => None,
    })
}

fn cleanup(dirs: &[&PathBuf]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn a_fetch_is_cancelled_and_ends_as_cancelled() {
    let (mut st, dir, origin) = with_origin("job-stop-fetch");
    assert!(!st.job_cancellable());
    let fx = run(&mut st, Msg::git("Fetch", &["fetch", "origin"]));
    assert_eq!(stoppable(&fx), Some(true));
    assert!(st.job_cancellable() && !st.job_cancelling());

    let fx = run(&mut st, Msg::Job(JobMsg::Cancel));
    assert!(matches!(&fx[..], [Effect::StopGit(StopKey::Job)]));
    assert!(st.job_cancelling() && !st.job_cancellable());
    assert!(st.log.last().is_some_and(|l| l.text == "Cancelling Fetch…"));
    assert!(run(&mut st, Msg::Job(JobMsg::Cancel)).is_empty(), "asked once");

    let output = "$ git fetch origin\nCancelled.".to_owned();
    drive(&mut st, Msg::Job(JobMsg::Stopped { output }));
    assert!(st.job.is_none() && st.job_stop.is_none());
    let toast = st.toasts.last().expect("a toast");
    assert!(toast.ok, "a cancel isn't a failure");
    assert_eq!(toast.text, "Fetch cancelled");
    cleanup(&[&dir, &origin]);
}

#[test]
fn a_job_that_does_more_than_fetch_has_no_cancel() {
    let (mut st, dir, origin) = with_origin("job-stop-tag");
    let fx = run(&mut st, Msg::git("Tag", &["tag", "v1"]));
    assert_eq!(stoppable(&fx), Some(false));
    assert!(!st.job_cancellable());
    assert!(run(&mut st, Msg::Job(JobMsg::Cancel)).is_empty());
    cleanup(&[&dir, &origin]);
}

#[test]
fn a_fetch_that_finished_before_the_stop_reached_it_ends_as_usual() {
    let (mut st, dir, origin) = with_origin("job-stop-late");
    run(&mut st, Msg::git("Fetch", &["fetch", "origin"]));
    run(&mut st, Msg::Job(JobMsg::Cancel));
    drive(&mut st, Msg::Job(JobMsg::JobDone { ok: true, output: String::new(), undo: None }));
    assert!(st.job.is_none() && st.job_stop.is_none());
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Fetch succeeded"));
    // A stop reported for a job that is gone is dropped.
    assert!(run(&mut st, Msg::Job(JobMsg::Stopped { output: String::new() })).is_empty());
    cleanup(&[&dir, &origin]);
}
