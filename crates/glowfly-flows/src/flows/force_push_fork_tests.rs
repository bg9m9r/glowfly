//! Headless tests for Force push when `remote.pushDefault` sends a plain push to a fork
//! (#885), on real repositories: main and feature pushed to a bare `origin` (their upstreams)
//! and feature also to a bare `fork`. Each test sets push.default and blanks remote.pushDefault
//! in the repository's own config (`push_target_tests::setup`), then points it at the fork. The
//! toolbar's Force push, a branch row's and the rejected push toast's go to the fork and leave
//! origin alone, the lease is the fork's tip, and what git refuses stays refused.

use super::branch_tests::remote_commit;
use super::force_push_tests::{branch, cleanup, confirmation, fix_labels, rev, toast_fixes};
use super::push_target_tests::{config, setup};
use super::tests::{drive, finish, git};
use super::update;
use glowfly_core::gitcli::args;
use glowfly_model::effect::{Effect, JobEffect};
use glowfly_model::msg::{BranchMsg, JobMsg, Msg, RepoMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::branches::push::{force_blocked, force_confirm};
use glowfly_model::state::toolbar_tips;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// `setup(name, "simple")` with feature also on the fork, and remote.pushDefault set to it:
/// a plain push of feature goes to fork/feature, its upstream being origin/feature.
fn forked(name: &str) -> (AppState, PathBuf, PathBuf, PathBuf) {
    let (mut st, work, bare, fork) = setup(name, "simple");
    git(&work, &["push", "-q", "fork", "feature"]);
    config(&mut st, &work, &[("remote.pushDefault", "fork")]);
    (st, work, bare, fork)
}

/// Checks out feature and rewrites its last commit, so a push needs force; reloads.
fn rewrite_feature(st: &mut AppState, work: &Path) {
    git(work, &["switch", "-q", "feature"]);
    git(work, &["commit", "-q", "--amend", "-m", "rewritten"]);
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

/// Gives feature (not checked out) a commit that is pushed to the fork and then rewritten away,
/// so fork/feature has 1 commit feature lacks; reloads.
fn diverge_from_fork(st: &mut AppState, work: &Path) {
    git(work, &["switch", "-q", "feature"]);
    git(work, &["commit", "-q", "--allow-empty", "-m", "pushed"]);
    git(work, &["push", "-q", "fork", "feature"]);
    git(work, &["reset", "-q", "--hard", "HEAD~1"]);
    git(work, &["commit", "-q", "--allow-empty", "-m", "rewritten"]);
    git(work, &["switch", "-q", "main"]);
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

/// The git command `fx` starts.
fn git_cmd(fx: &[Effect]) -> Vec<String> {
    fx.iter()
        .find_map(|e| match e {
            Effect::Job(JobEffect::RunGit { cmds, .. }) => cmds.first().cloned(),
            _ => None,
        })
        .expect("a git job")
}

/// The remote a fix's confirmation force pushes to.
fn force_remote(msg: &Msg) -> Option<&str> {
    let Msg::ConfirmDanger { then, .. } = msg else { return None };
    match &**then {
        Msg::Branch(BranchMsg::ForcePush { remote, .. }) => Some(remote),
        _ => None,
    }
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

#[test]
fn the_toolbar_force_push_rewrites_the_fork_and_leaves_origin_alone() {
    let (mut st, work, bare, fork) = forked("force-fork-toolbar");
    rewrite_feature(&mut st, &work);
    let seen = rev(&work, "fork/feature");
    let origin_before = rev(&bare, "feature");
    let fx = update(&mut st, Msg::Job(JobMsg::Push { force: true }), Instant::now());
    assert_eq!(
        git_cmd(&fx),
        args(&[
            "push",
            &format!("--force-with-lease=refs/heads/feature:{seen}"),
            "--force-if-includes",
            "--",
            "fork",
            "refs/heads/feature:refs/heads/feature",
        ])
    );
    finish(&mut st, fx);
    assert_eq!(rev(&fork, "feature"), rev(&work, "feature"), "{}", last_log(&st));
    assert_eq!(rev(&bare, "feature"), origin_before, "origin is left alone");
    cleanup(&[&work, &bare, &fork]);
}

#[test]
fn a_row_force_push_counts_and_leases_the_fork() {
    let (mut st, work, bare, fork) = forked("force-fork-row");
    diverge_from_fork(&mut st, &work);
    let origin_before = rev(&bare, "feature");
    let (body, then) = confirmation(&st, "feature");
    assert!(body.contains("fork/feature has 1 commit that feature doesn't have"), "{body}");
    drive(&mut st, then);
    assert_eq!(rev(&fork, "feature"), rev(&work, "feature"), "{}", last_log(&st));
    assert_eq!(rev(&bare, "feature"), origin_before, "origin is left alone");
    assert_eq!(branch(&st, "feature").tracking.as_ref().map(|t| t.display()).as_deref(), Some("origin/feature"));
    cleanup(&[&work, &bare, &fork]);
}

#[test]
fn a_push_rejected_by_the_fork_offers_force_push_toward_it() {
    let (mut st, work, bare, fork) = forked("force-fork-toast");
    remote_commit(&fork, "feature", "theirs.txt");
    git(&work, &["switch", "-q", "feature"]);
    git(&work, &["commit", "-q", "--allow-empty", "-m", "ours"]);
    git(&work, &["switch", "-q", "main"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    // The row's Push goes to the fork, which rejects it.
    drive(&mut st, Msg::Branch(BranchMsg::PushBranch { branch: "feature".into(), remote: None }));
    assert_eq!(fix_labels(&st), ["Force push with lease…"], "{:?}", st.toasts.last().map(|t| &t.text));
    assert_eq!(force_remote(toast_fixes(&st)[0].1), Some("fork"));
    // The toolbar's plain push of the checked-out branch: the remote is read from git's output.
    git(&work, &["switch", "-q", "feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Job(JobMsg::Push { force: false }));
    let fixes = toast_fixes(&st);
    assert_eq!(fixes.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(), ["Pull", "Force push with lease…"]);
    assert_eq!(force_remote(fixes[1].1), Some("fork"));
    cleanup(&[&work, &bare, &fork]);
}

#[test]
fn a_fork_tip_that_moved_since_the_snapshot_is_never_overwritten() {
    let (mut st, work, bare, fork) = forked("force-fork-moved");
    diverge_from_fork(&mut st, &work);
    let (_, then) = confirmation(&st, "feature");
    remote_commit(&fork, "feature", "theirs.txt");
    git(&work, &["fetch", "-q", "fork"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(update(&mut st, then, Instant::now()).is_empty(), "nothing is pushed");
    assert!(last_log(&st).starts_with("fork/feature moved since you confirmed"), "{}", last_log(&st));
    // The toolbar carries no tip to compare: git's own lease check refuses.
    git(&work, &["switch", "-q", "feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let theirs = rev(&fork, "feature");
    git(&work, &["commit", "-q", "--amend", "--allow-empty", "-m", "again"]);
    remote_commit(&fork, "feature", "more.txt");
    git(&work, &["fetch", "-q", "fork"]);
    let theirs_now = rev(&fork, "feature");
    assert_ne!(theirs, theirs_now);
    drive(&mut st, Msg::Job(JobMsg::Push { force: true }));
    assert_eq!(rev(&fork, "feature"), theirs_now, "their commit survives");
    let log = last_log(&st);
    assert!(log.contains("Force push FAILED") && log.contains("stale info"), "{log}");
    cleanup(&[&work, &bare, &fork]);
}

#[test]
fn push_default_nothing_refuses_force_push_with_the_plain_push_text() {
    let (mut st, work, bare, fork) = forked("force-fork-nothing");
    git(&work, &["switch", "-q", "feature"]);
    config(&mut st, &work, &[("push.default", "nothing")]);
    let why = toolbar_tips::push_blocked(&st).expect("git refuses the plain push");
    let fx = update(&mut st, Msg::Job(JobMsg::Push { force: true }), Instant::now());
    assert!(fx.is_empty(), "no job");
    assert_eq!(last_log(&st), why);
    let t = st.settings.lang();
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(force_blocked(t, snap, branch(&st, "feature")).as_deref(), Some(why.as_str()));
    assert_eq!(force_confirm(t, snap, branch(&st, "feature")).err().as_deref(), Some(why.as_str()));
    cleanup(&[&work, &bare, &fork]);
}

#[test]
fn a_missing_push_remote_force_pushes_to_origin() {
    let (mut st, work, bare, fork) = forked("force-fork-missing");
    rewrite_feature(&mut st, &work);
    config(&mut st, &work, &[("branch.feature.pushRemote", "gone")]);
    let seen = rev(&work, "origin/feature");
    let fx = update(&mut st, Msg::Job(JobMsg::Push { force: true }), Instant::now());
    let cmd = git_cmd(&fx);
    assert!(cmd.contains(&format!("--force-with-lease=refs/heads/feature:{seen}")), "{cmd:?}");
    assert_eq!(cmd[cmd.len() - 3..], ["--", "origin", "refs/heads/feature:refs/heads/feature"]);
    cleanup(&[&work, &bare, &fork]);
}
