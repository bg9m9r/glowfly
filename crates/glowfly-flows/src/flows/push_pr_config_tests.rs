//! Headless tests for the remote a pull request's push goes to under push config:
//! `branch.<name>.pushRemote`, then `remote.pushDefault`, before the fork the branch tracks or
//! the code host's remote; a configured remote that is missing or off the code host refuses the
//! push, as git's own push would never move on to the next choice.

use super::push_pr_fork_tests::with_fork;
use super::push_pr_tests::cleanup;
use super::tests::{drive, git};
use super::update;
use crate::effect::Effect;
use crate::msg::{ForgeMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::pr_repos::{UnusablePush, pr_push_target};
use crate::state::push_pr::{HostBranch, host_branch};
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

/// Log lines refusing a push for a pull request.
fn refusals(st: &AppState) -> usize {
    st.log.iter().filter(|l| l.text.contains("isn't pushed for a pull request")).count()
}

#[test]
fn push_remote_then_push_default_choose_the_remote() {
    let (mut st, work, bare, ada) = with_fork("pr-config-order");
    assert_eq!(push_and_start(&mut st, "topic"), push("origin", "topic", true), "no push config: the code host's");
    config(&mut st, &work, "remote.pushDefault", "ada");
    assert_eq!(push_and_start(&mut st, "topic"), push("ada", "topic", true), "pushDefault");
    config(&mut st, &work, "branch.topic.pushRemote", "origin");
    assert_eq!(push_and_start(&mut st, "topic"), push("origin", "topic", true), "the branch's pushRemote wins");
    assert_eq!(refusals(&st), 0);
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
fn a_missing_push_remote_refuses_the_push() {
    // git would fail to push to a remote that isn't there, so neither pushDefault nor origin is tried.
    let (mut st, work, bare, ada) = with_fork("pr-config-missing");
    git(&work, &["branch", "-q", "--set-upstream-to=origin/main", "topic"]);
    git(&work, &["config", "remote.pushDefault", "ada"]);
    config(&mut st, &work, "branch.topic.pushRemote", "gone");
    let (snap, fs) = (st.snap.as_ref().unwrap(), st.session.forge.as_ref().unwrap());
    let b = snap.locals.iter().find(|b| b.name == "topic").unwrap();
    let unusable = UnusablePush { key: "branch.topic.pushRemote".into(), remote: "gone".into(), missing: true };
    assert_eq!(pr_push_target(b, snap, fs), Err(unusable), "not on to pushDefault");
    assert!(push_and_start(&mut st, "topic").is_empty(), "nothing pushed");
    let why = "The configured push remote \"gone\" (branch.topic.pushRemote) isn't a remote in this repository, so topic \
        isn't pushed for a pull request. Add that remote, or point branch.topic.pushRemote at a remote on GitHub.";
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some(why));
    let (snap, fs) = (st.snap.as_ref().unwrap(), st.session.forge.as_ref().unwrap());
    let b = snap.locals.iter().find(|b| b.name == "topic").unwrap();
    assert_eq!(host_branch(st.settings.lang(), b, snap, fs), Some(HostBranch::Blocked(why.to_owned())), "the menus say why");

    // A missing pushDefault, with no pushRemote, refuses it too.
    git(&work, &["config", "--unset", "branch.topic.pushRemote"]);
    config(&mut st, &work, "remote.pushDefault", "gone");
    assert!(push_and_start(&mut st, "topic").is_empty(), "not pushed to origin either");
    assert!(st.log.last().is_some_and(|l| l.text.starts_with("The configured push remote \"gone\" (remote.pushDefault)")));
    done(work, bare, ada);
}

#[test]
fn a_push_remote_off_the_code_host_refuses_the_push() {
    // The user meant the branch for backup, so it isn't pushed to origin instead.
    let (mut st, work, bare, ada) = with_fork("pr-config-off-host");
    git(&work, &["remote", "add", "backup", "https://example.com/backup/r.git"]);
    config(&mut st, &work, "remote.pushDefault", "backup");
    assert!(push_and_start(&mut st, "topic").is_empty(), "nothing pushed");
    let why = "remote.pushDefault names backup, which isn't a repository on GitHub, so topic isn't pushed for a pull request. \
        Point remote.pushDefault at a remote on GitHub, or pick the repository in the create dialog.";
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some(why));
    let (snap, fs) = (st.snap.as_ref().unwrap(), st.session.forge.as_ref().unwrap());
    let b = snap.locals.iter().find(|b| b.name == "topic").unwrap();
    assert_eq!(host_branch(st.settings.lang(), b, snap, fs), Some(HostBranch::Blocked(why.to_owned())), "the menus say why");
    // A pushRemote that names a PR repository still wins over it.
    config(&mut st, &work, "branch.topic.pushRemote", "ada");
    assert_eq!(push_and_start(&mut st, "topic"), push("ada", "topic", true));
    assert_eq!(refusals(&st), 1, "only the first push was refused");
    done(work, bare, ada);
}
