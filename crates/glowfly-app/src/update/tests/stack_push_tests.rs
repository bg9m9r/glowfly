//! Push stack end to end on a repository with a bare origin: the job, upstreams, rewritten
//! layers replaced with a lease, a stale lease and a behind layer left alone, and every
//! refusal.

use super::branch_tests::{remote_commit, with_origin};
use super::tests::{drive, finish, git};
use super::update;
use crate::effect::{Effect, StackEffect};
use crate::msg::{CommitMsg, Msg, RebaseMsg, RepoMsg, StackMsg};
use crate::state::AppState;
use crate::state::branches::NO_REMOTES;
use git2::{Oid, RepositoryState};
use glowfly_core::stack::RestackScope;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub(super) fn commit(dir: &Path, msg: &str) {
    std::fs::write(dir.join(format!("{msg}.txt")), format!("{msg}\n")).unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", msg]);
}

pub(super) fn tip(dir: &Path, rev: &str) -> Oid {
    git2::Repository::open(dir).unwrap().revparse_single(rev).unwrap().peel_to_commit().unwrap().id()
}

/// `with_origin` plus main <- low <- mid <- top, none pushed, low checked out and the work
/// tree clean.
pub(super) fn stacked(name: &str) -> (AppState, PathBuf, PathBuf) {
    let (mut st, dir, bare) = with_origin(name);
    git(&dir, &["checkout", "--", "a.txt"]);
    git(&dir, &["checkout", "-q", "-b", "low"]);
    commit(&dir, "l1");
    git(&dir, &["checkout", "-q", "-b", "mid"]);
    commit(&dir, "m1");
    git(&dir, &["checkout", "-q", "-b", "top"]);
    commit(&dir, "t1");
    git(&dir, &["checkout", "-q", "low"]);
    refresh(&mut st);
    assert!(st.snap.as_ref().unwrap().stacks.below("top").is_some(), "top is stacked on mid");
    (st, dir, bare)
}

