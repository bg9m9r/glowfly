//! Headless tests for Push to your fork…: the Fork dialog it opens for the base repository, and
//! the jobs that add the user's fork and push the branch to it. The service is answered by hand.

use super::push_pr_fork_tests::with_fork;
use super::push_pr_tests::{cleanup, with_host};
use super::update;
use crate::effect::Effect;
use crate::msg::{ForgeMsg, Msg};
use crate::remote::{RemoteEffect, RemoteMsg};
use crate::state::AppState;
use crate::state::Modal;
use crate::state::forks::{ForkCreate, ForkThenPr};
use crate::state::push_pr::PrStart;
use glowfly_core::forge::{CreatedFork, Fork, ForkLookup, Target, parse_remote};
use glowfly_core::gitcli::args;
use std::time::Instant;

fn origin() -> Target {
    parse_remote("https://github.com/o/r.git", &[]).unwrap()
}

fn fork(owner: &str) -> Fork {
    Fork {
        owner: owner.into(),
        full_name: format!("{owner}/r"),
        https_url: format!("https://github.com/{owner}/r.git"),
        ssh_url: format!("git@github.com:{owner}/r.git"),
        web_url: format!("https://github.com/{owner}/r"),
        private: false,
    }
}

fn run(st: &mut AppState, m: RemoteMsg) -> Vec<Effect> {
    update(st, Msg::Remote(m), Instant::now())
}

fn creator(st: &AppState) -> &ForkCreate {
    match st.modal.as_ref() {
        Some(Modal::CreateFork(f)) => f,
        _ => panic!("the Fork dialog is not open"),
    }
}

/// Push to your fork… for `topic`, then the lookup's answer.
fn open(st: &mut AppState, mine: Option<Fork>) -> Vec<Effect> {
    let fx = update(st, Msg::Forge(ForgeMsg::PushToMyFork { branch: "topic".into(), target: Some("main".into()) }), Instant::now());
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    let result = Box::new(Ok(ForkLookup { user: "ada".into(), mine, own_repository: false }));
    run(st, RemoteMsg::MyForkFound { workdir, target: origin(), result });
    fx
}

fn git_job(fx: &[Effect]) -> Vec<Vec<String>> {
    match fx {
        [Effect::RunGit { cmds, .. }] => cmds.clone(),
        other => panic!("unexpected effects {other:?}"),
    }
}

fn then_pr(st: &AppState) -> Option<PrStart> {
    st.job.as_ref().and_then(|j| j.then_pr.clone())
}

fn pushed_to_ada() -> PrStart {
    PrStart { source: "topic".into(), target: Some("main".into()), remote: Some("ada".into()) }
}

const PUSH: [&str; 4] = ["push", "-u", "ada", "refs/heads/topic:refs/heads/topic"];

#[test]
fn push_to_your_fork_opens_the_fork_dialog_for_the_base() {
    let (mut st, work, bare) = with_host("fork-pr-open");
    let fx = update(&mut st, Msg::Forge(ForgeMsg::PushToMyFork { branch: "topic".into(), target: None }), Instant::now());
    assert!(matches!(&fx[..], [Effect::Remote(RemoteEffect::FindMyFork(r))] if r.target == origin()), "{fx:?}");
    let f = creator(&st);
    assert_eq!((f.remote.as_str(), f.then_pr.clone()), ("origin", Some(ForkThenPr { branch: "topic".into(), target: None })));
    cleanup(&work, &bare);
}

#[test]
fn a_fork_the_user_has_is_added_and_pushed_in_one_job() {
    let (mut st, work, bare) = with_host("fork-pr-mine");
    open(&mut st, Some(fork("ada")));
    let cmds = git_job(&run(&mut st, RemoteMsg::AddMyFork));
    let add = [args(&["remote", "add", "ada", "https://github.com/ada/r.git"]), args(&["fetch", "--prune", "ada"]), args(&PUSH)];
    assert_eq!(cmds, add);
    assert_eq!(then_pr(&st), Some(pushed_to_ada()));
    assert!(st.modal.is_none());
    cleanup(&work, &bare);
}

#[test]
fn a_fork_that_already_is_a_remote_is_only_pushed_to() {
    let (mut st, work, bare, ada) = with_fork("fork-pr-added");
    open(&mut st, Some(fork("ada")));
    assert_eq!(git_job(&run(&mut st, RemoteMsg::AddMyFork)), [args(&PUSH)]);
    assert_eq!(then_pr(&st), Some(pushed_to_ada()));
    cleanup(&work, &bare);
    let _ = std::fs::remove_dir_all(&ada);
}

#[test]
fn a_created_fork_is_added_and_pushed_once_ready() {
    let (mut st, work, bare) = with_host("fork-pr-created");
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    let created = |st: &mut AppState, ready: bool| {
        open(st, None);
        run(st, RemoteMsg::CreateFork);
        let result = Box::new(Ok(CreatedFork { fork: fork("ada"), ready }));
        run(st, RemoteMsg::ForkCreated { workdir: workdir.clone(), target: origin(), name: "ada".into(), result })
    };
    let cmds = git_job(&created(&mut st, true));
    let add = [args(&["remote", "add", "ada", "https://github.com/ada/r.git"]), args(&["fetch", "--prune", "ada"]), args(&PUSH)];
    assert_eq!(cmds, add);
    assert_eq!(then_pr(&st), Some(pushed_to_ada()));
    st.job = None;

    assert_eq!(git_job(&created(&mut st, false)), [args(&["remote", "add", "ada", "https://github.com/ada/r.git"])]);
    assert_eq!(then_pr(&st), None);
    assert!(st.log.iter().any(|l| l.text.contains("still preparing the fork; push topic to ada in a minute")));
    cleanup(&work, &bare);
}
