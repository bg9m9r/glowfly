//! Headless tests for the remote a pull request's push goes to under push config:
//! `branch.<name>.pushRemote`, then `remote.pushDefault`, before the fork the branch tracks or
//! the code host's remote, and a configured remote that can't be used logged once.

use super::push_pr_fork_tests::with_fork;
use super::push_pr_tests::cleanup;
use super::tests::{drive, git};
use super::update;
use crate::effect::Effect;
use crate::msg::{ForgeMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::pr_repos::{UnusablePush, pr_push_target};
use glowfly_core::gitcli::args;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn push_and_start(st: &mut AppState, branch: &str) -> Vec<Vec<Vec<String>>> {
    let fx = update(st, Msg::Forge(ForgeMsg::PushAndStartPr { branch: branch.into(), target: None }), Instant::now());
    st.job = None;
    fx.into_iter().filter_map(|e| if let Effect::RunGit { cmds, .. } = e { Some(cmds) } else { None }).collect()
}

fn push(remote: &str, branch: &str, upstream: bool) -> Vec<Vec<Vec<String>>> {
    let spec = format!("refs/heads/{branch}:refs/heads/{branch}");
    let cmd = if upstream { args(&["push", "-u", remote, &spec]) } else { args(&["push", remote, &spec]) };
    vec![vec![cmd]]
}

fn config(st: &mut AppState, work: &Path, key: &str, value: &str) {
    git(work, &["config", key, value]);
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

fn done(work: PathBuf, bare: PathBuf, ada: PathBuf) {
    cleanup(&work, &bare);
    let _ = std::fs::remove_dir_all(&ada);
}

fn warnings(st: &AppState) -> usize {
    st.log.iter().filter(|l| l.text.contains("which isn't a remote on GitHub")).count()
}

#[test]
fn push_remote_then_push_default_choose_the_remote() {
    let (mut st, work, bare, ada) = with_fork("pr-config-order");
    assert_eq!(push_and_start(&mut st, "topic"), push("origin", "topic", true), "no push config: the code host's");
    config(&mut st, &work, "remote.pushDefault", "ada");
    assert_eq!(push_and_start(&mut st, "topic"), push("ada", "topic", true), "pushDefault");
    config(&mut st, &work, "branch.topic.pushRemote", "origin");
    assert_eq!(push_and_start(&mut st, "topic"), push("origin", "topic", true), "the branch's pushRemote wins");
    assert_eq!(warnings(&st), 0);
    done(work, bare, ada);
}

#[test]
fn a_triangular_branch_keeps_the_upstream_it_pulls_from() {
    let (mut st, work, bare, ada) = with_fork("pr-config-triangular");
    git(&work, &["branch", "-q", "--set-upstream-to=origin/main", "topic"]);
    config(&mut st, &work, "branch.topic.pushRemote", "ada");
    assert_eq!(push_and_start(&mut st, "topic"), push("ada", "topic", false), "its own name, no -u");
    done(work, bare, ada);
}

#[test]
fn a_missing_push_remote_falls_back_and_is_logged_once() {
    let (mut st, work, bare, ada) = with_fork("pr-config-missing");
    git(&work, &["branch", "-q", "--set-upstream-to=origin/main", "topic"]);
    git(&work, &["config", "remote.pushDefault", "ada"]);
    config(&mut st, &work, "branch.topic.pushRemote", "gone");
    let (snap, fs) = (st.snap.as_ref().unwrap(), st.session.forge.as_ref().unwrap());
    let b = snap.locals.iter().find(|b| b.name == "topic").unwrap();
    let unusable = UnusablePush { key: "branch.topic.pushRemote".into(), remote: "gone".into() };
    assert_eq!(pr_push_target(b, snap, fs), ("ada".to_owned(), Some(unusable)), "on to pushDefault");
    assert_eq!(push_and_start(&mut st, "topic"), push("ada", "topic", false));
    assert_eq!(push_and_start(&mut st, "topic"), push("ada", "topic", false));
    assert_eq!(warnings(&st), 1, "logged once: {:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());
    let line = st.log.iter().find(|l| l.text.contains("gone")).map(|l| l.text.clone());
    let want = "branch.topic.pushRemote names gone, which isn't a remote on GitHub, so topic is pushed to ada.";
    assert_eq!(line.as_deref(), Some(want));
    done(work, bare, ada);
}

#[test]
fn a_push_remote_off_the_code_host_falls_back() {
    let (mut st, work, bare, ada) = with_fork("pr-config-off-host");
    git(&work, &["remote", "add", "backup", "https://example.com/backup/r.git"]);
    config(&mut st, &work, "remote.pushDefault", "backup");
    assert_eq!(push_and_start(&mut st, "topic"), push("origin", "topic", true), "the code host's remote, as without config");
    assert_eq!(warnings(&st), 1);
    done(work, bare, ada);
}
