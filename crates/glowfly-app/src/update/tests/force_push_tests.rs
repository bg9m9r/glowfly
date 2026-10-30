//! Headless tests for Force push: the lease names the remote commit the snapshot last saw, so
//! a remote branch that moved since (and was fetched in the background) is never overwritten;
//! the same from a branch row for a branch that isn't checked out, its refusals, the rejected
//! push toast's Force push with lease, and Push to all remotes.

use super::branch_tests::{remote_commit, with_origin};
use super::tests::{drive, finish, git};
use super::update;
use crate::effect::Effect;
use crate::logs::LogsMsg;
use crate::msg::{BranchMsg, JobMsg, Msg, RepoMsg};
use crate::state::branches::push::{force_blocked, force_confirm};
use crate::state::{AppState, ToastAction};
use glowfly_core::gitcli::{args, run};
use glowfly_core::repo::BranchInfo;
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

/// Gives feature (not checked out) a commit that is pushed and then rewritten away, so
/// origin/feature has 1 commit feature lacks; reloads the snapshot.
fn diverge_feature(st: &mut AppState, work: &Path) {
    git(work, &["switch", "-q", "feature"]);
    git(work, &["commit", "-q", "--allow-empty", "-m", "pushed"]);
    git(work, &["push", "-q"]);
    git(work, &["reset", "-q", "--hard", "HEAD~1"]);
    git(work, &["commit", "-q", "--allow-empty", "-m", "rewritten"]);
    git(work, &["switch", "-q", "main"]);
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

fn branch<'a>(st: &'a AppState, name: &str) -> &'a BranchInfo {
    st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == name).expect("the branch")
}

/// The confirmation the branch row's Force push opens: its body and the message it runs.
fn confirmation(st: &AppState, name: &str) -> (String, Msg) {
    let t = st.settings.lang();
    match force_confirm(t, st.snap.as_ref().unwrap(), branch(st, name)) {
        Ok(Msg::ConfirmDanger { body, then, .. }) => (body, *then),
        Ok(_) => panic!("not a confirmation"),
        Err(why) => panic!("refused: {why}"),
    }
}

#[test]
fn force_push_of_a_branch_that_is_not_checked_out_leases_its_upstream() {
    let (mut st, work, bare) = with_origin("force-lease-other");
    diverge_feature(&mut st, &work);
    let seen = rev(&work, "origin/feature");
    let (body, then) = confirmation(&st, "feature");
    assert!(body.contains("origin/feature has 1 commit that feature doesn't have"), "{body}");
    let fx = update(&mut st, then, Instant::now());
    match &fx[..] {
        [Effect::RunGit { cmds, .. }] => assert_eq!(
            cmds[..],
            [args(&[
                "push",
                &format!("--force-with-lease=refs/heads/feature:{seen}"),
                "--force-if-includes",
                "origin",
                "refs/heads/feature:refs/heads/feature",
            ])]
        ),
        other => panic!("expected a push, got {other:?}"),
    }
    finish(&mut st, fx);
    assert_eq!(rev(&bare, "feature"), rev(&work, "feature"), "{:?}", st.log.last().map(|l| &l.text));
    assert_eq!(rev(&work, "HEAD"), rev(&work, "main"), "main stays checked out");
    cleanup(&[&work, &bare]);
}

