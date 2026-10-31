//! Headless tests for checking out a remote branch through a local branch tracking it: the
//! fast-forward in the same job, the diverged branch left where it is, a chosen one of several
//! trackers, the refusals, the palette's route, and a tracking branch under another name from
//! the New branch dialog. Commands are checked as emitted, and end to end against a repository
//! with a bare `origin`.

use super::branch_tests::{remote_commit, with_origin};
use super::tests::{clean_tree, drive, git};
use super::update;
use crate::effect::Effect;
use crate::msg::{BranchMsg, JobMsg, Msg, PaletteMsg, RepoMsg};
use crate::state::palette::PaletteCommand;
use crate::state::{AppState, Modal};
use git2::Oid;
use glowfly_core::branch_ops::create_tracking_cmd;
use glowfly_core::gitcli::args;
use glowfly_core::repo::Tracking;
use glowfly_core::undo::UndoKind;
use std::path::Path;
use std::time::Instant;

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// The commands `msg` starts and how they are recorded for undo, or nothing. Clears the job so
/// the next message can start one.
fn run_git(st: &mut AppState, msg: Msg) -> (Vec<Vec<String>>, Option<UndoKind>) {
    let fx = update(st, msg, Instant::now());
    st.job = None;
    match &fx[..] {
        [Effect::RunGit { cmds, record, .. }] => (cmds.clone(), record.clone()),
        [] => (Vec::new(), None),
        other => panic!("unexpected effects {other:?}"),
    }
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

fn checkout(remote: &str, name: &str) -> Msg {
    Msg::Branch(BranchMsg::CheckoutRemote { remote: remote.into(), name: name.into() })
}

fn tracking(local: &str) -> Msg {
    Msg::Branch(BranchMsg::CheckoutTracking { remote: "origin".into(), name: "feature".into(), local: local.into() })
}

fn tip(dir: &Path, refname: &str) -> Oid {
    git2::Repository::open(dir).unwrap().refname_to_id(refname).unwrap()
}

/// Adds a commit on top of local `branch` without checking it out.
fn local_commit(dir: &Path, branch: &str) {
    let oid = glowfly_core::testkit::git_out(dir, &["commit-tree", &format!("{branch}^{{tree}}"), "-p", branch, "-m", "local"]);
    git(dir, &["update-ref", &format!("refs/heads/{branch}"), oid.trim()]);
}

/// `with_origin` with `origin/feature` one commit ahead of the local `feature` tracking it.
fn behind(name: &str) -> (AppState, std::path::PathBuf, std::path::PathBuf) {
    let (mut st, work, bare) = with_origin(name);
    clean_tree(&work);
    remote_commit(&bare, "feature", "f.txt");
    git(&work, &["fetch", "-q", "origin"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    (st, work, bare)
}

#[test]
fn checkout_fast_forwards_a_tracking_branch_that_is_behind() {
    let (mut st, work, bare) = behind("rco-ff");
    let (from, to) = (tip(&work, "refs/heads/feature"), tip(&work, "refs/remotes/origin/feature"));
    let (cmds, record) = run_git(&mut st, checkout("origin", "feature"));
    assert_eq!(
        cmds,
        vec![
            args(&["merge-base", "--is-ancestor", &from.to_string(), &to.to_string()]),
            args(&["update-ref", "-m", "fast-forward to origin/feature", "refs/heads/feature", &to.to_string(), &from.to_string()]),
            args(&["checkout", "feature", "--"]),
        ]
    );
    assert_eq!(record, Some(UndoKind::Checkout));
    drive(&mut st, checkout("origin", "feature"));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.head_branch.as_deref(), Some("feature"));
    assert_eq!(snap.head_oid, Some(to), "checked out at the remote branch's tip");
    cleanup(&[&work, &bare]);
}

#[test]
fn checkout_leaves_a_diverged_tracking_branch_where_it_is() {
    let (mut st, work, bare) = behind("rco-diverged");
    local_commit(&work, "feature");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let before = tip(&work, "refs/heads/feature");
    let (cmds, _) = run_git(&mut st, checkout("origin", "feature"));
    assert_eq!(cmds, vec![args(&["checkout", "feature", "--"])], "no fast-forward");
    assert_eq!(
        last_log(&st),
        "feature and origin/feature have diverged (1 ahead, 1 behind), so feature is checked out without moving it. \
         Merge or rebase to bring them together."
    );
    drive(&mut st, checkout("origin", "feature"));
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("feature"));
    assert_eq!(tip(&work, "refs/heads/feature"), before, "not moved");
    cleanup(&[&work, &bare]);
}

#[test]
fn checkout_of_one_of_several_trackers() {
    let (mut st, work, bare) = behind("rco-several");
    git(&work, &["branch", "-q", "--track", "x", "origin/feature"]);
    git(&work, &["update-ref", "refs/heads/x", "refs/heads/feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let snap = st.snap.as_ref().unwrap();
    let names: Vec<&str> = crate::state::remote_branch::trackers(snap, "origin", "feature").iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["feature", "x"], "the same name first");
    let (cmds, _) = run_git(&mut st, tracking("x"));
    assert_eq!(cmds.last(), Some(&args(&["checkout", "x", "--"])));
    assert!(cmds[1].contains(&"refs/heads/x".to_owned()), "x is fast-forwarded, not feature: {cmds:?}");
    drive(&mut st, tracking("x"));
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("x"));
    assert_eq!(tip(&work, "refs/heads/x"), tip(&work, "refs/remotes/origin/feature"));
    assert_ne!(tip(&work, "refs/heads/feature"), tip(&work, "refs/heads/x"), "feature stays behind");
    cleanup(&[&work, &bare]);
}

#[test]
fn checkout_tracking_refusals() {
    let (mut st, work, bare) = behind("rco-refused");
    let wt = work.with_file_name(format!("{}-wt", work.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&wt);
    git(&work, &["worktree", "add", "-q", &wt.display().to_string(), "feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let before = tip(&work, "refs/heads/feature");
    assert!(run_git(&mut st, tracking("feature")).0.is_empty(), "another worktree has feature");
    assert!(last_log(&st).starts_with("feature is checked out in the worktree at "), "{}", last_log(&st));
    assert_eq!(tip(&work, "refs/heads/feature"), before, "not moved");
    assert!(run_git(&mut st, tracking("main")).0.is_empty());
    assert_eq!(last_log(&st), "main no longer tracks origin/feature.");
    git(&work, &["worktree", "remove", "--force", &wt.display().to_string()]);
    cleanup(&[&work, &bare, &wt]);
}

/// The checked-out tracker is fast-forwarded in place, as Fast-forward does.
#[test]
fn checkout_of_the_checked_out_tracker_fast_forwards_it() {
    let (mut st, work, bare) = behind("rco-head");
    let snap = st.snap.as_mut().unwrap();
    let main = snap.locals.iter_mut().find(|b| b.name == "main").unwrap();
    assert!(main.is_head);
    main.tracking = Some(Tracking { remote: "origin".into(), merge: "feature".into() });
    main.upstream_ref = Some("refs/remotes/origin/feature".into());
    main.behind = 1;
    let (cmds, _) = run_git(&mut st, tracking("main"));
    assert_eq!(cmds, vec![args(&["merge", "--ff-only", "refs/remotes/origin/feature"])]);
    cleanup(&[&work, &bare]);
}

#[test]
fn palette_remote_checkout_goes_through_the_branch_message() {
    let (mut st, work, bare) = with_origin("rco-palette");
    clean_tree(&work);
    git(&work, &["push", "-q", "origin", "main:other"]);
    git(&work, &["fetch", "-q", "origin"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let run = |remote: &str, branch: &str| {
        Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::CheckoutRemote { remote: remote.into(), branch: branch.into() }))
    };
    assert_eq!(
        run_git(&mut st, run("origin", "other")),
        (vec![args(&["checkout", "--track", "origin/other", "--"])], Some(UndoKind::Checkout))
    );
    // A palette entry gone stale is refused as the menu's is.
    assert!(run_git(&mut st, run("origin", "gone")).0.is_empty());
    assert_eq!(last_log(&st), "origin/gone no longer exists.");
    cleanup(&[&work, &bare]);
}

#[test]
fn new_branch_from_a_remote_branch_tracks_it_under_another_name() {
    let (mut st, work, bare) = with_origin("rco-track-as");
    clean_tree(&work);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let fx = update(&mut st, Msg::Branch(BranchMsg::NewTrackingBranch { remote: "origin".into(), name: "feature".into() }), Instant::now());
    assert!(fx.is_empty());
    let tip = tip(&work, "refs/remotes/origin/feature").to_string();
    assert!(
        matches!(&st.modal, Some(Modal::NewBranch { start: Some(s), track: Some((r, n)), checkout: true, .. }) if *s == tip && r == "origin" && n == "feature")
    );
    st.modal = None;
    // What the dialog's Create sends (ui/branch_dialogs.rs).
    let create =
        Msg::Job(JobMsg::Git { label: "Create branch".into(), cmds: vec![create_tracking_cmd("mine", "origin", "feature", true)] });
    drive(&mut st, create);
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.head_branch.as_deref(), Some("mine"));
    let mine = snap.head_info().unwrap();
    assert_eq!(mine.tracking, Some(Tracking { remote: "origin".into(), merge: "feature".into() }));
    let trackers = crate::state::remote_branch::trackers(snap, "origin", "feature");
    assert_eq!(trackers.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), ["mine", "feature"], "checked-out first");

    assert!(
        update(&mut st, Msg::Branch(BranchMsg::NewTrackingBranch { remote: "origin".into(), name: "gone".into() }), Instant::now())
            .is_empty()
    );
    assert_eq!(last_log(&st), "origin/gone no longer exists.");
    cleanup(&[&work, &bare]);
}
