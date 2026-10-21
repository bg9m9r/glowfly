//! Headless tests for Force push: the lease names the remote commit the snapshot last saw, so
//! a remote branch that moved since (and was fetched in the background) is never overwritten.

use super::branch_tests::{remote_commit, with_origin};
use super::tests::{drive, git};
use super::update;
use crate::effect::Effect;
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use glowfly_core::gitcli::{args, run};
use std::path::Path;
use std::time::Instant;

fn rev(dir: &Path, what: &str) -> String {
    let (ok, out) = run(dir, &args(&["rev-parse", what]));
    assert!(ok, "{out}");
    out.trim().to_owned()
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// Rewrites main's last commit locally (so the push needs force) and reloads the snapshot.
fn rewrite_main(st: &mut crate::state::AppState, work: &Path) {
    git(work, &["commit", "-q", "--amend", "-m", "rewritten"]);
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

#[test]
fn force_push_leases_the_commit_the_snapshot_saw() {
    let (mut st, work, bare) = with_origin("force-lease-cmd");
    rewrite_main(&mut st, &work);
    let seen = rev(&work, "origin/main");
    let fx = update(&mut st, Msg::Job(JobMsg::Push { force: true }), Instant::now());
    match &fx[..] {
        [Effect::RunGit { cmds, .. }] => assert_eq!(
            cmds[..],
            [args(&[
                "push",
                &format!("--force-with-lease=refs/heads/main:{seen}"),
                "--force-if-includes",
                "origin",
                "refs/heads/main:refs/heads/main",
            ])]
        ),
        other => panic!("expected a push, got {other:?}"),
    }
    cleanup(&[&work, &bare]);
}

#[test]
fn force_push_replaces_an_unchanged_remote() {
    let (mut st, work, bare) = with_origin("force-lease-ok");
    rewrite_main(&mut st, &work);
    drive(&mut st, Msg::Job(JobMsg::Push { force: true }));
    assert_eq!(rev(&bare, "main"), rev(&work, "main"), "{:?}", st.log.last().map(|l| &l.text));
    cleanup(&[&work, &bare]);
}

#[test]
fn force_push_refuses_when_the_remote_moved_after_the_snapshot() {
    let (mut st, work, bare) = with_origin("force-lease-moved");
    rewrite_main(&mut st, &work);
    // Someone else pushes, and a background fetch updates origin/main without the snapshot
    // being reloaded: bare --force-with-lease would now overwrite their commit.
    remote_commit(&bare, "main", "theirs.txt");
    git(&work, &["fetch", "-q", "origin"]);
    let theirs = rev(&bare, "main");
    assert_eq!(rev(&work, "origin/main"), theirs);
    drive(&mut st, Msg::Job(JobMsg::Push { force: true }));
    assert_eq!(rev(&bare, "main"), theirs, "their commit survives");
    let log = st.log.last().map(|l| l.text.clone()).unwrap_or_default();
    assert!(log.contains("Force push FAILED") && log.contains("stale info"), "{log}");
    cleanup(&[&work, &bare]);
}

#[test]
fn force_push_of_a_new_branch_expects_it_absent() {
    let (mut st, work, bare) = with_origin("force-lease-new");
    git(&work, &["switch", "-q", "-c", "topic"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let fx = update(&mut st, Msg::Job(JobMsg::Push { force: true }), Instant::now());
    match &fx[..] {
        [Effect::RunGit { cmds, .. }] => assert_eq!(
            cmds[..],
            [args(&[
                "push",
                "--force-with-lease=refs/heads/topic:",
                "--force-if-includes",
                "-u",
                "origin",
                "refs/heads/topic:refs/heads/topic"
            ])]
        ),
        other => panic!("expected a push, got {other:?}"),
    }
    cleanup(&[&work, &bare]);
}
