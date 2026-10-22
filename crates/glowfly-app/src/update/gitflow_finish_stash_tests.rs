//! Headless tests for Git Flow's Stash changes and finish: the job's stash command in front of
//! the plan, the changes put back on the development branch, a plain Finish still refused, a
//! clean tree finishing without a stash, a merge conflict holding the stash until Continue
//! finish or Forget, and the headline when the stash itself fails, end to end against real
//! repositories.

use super::gitflow_finish::headline;
use super::gitflow_finish_tests::{commit, exists, ready};
use super::gitflow_tests::{cleanup, flow, git_job, job_label, last_log};
use super::tests::{drive, finish, git};
use crate::effect::{Effect, StashEffect};
use crate::gitflow::GitFlowMsg;
use crate::msg::{Msg, RepoMsg};
use crate::state::gitflow::FinishJob;
use crate::state::{AppState, RunningJob};
use git2::RepositoryState;
use glowfly_core::gitcli::{args, run as git_run};
use glowfly_core::gitflow::{Branches, FinishOptions, StartTips, WorkState, check_finish};
use std::path::{Path, PathBuf};

const STASH: &str = "Auto-stash before finishing feature/login";

fn stash_and_finish() -> GitFlowMsg {
    GitFlowMsg::StashAndFinish { branch: "feature/login".into(), options: FinishOptions::default() }
}

fn refresh(st: &mut AppState) {
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

fn stash_list(dir: &Path) -> String {
    git_run(dir, &args(&["stash", "list"])).1
}

fn read(dir: &Path, file: &str) -> String {
    std::fs::read_to_string(dir.join(file)).unwrap().replace("\r\n", "\n")
}

fn toasted(st: &AppState, start: &str) -> bool {
    st.toasts.iter().any(|t| t.text.starts_with(start))
}

/// The `ready` fixture with a.txt edited (a tracked change) and the snapshot refreshed.
fn dirty(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = ready(name);
    std::fs::write(dir.join("a.txt"), "edited\n").unwrap();
    refresh(&mut st);
    (st, dir)
}

#[test]
fn stash_and_finish_end_to_end() {
    let (mut st, dir) = dirty("stash-e2e");
    let fx = flow(&mut st, stash_and_finish());
    assert_eq!(
        git_job(&fx),
        Some(vec![
            args(&["stash", "push", "-m", STASH]),
            args(&["checkout", "develop", "--"]),
            args(&["merge", "--no-ff", "--no-edit", "refs/heads/feature/login"]),
            args(&["branch", "-d", "feature/login"]),
        ])
    );
    assert_eq!(job_label(&st).as_deref(), Some("Finish feature login"));
    assert!(st.gitflow_finish.as_ref().is_some_and(|f| f.stashed));
    finish(&mut st, fx);
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("develop"));
    assert!(!exists(&dir, "refs/heads/feature/login"));
    assert_eq!(read(&dir, "a.txt"), "edited\n", "the changes are back");
    assert_eq!(stash_list(&dir).trim(), "", "the stash was popped");
    assert!(toasted(&st, "Finished feature login: merged into develop and deleted feature/login."));
    assert!(toasted(&st, &format!("Your changes from \"{STASH}\" are back.")));
    cleanup(&dir);
}

#[test]
fn a_plain_finish_still_refuses_changes() {
    let (mut st, dir) = dirty("stash-plain");
    let fx = flow(&mut st, GitFlowMsg::Finish { branch: "feature/login".into(), options: FinishOptions::default() });
    assert!(git_job(&fx).is_none());
    assert_eq!(last_log(&st), "Commit or stash your changes before finishing feature/login.");
    assert!(st.job.is_none() && st.gitflow_finish.is_none());
    cleanup(&dir);
}

#[test]
fn a_clean_tree_finishes_without_a_stash() {
    let (mut st, dir) = ready("stash-clean");
    let cmds = git_job(&flow(&mut st, stash_and_finish())).expect("a job");
    assert_eq!(cmds[0], args(&["checkout", "develop", "--"]));
    assert!(cmds.iter().all(|c| c[0] != "stash"), "{cmds:?}");
    assert!(st.gitflow_finish.as_ref().is_some_and(|f| !f.stashed));
    assert!(st.auto_stash.making.is_none());
    cleanup(&dir);
}