#[test]
fn force_push_refuses_the_default_branch_and_a_moved_upstream() {
    let (mut st, work, bare) = with_origin("force-lease-refused");
    let t = st.settings.lang();
    let why = force_blocked(t, st.snap.as_ref().unwrap(), branch(&st, "main")).expect("main is refused");
    assert_eq!(why, "main is the default branch of origin, so it is never force pushed: others build on it.");
    let main = Msg::Branch(BranchMsg::ForcePush { branch: "main".into(), seen: None });
    assert!(update(&mut st, main, Instant::now()).is_empty());
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some(why.as_str()));
    // The confirmation counted origin/feature at one commit; a fetch since moved it.
    diverge_feature(&mut st, &work);
    let (_, then) = confirmation(&st, "feature");
    remote_commit(&bare, "feature", "theirs.txt");
    git(&work, &["fetch", "-q", "origin"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(update(&mut st, then, Instant::now()).is_empty(), "nothing is pushed");
    let log = st.log.last().map(|l| l.text.clone()).unwrap_or_default();
    assert!(log.starts_with("origin/feature moved since you confirmed"), "{log}");
    cleanup(&[&work, &bare]);
}

/// The fixes kept for the newest toast's buttons, first then second: label and message.
fn toast_fixes(st: &AppState) -> Vec<(String, &Msg)> {
    let t = st.toasts.last().expect("a toast");
    let fix = |a: &Option<ToastAction>, alt: bool| {
        let a = a.as_ref()?;
        let Msg::Logs(LogsMsg::Fix(id) | LogsMsg::AltFix(id)) = *a.msg else { return None };
        let kept = if alt { st.logs.alt_fix(id) } else { st.logs.fix(id) }?;
        Some((kept.label.clone(), &*kept.msg))
    };
    [fix(&t.action, false), fix(&t.alt, true)].into_iter().flatten().collect()
}

/// Whether `msg` asks to force push branch `name`.
fn forces(msg: &Msg, name: &str) -> bool {
    let Msg::ConfirmDanger { then, .. } = msg else { return false };
    matches!(&**then, Msg::Branch(BranchMsg::ForcePush { branch, .. }) if branch == name)
}

#[test]
fn a_push_rejected_as_behind_offers_force_push_for_that_branch() {
    let (mut st, work, bare) = with_origin("force-lease-toast");
    // Someone else pushed to feature, unfetched, and feature has a commit of its own: git
    // rejects the push the snapshot let through.
    remote_commit(&bare, "feature", "theirs.txt");
    git(&work, &["switch", "-q", "feature"]);
    git(&work, &["commit", "-q", "--allow-empty", "-m", "ours"]);
    git(&work, &["switch", "-q", "main"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    // Not checked out: Force push with lease is the only fix.
    drive(&mut st, Msg::Branch(BranchMsg::PushBranch { branch: "feature".into(), remote: None }));
    let fixes = toast_fixes(&st);
    let labels: Vec<&str> = fixes.iter().map(|f| f.0.as_str()).collect();
    assert_eq!(labels, ["Force push with lease…"], "{:?}", st.toasts.last().map(|t| &t.text));
    assert!(forces(fixes[0].1, "feature"));
    // Checked out: Pull first, Force push with lease beside it.
    git(&work, &["switch", "-q", "feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Job(JobMsg::Push { force: false }));
    let fixes = toast_fixes(&st);
    let labels: Vec<&str> = fixes.iter().map(|f| f.0.as_str()).collect();
    assert_eq!(labels, ["Pull", "Force push with lease…"]);
    assert!(forces(fixes[1].1, "feature"), "only the rejected branch");
    cleanup(&[&work, &bare]);
}

#[test]
fn push_to_all_remotes_pushes_each_in_turn_and_names_the_ones_that_failed() {
    let (mut st, work, bare) = with_origin("push-all");
    let backup = bare.with_file_name(format!("{}-backup.git", work.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&backup);
    std::fs::create_dir_all(&backup).unwrap();
    git(&backup, &["init", "-q", "--bare"]);
    let gone = bare.with_file_name("glowfly-push-all-missing.git");
    git(&work, &["remote", "add", "broken", &gone.display().to_string()]);
    git(&work, &["remote", "add", "backup", &backup.display().to_string()]);
    git(&work, &["commit", "-q", "--allow-empty", "-m", "more"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Branch(BranchMsg::PushAll("main".into())));
    let headline = st.toasts.last().map(|t| t.text.clone()).unwrap_or_default();
    assert_eq!(headline, "Pushing main failed on broken (1 of 3 remotes). The log shows why.");
    assert_eq!(rev(&backup, "main"), rev(&work, "main"), "a remote after the failed one still got it");
    assert_eq!(rev(&bare, "main"), rev(&work, "main"));
    // No Try again: it would push again to the remotes that took it.
    assert!(toast_fixes(&st).is_empty() && st.logs.job_retry.is_none());
    let upstream = branch(&st, "main").tracking.as_ref().map(|t| t.display());
    assert_eq!(upstream.as_deref(), Some("origin/main"), "the upstream stays");
    cleanup(&[&work, &bare, &backup]);
}
