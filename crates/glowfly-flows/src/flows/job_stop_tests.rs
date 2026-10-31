//! Headless tests for cancelling the active job from the status bar: a fetch, a push, a pull
//! while it fetches and a clone can be, a merge or a local step can't, a stopped job ends as
//! cancelled with no Try again, and one that finished first ends as usual.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::effect::partial::StopKey;
use crate::msg::{CloneMsg, JobMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::clone::CloneRequest;
use crate::state::job_stop::Step;
use glowfly_core::clone_ops::CloneOptions;
use glowfly_core::gitcli::args;
use std::path::PathBuf;
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// A repository with no local changes and an origin (another repository on this computer), open
/// in a fresh state.
fn with_origin(name: &str) -> (AppState, PathBuf, PathBuf) {
    let origin = scratch_repo(&format!("{name}-origin"));
    let dir = scratch_repo(name);
    git(&dir, &["commit", "-qam", "second"]);
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
    drive(&mut st, Msg::Job(JobMsg::Stopped { output, undo: None }));
    assert!(st.job.is_none() && st.job_stop.is_none());
    let toast = st.toasts.last().expect("a toast");
    assert!(toast.ok, "a cancel isn't a failure");
    assert_eq!(toast.text, "Fetch cancelled");
    cleanup(&[&dir, &origin]);
}

#[test]
fn a_job_without_a_network_command_has_no_cancel() {
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
    assert!(run(&mut st, Msg::Job(JobMsg::Stopped { output: String::new(), undo: None })).is_empty());
    cleanup(&[&dir, &origin]);
}

/// Cancels the running job and has the runner report it stopped with `output`.
fn cancel_and_stop(st: &mut AppState, output: &str) {
    let fx = run(st, Msg::Job(JobMsg::Cancel));
    assert!(matches!(&fx[..], [Effect::StopGit(StopKey::Job)]), "{fx:?}");
    drive(st, Msg::Job(JobMsg::Stopped { output: output.into(), undo: None }));
}

#[test]
fn a_push_is_cancelled_and_never_offered_again() {
    let (mut st, dir, origin) = with_origin("job-stop-push");
    let fx = run(&mut st, Msg::git("Push", &["push", "origin", "main"]));
    assert_eq!(stoppable(&fx), Some(true));
    assert!(st.job_cancellable());
    assert_eq!(st.job_stop.as_ref().map(|s| s.step), Some(Step::Push));
    cancel_and_stop(&mut st, "$ git push origin main\nCancelled.");
    let toast = st.toasts.last().expect("a toast");
    assert_eq!(toast.text, "Push cancelled");
    assert!(toast.action.is_none(), "no Try again for a cancelled push");
    assert!(st.logs.job_retry.is_none(), "nothing keeps the push for running again");
    cleanup(&[&dir, &origin]);
}

#[test]
fn a_pull_can_be_cancelled_only_while_it_fetches() {
    let (mut st, dir, origin) = with_origin("job-stop-pull");
    let fx = run(&mut st, Msg::git("Pull", &["pull", "--ff"]));
    assert_eq!(stoppable(&fx), Some(true));
    assert!(st.job_cancellable(), "a pull starts with its fetch");
    // The runner starts the merge: Cancel is shown disabled, with the reason.
    run(&mut st, Msg::Job(JobMsg::Step(Step::Merge)));
    assert!(!st.job_cancellable());
    assert_eq!(st.job_cancel_blocked(), Some(Step::Merge));
    assert!(run(&mut st, Msg::Job(JobMsg::Cancel)).is_empty(), "a merge is never stopped");
    drive(&mut st, Msg::Job(JobMsg::JobDone { ok: true, output: String::new(), undo: None }));
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Pull succeeded"));
    assert_eq!(st.job_cancel_blocked(), None);
    cleanup(&[&dir, &origin]);
}

#[test]
fn a_pull_stopped_in_its_fetch_ends_as_cancelled() {
    let (mut st, dir, origin) = with_origin("job-stop-pull-fetch");
    run(&mut st, Msg::git("Pull", &["pull", "--ff"]));
    run(&mut st, Msg::Job(JobMsg::Step(Step::Fetch)));
    cancel_and_stop(&mut st, "$ git fetch --update-head-ok\nCancelled.");
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Pull cancelled"));
    cleanup(&[&dir, &origin]);
}

#[test]
fn a_clone_is_cancelled_and_not_opened() {
    let mut st = state();
    let parent = scratch_repo("job-stop-clone-parent");
    let req = CloneRequest {
        url: "https://example.com/team/app.git".into(),
        parent: parent.clone(),
        name: "app".into(),
        options: CloneOptions::default(),
    };
    let fx = run(&mut st, Msg::Clone(CloneMsg::Start(req)));
    assert_eq!(stoppable(&fx), Some(true));
    assert_eq!(st.job_stop.as_ref().map(|s| s.step), Some(Step::Clone));
    let fx = run(&mut st, Msg::Job(JobMsg::Cancel));
    assert!(matches!(&fx[..], [Effect::StopGit(StopKey::Job)]));
    let output = format!("$ git clone -- x app\nCancelled.\nRemoved {}, which the stopped clone had made.", parent.join("app").display());
    let fx = run(&mut st, Msg::Job(JobMsg::Stopped { output, undo: None }));
    assert!(!fx.iter().any(|e| matches!(e, Effect::LoadRepo { .. })), "a cancelled clone isn't opened: {fx:?}");
    assert!(st.job.is_none());
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Clone cancelled"));
    let logged = st.log.iter().rev().find(|l| l.text.starts_with("Clone cancelled")).map(|l| l.text.clone()).unwrap_or_default();
    assert!(logged.contains("which the stopped clone had made"), "the log says what was removed: {logged}");
    cleanup(&[&parent]);
}

#[test]
fn a_job_with_a_local_step_before_its_push_waits_for_that_step() {
    let (mut st, dir, origin) = with_origin("job-stop-two");
    git(&dir, &["branch", "old"]);
    let cmds = vec![args(&["branch", "-D", "old"]), args(&["push", "origin", "--delete", "old"])];
    let fx = run(&mut st, Msg::Job(JobMsg::Git { label: "Delete local and remote branch".into(), cmds }));
    assert_eq!(stoppable(&fx), Some(true));
    assert_eq!(st.job_cancel_blocked(), Some(Step::Local), "the local delete runs first");
    run(&mut st, Msg::Job(JobMsg::Step(Step::Push)));
    assert!(st.job_cancellable());
    // Progress belongs to the step it was reported in.
    let p = glowfly_core::git_progress::parse("Writing objects:  50% (1/2)").unwrap();
    run(&mut st, Msg::Job(JobMsg::Progress(p.clone())));
    assert_eq!(st.job_progress(), Some(&p));
    run(&mut st, Msg::Job(JobMsg::Step(Step::Local)));
    assert_eq!(st.job_progress(), None);
    cleanup(&[&dir, &origin]);
}