/// develop changes login.txt too, so Finish's merge conflicts; a.txt is edited (tracked) and
/// Stash changes and finish stopped at the conflict.
fn conflicted(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = ready(name);
    git(&dir, &["checkout", "-q", "develop"]);
    commit(&dir, "login.txt", "develop\n");
    git(&dir, &["checkout", "-q", "feature/login"]);
    std::fs::write(dir.join("a.txt"), "edited\n").unwrap();
    refresh(&mut st);
    drive(&mut st, Msg::GitFlow(stash_and_finish()));
    assert_eq!(st.snap.as_ref().unwrap().state, RepositoryState::Merge);
    (st, dir)
}

/// Resolves login.txt, commits the merge and refreshes.
fn commit_merge(st: &mut AppState, dir: &Path) {
    std::fs::write(dir.join("login.txt"), "resolved\n").unwrap();
    git(dir, &["add", "login.txt"]);
    git(dir, &["commit", "-q", "--no-edit"]);
    refresh(st);
}

#[test]
fn a_conflict_holds_the_stash_until_continue_finish() {
    let (mut st, dir) = conflicted("stash-conflict");
    assert!(
        toasted(&st, "Finish feature login stopped: \"merge feature/login into develop\" has conflicts."),
        "{:?}",
        st.toasts.iter().map(|t| &t.text).collect::<Vec<_>>()
    );
    assert!(toasted(&st, &format!("Stashed your changes as \"{STASH}\".")));
    assert!(st.gitflow_resume.is_some());
    assert!(stash_list(&dir).contains(STASH));
    commit_merge(&mut st, &dir);
    assert!(st.gitflow_resume.as_ref().is_some_and(|r| r.ready));
    assert!(stash_list(&dir).contains(STASH), "held while Continue finish waits for a clean tree");
    drive(&mut st, Msg::GitFlow(GitFlowMsg::ContinueFinish));
    assert!(st.gitflow_resume.is_none());
    assert!(!exists(&dir, "refs/heads/feature/login"), "the rest ran");
    assert_eq!(read(&dir, "a.txt"), "edited\n", "the changes are back");
    assert_eq!(stash_list(&dir).trim(), "");
    cleanup(&dir);
}

#[test]
fn forget_puts_a_held_stash_back() {
    let (mut st, dir) = conflicted("stash-forget");
    commit_merge(&mut st, &dir);
    assert!(stash_list(&dir).contains(STASH));
    let fx = flow(&mut st, GitFlowMsg::ForgetFinish);
    assert!(fx.iter().any(|e| matches!(e, Effect::Stash(StashEffect::PutBack(_)))), "put back at once");
    finish(&mut st, fx);
    assert_eq!(read(&dir, "a.txt"), "edited\n");
    assert_eq!(stash_list(&dir).trim(), "");
    cleanup(&dir);
}

/// A running Stash changes and finish job for feature/login, as `finish` records it.
fn pending(st: &mut AppState, dir: &Path) {
    let snap = st.snap.as_ref().unwrap();
    let s = snap.gitflow.settings().unwrap();
    let clean = WorkState { state: RepositoryState::Clean, tracked_changes: false, elsewhere: vec![] };
    let plan = check_finish(&s, "feature/login", &FinishOptions::default(), &Branches::of(snap), &clean).unwrap();
    let label = plan.label();
    let (done, tips) = (plan.summary(&s), StartTips::of(snap, &plan));
    st.job = Some(RunningJob {
        label: label.clone(),
        workdir: Some(dir.to_path_buf()),
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        then_pr: None,
        then_link: false,
        keeps_undo: false,
        fetches: false,
    });
    st.gitflow_finish = Some(FinishJob { label, plan, done, tips, resuming: false, stashed: true });
}

#[test]
fn headlines_skip_the_stash_command() {
    let (mut st, dir) = ready("stash-headline");
    pending(&mut st, &dir);
    let failed = "$ git stash push -m x\nerror: could not write index";
    assert_eq!(
        headline(&mut st, false, failed).as_deref(),
        Some("Finish feature login didn't start: your changes couldn't be stashed: error: could not write index")
    );
    pending(&mut st, &dir);
    let second = "$ git stash push -m x\nSaved working directory\n$ git checkout develop --\n\
                  $ git merge --no-ff --no-edit refs/heads/feature/login\nerror: something broke";
    assert_eq!(
        headline(&mut st, false, second).as_deref(),
        Some(
            "Finish feature login stopped at \"merge feature/login into develop\": error: something broke. Still to do: delete feature/login."
        )
    );
    cleanup(&dir);
}
