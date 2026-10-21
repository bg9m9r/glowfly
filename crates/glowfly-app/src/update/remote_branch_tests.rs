//! Headless tests for the remote branch menu's operations: checkout (the tracking local branch,
//! or a new one), compare with the current branch, conflict check and set upstream. Commands are
//! checked as emitted, and end to end against a repository with a bare `origin`.

use super::branch_tests::{remote_commit, with_origin};
use super::tests::{drive, git};
use super::update;
use crate::conflict_check::ConflictCheckMsg;
use crate::effect::Effect;
use crate::msg::{BranchMsg, Msg, RepoMsg, SelectionMsg};
use crate::state::remote_branch::{checkout_blocked, split, tracking_local};
use crate::state::{AppState, Selection};
use git2::Oid;
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

fn cmds(st: &mut AppState, msg: Msg) -> Vec<Vec<String>> {
    run_git(st, msg).0
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

fn checkout(remote: &str, name: &str) -> Msg {
    Msg::Branch(BranchMsg::CheckoutRemote { remote: remote.into(), name: name.into() })
}

fn remote_tip(st: &AppState, remote: &str, name: &str) -> Oid {
    crate::state::remote_branch::exists(st.snap.as_ref().unwrap(), remote, name).expect("remote branch")
}

#[test]
fn checkout_remote_creates_tracking_branch() {
    let (mut st, work, bare) = with_origin("rb-checkout-new");
    super::tests::clean_tree(&work);
    git(&work, &["push", "-q", "origin", "main:other"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let (cmds, record) = run_git(&mut st, checkout("origin", "other"));
    assert_eq!(cmds, vec![args(&["checkout", "--track", "origin/other", "--"])]);
    assert_eq!(record, Some(UndoKind::Checkout));
    drive(&mut st, checkout("origin", "other"));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.head_branch.as_deref(), Some("other"));
    let b = snap.locals.iter().find(|b| b.name == "other").expect("created");
    assert_eq!(b.upstream.as_deref(), Some("origin/other"));
    cleanup(&[&work, &bare]);
}

#[test]
fn checkout_remote_switches_to_tracking_local() {
    let (mut st, work, bare) = with_origin("rb-checkout-local");
    super::tests::clean_tree(&work);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(cmds(&mut st, checkout("origin", "feature")), vec![args(&["checkout", "feature", "--"])]);
    assert!(cmds(&mut st, checkout("origin", "main")).is_empty(), "main tracks origin/main and is checked out");
    assert_eq!(last_log(&st), "main is already checked out.");
    cleanup(&[&work, &bare]);
}

#[test]
fn checkout_remote_prefers_head_then_same_name() {
    let (mut st, work, bare) = with_origin("rb-tracking-pick");
    let snap = st.snap.as_mut().unwrap();
    let mut x = snap.locals.iter().find(|b| b.name == "feature").unwrap().clone();
    x.name = "x".into();
    snap.locals.insert(0, x);
    assert_eq!(tracking_local(snap, "origin", "feature").map(|b| b.name.as_str()), Some("feature"), "the same name before x");
    let main = snap.locals.iter_mut().find(|b| b.name == "main").unwrap();
    assert!(main.is_head);
    main.tracking = Some(Tracking { remote: "origin".into(), merge: "feature".into() });
    assert_eq!(tracking_local(snap, "origin", "feature").map(|b| b.name.as_str()), Some("main"), "the checked-out one first");
    cleanup(&[&work, &bare]);
}

#[test]
fn checkout_remote_refusals() {
    let (mut st, work, bare) = with_origin("rb-checkout-refused");
    git(&work, &["branch", "x"]);
    git(&work, &["push", "-q", "origin", "main:x"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(cmds(&mut st, checkout("origin", "x")).is_empty());
    assert_eq!(last_log(&st), "A local branch x already exists and doesn't track origin/x.");
    assert!(cmds(&mut st, checkout("origin", "gone")).is_empty());
    assert_eq!(last_log(&st), "origin/gone no longer exists.");
    cleanup(&[&work, &bare]);
}

/// Names that clash with a local branch only by case, Unicode form or folder are refused like
/// in the New branch dialog: loose ref files collide on Windows and macOS.
#[test]
fn checkout_remote_refuses_name_clashes() {
    let (mut st, work, bare) = with_origin("rb-checkout-clash");
    let snap = st.snap.as_mut().unwrap();
    let tip = snap.head_oid.unwrap();
    let feature = snap.locals.iter_mut().find(|b| b.name == "feature").unwrap();
    feature.name = "Feature".into();
    feature.tracking = None;
    feature.upstream = None;
    feature.upstream_ref = None;
    let origin = &mut snap.remotes.iter_mut().find(|(r, _)| r == "origin").unwrap().1;
    origin.push(("main/x".into(), tip));
    origin.push(("con".into(), tip));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(checkout_blocked(snap, "origin", "feature").as_deref(), Some("A branch named Feature already exists."));
    assert_eq!(checkout_blocked(snap, "origin", "main/x").as_deref(), Some("main/x clashes with branch main."), "a folder clash");
    assert_eq!(checkout_blocked(snap, "origin", "con").is_some(), cfg!(windows), "Windows can't store con as a ref file");
    assert!(cmds(&mut st, checkout("origin", "feature")).is_empty(), "Feature shadows a new feature");
    assert_eq!(last_log(&st), "A branch named Feature already exists.");
    cleanup(&[&work, &bare]);
}

#[test]
fn compare_with_head() {
    let (mut st, work, bare) = with_origin("rb-compare");
    let compare = |oid: Oid| Msg::Selection(SelectionMsg::CompareWithHead { oid, name: "origin/feature".into() });
    let before = st.selection.clone();
    let tip = remote_tip(&st, "origin", "feature");
    let fx = update(&mut st, compare(tip), Instant::now());
    assert!(fx.is_empty());
    assert_eq!(last_log(&st), "origin/feature is at the same commit as main.");
    assert_eq!(st.selection, before, "the selection is unchanged");

    remote_commit(&bare, "feature", "f.txt");
    git(&work, &["fetch", "-q", "origin"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    let tip = remote_tip(&st, "origin", "feature");
    let fx = update(&mut st, compare(tip), Instant::now());
    match &st.selection {
        Selection::Commits(m) => assert_eq!(m.compare_ends(), Some((head, tip))),
        other => panic!("not a pair: {other:?}"),
    }
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadCompare { .. })), "{fx:?}");

    assert!(update(&mut st, compare(Oid::zero()), Instant::now()).is_empty());
    assert_eq!(last_log(&st), "origin/feature's commit isn't loaded in the graph.");
    cleanup(&[&work, &bare]);
}

#[test]
fn conflict_check_remote_branch() {
    let (mut st, work, bare) = with_origin("rb-conflict-check");
    remote_commit(&bare, "feature", "f.txt");
    git(&work, &["fetch", "-q", "origin"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let tip = remote_tip(&st, "origin", "feature");
    let fx = update(&mut st, Msg::ConflictCheck(ConflictCheckMsg::CheckBranch("origin/feature".into())), Instant::now());
    assert!(
        matches!(fx.as_slice(), [Effect::CheckConflicts(j)] if j.request.target.name == "origin/feature" && j.request.target.oid == tip),
        "{fx:?}"
    );
    cleanup(&[&work, &bare]);
}

#[test]
fn set_upstream_from_remote_row() {
    let (mut st, work, bare) = with_origin("rb-set-upstream");
    let msg = Msg::Branch(BranchMsg::SetUpstream { branch: "main".into(), remote: "origin".into(), name: "feature".into() });
    assert_eq!(cmds(&mut st, msg), vec![args(&["branch", "--set-upstream-to=origin/feature", "main"])]);
    cleanup(&[&work, &bare]);
}

#[test]
fn split_uses_the_longest_remote() {
    let (mut st, work, bare) = with_origin("rb-split");
    let snap = st.snap.as_mut().unwrap();
    let origin = snap.remotes.iter().find(|(r, _)| r == "origin").cloned().expect("origin");
    snap.remotes = vec![origin, ("or".into(), Vec::new()), ("origin/x".into(), Vec::new())];
    assert_eq!(split(snap, "origin/feature"), Some(("origin", "feature")));
    assert_eq!(split(snap, "origin/x/y"), Some(("origin/x", "y")), "the longer remote wins");
    assert_eq!(split(snap, "orphan/z"), None, "\"or\" isn't followed by '/'");
    assert_eq!(split(snap, "nowhere"), None);
    cleanup(&[&work, &bare]);
}