pub(super) fn refresh(st: &mut AppState) {
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

fn push_stack(branch: &str) -> Msg {
    Msg::Stack(StackMsg::Push { branch: branch.into() })
}

fn last_toast(st: &AppState) -> (bool, String) {
    st.toasts.last().map(|t| (t.ok, t.text.clone())).expect("a toast")
}

fn logged(st: &AppState, text: &str) -> bool {
    st.log.iter().any(|l| l.text.contains(text))
}

fn config(dir: &Path, key: &str) -> Option<String> {
    git2::Repository::open(dir).unwrap().config().unwrap().snapshot().unwrap().get_string(key).ok()
}

fn bare_tip(bare: &Path, b: &str) -> Oid {
    tip(bare, &format!("refs/heads/{b}"))
}

pub(super) fn cleanup(dir: &Path, bare: &Path) {
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(bare);
}

/// Pushes the stack, then amends low and restacks mid and top onto it, without fetching.
fn rewritten(name: &str) -> (AppState, PathBuf, PathBuf) {
    let (mut st, dir, bare) = stacked(name);
    drive(&mut st, push_stack("mid"));
    st.session.draft.amend = true;
    st.session.draft.summary = "l1, amended".into();
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    drive(&mut st, Msg::Rebase(RebaseMsg::Restack { scope: RestackScope::Above("low".into()), autostash: false }));
    assert_eq!(tip(&dir, "mid^"), tip(&dir, "low"), "mid is on the amended low");
    assert_ne!(bare_tip(&bare, "low"), tip(&dir, "low"));
    st.log.clear();
    (st, dir, bare)
}

#[test]
fn an_unpushed_stack_goes_up_in_one_job_with_upstreams() {
    let (mut st, dir, bare) = stacked("stack-push-job");
    let undo = st.session.undo.undo.len();
    let fx = update(&mut st, push_stack("mid"), Instant::now());
    assert_eq!(fx.iter().filter(|e| matches!(e, Effect::Stack(StackEffect::Push { .. }))).count(), 1, "{fx:?}");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Push stack"));
    finish(&mut st, fx);
    for b in ["low", "mid", "top"] {
        assert_eq!(bare_tip(&bare, b), tip(&dir, b), "{b}");
        assert_eq!(config(&dir, &format!("branch.{b}.remote")).as_deref(), Some("origin"));
        assert!(logged(&st, &format!("{b}: pushed as a new branch, tracking origin/{b}")), "{:?}", st.log);
    }
    assert_eq!(last_toast(&st), (true, "Pushed the stack".into()));
    assert_eq!(st.session.undo.undo.len(), undo, "a push keeps the undo history");
    assert!(st.job.is_none());
    cleanup(&dir, &bare);
}

#[test]
fn a_restacked_stack_replaces_its_pushed_versions_with_a_lease() {
    let (mut st, dir, bare) = rewritten("stack-push-restacked");
    drive(&mut st, push_stack("top"));
    for b in ["low", "mid", "top"] {
        assert_eq!(bare_tip(&bare, b), tip(&dir, b), "{b}");
        assert!(logged(&st, &format!("{b}: replaced its rewritten version on origin/{b} (force with lease)")), "{:?}", st.log);
    }
    assert!(!logged(&st, "pushed to") && !logged(&st, "pushed as a new branch"), "{:?}", st.log);
    assert_eq!(last_toast(&st), (true, "Pushed the stack".into()));
    cleanup(&dir, &bare);
}

#[test]
fn a_remote_moved_since_the_snapshot_is_left_alone() {
    let (mut st, dir, bare) = rewritten("stack-push-stale");
    remote_commit(&bare, "mid", "theirs.txt");
    let foreign = bare_tip(&bare, "mid");
    drive(&mut st, push_stack("low"));
    assert_eq!(last_toast(&st), (false, "Pushed 2 of 3 branches of the stack".into()));
    let line = "mid: not pushed; origin/mid moved since the last fetch. Fetch, check it, and push again.";
    assert!(st.log.iter().any(|l| !l.ok && l.text == line), "{:?}", st.log);
    assert_eq!(bare_tip(&bare, "mid"), foreign, "the remote's commit stays");
    assert_eq!(bare_tip(&bare, "low"), tip(&dir, "low"));
    cleanup(&dir, &bare);
}

#[test]
fn a_behind_layer_is_not_pushed_and_nothing_is_forced() {
    let (mut st, dir, bare) = stacked("stack-push-behind");
    drive(&mut st, push_stack("low"));
    remote_commit(&bare, "top", "theirs.txt");
    git(&dir, &["fetch", "-q", "origin"]);
    git(&dir, &["checkout", "-q", "mid"]);
    commit(&dir, "m2");
    refresh(&mut st);
    let theirs = bare_tip(&bare, "top");
    st.log.clear();
    drive(&mut st, push_stack("mid"));
    assert_eq!(bare_tip(&bare, "mid"), tip(&dir, "mid"));
    assert_eq!(bare_tip(&bare, "top"), theirs);
    assert!(logged(&st, "top: not pushed; origin/top has commits it lacks. Pull first."), "{:?}", st.log);
    assert!(logged(&st, "low: already up to date on origin/low"));
    assert!(!logged(&st, "force-with-lease"), "{:?}", st.log);
    assert_eq!(last_toast(&st), (false, "Pushed 2 of 3 branches of the stack".into()));
    cleanup(&dir, &bare);
}

#[test]
fn refusals_start_nothing() {
    let (mut st, dir, bare) = stacked("stack-push-refusals");
    let no_effect = |st: &mut AppState, msg: Msg, want: &str| {
        let fx = update(st, msg, Instant::now());
        assert!(!fx.iter().any(|e| matches!(e, Effect::Stack(_))), "{fx:?}");
        assert_eq!(st.log.last().map(|l| l.text.clone()).as_deref(), Some(want));
    };
    no_effect(&mut st, push_stack("feature"), "feature isn't in a stack.");

    st.job = Some(super::rebase_tests::running_job(&dir));
    no_effect(&mut st, push_stack("mid"), "Another operation is still running.");
    st.job = None;

    st.snap.as_mut().unwrap().state = RepositoryState::Merge;
    let busy = crate::state::rebase_onto::in_progress(st.settings.lang(), RepositoryState::Merge).unwrap();
    no_effect(&mut st, push_stack("mid"), busy);
    refresh(&mut st);

    // The effect carries the remote tips the snapshot saw.
    drive(&mut st, push_stack("mid"));
    git(&dir, &["checkout", "-q", "top"]);
    commit(&dir, "t2");
    refresh(&mut st);
    let fx = update(&mut st, push_stack("mid"), Instant::now());
    let Some(Effect::Stack(StackEffect::Push { job, .. })) = fx.iter().find(|e| matches!(e, Effect::Stack(_))) else { panic!("{fx:?}") };
    let seen: Vec<(&str, Option<Oid>)> = job.targets.iter().map(|t| (t.branch.as_str(), t.seen)).collect();
    assert_eq!(seen, [("low", Some(tip(&dir, "low"))), ("mid", Some(tip(&dir, "mid"))), ("top", Some(tip(&dir, "top^")))]);
    finish(&mut st, fx);

    no_effect(&mut st, push_stack("mid"), "Every branch in the stack is already pushed.");
    git(&dir, &["remote", "remove", "origin"]);
    refresh(&mut st);
    no_effect(&mut st, push_stack("mid"), NO_REMOTES);
    cleanup(&dir, &bare);
}

#[test]
fn layer_lines_go_to_the_pushed_repos_tab_after_a_tab_switch() {
    let (mut st, dir, bare) = stacked("stack-push-tab-switch");
    st.log.clear();
    let fx = update(&mut st, push_stack("mid"), Instant::now());
    // Another repository opens in a new, active tab while the push runs.
    let other = super::tests::scratch_repo("stack-push-tab-switch-other");
    drive(&mut st, Msg::Repo(RepoMsg::Open(other.clone())));
    assert!(st.tabs.len() == 2 && st.active_tab == 1);
    finish(&mut st, fx);
    let line = "low: pushed as a new branch, tracking origin/low";
    assert!(!logged(&st, line), "the active tab's log: {:?}", st.log);
    assert!(st.tabs[0].log.iter().any(|l| l.text == line), "{:?}", st.tabs[0].log);
    assert_eq!(bare_tip(&bare, "top"), tip(&dir, "top"));
    cleanup(&dir, &bare);
    let _ = std::fs::remove_dir_all(&other);
}
