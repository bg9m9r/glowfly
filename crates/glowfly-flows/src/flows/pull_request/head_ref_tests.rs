//! End-to-end tests for the refs that keep a pull request's head (`refs/glowfly/pr/<id>`): the
//! fetched head is kept while its panel shows it and survives `git gc`, a head that was already
//! here is pointed at, closing deletes the ref, a parked tab keeps it, a fetch answered after the
//! close is cleaned up, opening a repository sweeps leftovers and keeps a worktree tab's.

use super::source_tests::{head_on_remote, is_files, is_head, refs, step, take};
use super::tests::{cleanup, open, paths, set_pr, setup};
use crate::flows::{drive, finish, git};
use git2::Oid;
use glowfly_core::repo::SourceFrom;
use glowfly_model::msg::{Msg, RepoMsg, TabsMsg};
use glowfly_model::pull_request::PullRequestMsg;
use glowfly_model::state::AppState;
use glowfly_model::testing::state;
use glowfly_model::update::update;
use std::path::{Path, PathBuf};
use std::time::Instant;

const REF: &str = "refs/glowfly/pr/7";

fn at(dir: &Path, name: &str) -> Option<Oid> {
    git2::Repository::open(dir).unwrap().refname_to_id(name).ok()
}

/// A fork's pull request 7 whose head is only on the remote, at `refs/pull/7/head`.
fn fork(name: &str) -> (AppState, PathBuf, PathBuf, Oid) {
    let (mut st, work, bare, _) = setup(name, &[]);
    let head = head_on_remote(&bare);
    set_pr(&mut st, |p| {
        p.source = "from-a-fork".into();
        p.head = Some(head.to_string());
    });
    (st, work, bare, head)
}

fn close(st: &mut AppState) {
    drive(st, Msg::PullRequest(PullRequestMsg::Close));
}

#[test]
fn a_fetched_head_is_kept_while_open_and_survives_gc() {
    let (mut st, work, bare, head) = fork("prr-fetched");
    open(&mut st);
    assert_eq!(at(&work, REF), Some(head));
    git(&work, &["gc", "--prune=now", "-q"]);
    drive(&mut st, Msg::PullRequest(PullRequestMsg::ReloadFiles));
    assert_eq!(paths(&st), ["f.txt", "g.txt"], "the commits are still here");
    close(&mut st);
    assert_eq!(at(&work, REF), None);
    cleanup(&[&work, &bare]);
}

#[test]
fn a_head_already_here_is_kept_then_dropped() {
    let (mut st, work, bare, head) = fork("prr-here");
    // The objects only: no ref points at them.
    git(&work, &["fetch", "-q", "--refmap=", "origin", "refs/pull/7/head"]);
    assert_eq!(at(&work, REF), None);
    open(&mut st);
    let shown = st.session.pull_request.view.as_ref().and_then(|v| v.shown.as_ref()).expect("compared");
    assert_eq!((shown.source.oid, shown.source.from), (head, SourceFrom::Head));
    assert_eq!(at(&work, REF), Some(head));
    close(&mut st);
    assert_eq!(at(&work, REF), None);
    cleanup(&[&work, &bare]);
}

#[test]
fn a_parked_tab_keeps_the_ref_and_closing_it_drops_it() {
    let (mut st, work, bare, head) = fork("prr-parked");
    open(&mut st);
    assert_eq!(at(&work, REF), Some(head));
    drive(&mut st, Msg::Tabs(TabsMsg::NewTab));
    assert_eq!(at(&work, REF), Some(head), "the parked panel still shows it");
    drive(&mut st, Msg::Tabs(TabsMsg::CloseTab(0)));
    assert_eq!(at(&work, REF), None);
    cleanup(&[&work, &bare]);
}

#[test]
fn a_fetch_answered_after_close_drops_its_ref() {
    let (mut st, work, bare, head) = fork("prr-late");
    let fx = update(&mut st, Msg::PullRequest(PullRequestMsg::Open(7)), Instant::now());
    let fx = step(&mut st, take(fx, is_files));
    let fetch = take(fx, is_head);
    close(&mut st);
    // The fetch runs on after the panel closed, and writes the ref.
    let answer = glowfly_runner::testing::execute(fetch).expect("a result");
    assert_eq!(at(&work, REF), Some(head));
    let fx = update(&mut st, answer, Instant::now());
    finish(&mut st, fx);
    assert_eq!(at(&work, REF), None);
    cleanup(&[&work, &bare]);
}

#[test]
fn stale_refs_go_when_the_repository_opens() {
    let (_, work, bare, _) = setup("prr-stale", &[]);
    for name in ["refs/glowfly/pr/3", "refs/glowfly/pr/9", "refs/glowfly/discarded/k"] {
        git(&work, &["update-ref", name, "HEAD"]);
    }
    let mut fresh = state();
    drive(&mut fresh, Msg::Repo(RepoMsg::Open(work.clone())));
    assert_eq!(at(&work, "refs/glowfly/pr/3"), None);
    assert_eq!(at(&work, "refs/glowfly/pr/9"), None);
    assert!(at(&work, "refs/glowfly/discarded/k").is_some());
    assert!(refs(&work).contains("refs/heads/main"));
    cleanup(&[&work, &bare]);
}

#[test]
fn a_worktree_tab_keeps_its_ref_when_another_worktree_opens() {
    let (mut st, work, bare, head) = fork("prr-worktree");
    let tree = work.with_file_name(format!("{}-tree", work.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&tree);
    git(&work, &["worktree", "add", "-q", "-b", "side", &tree.display().to_string()]);
    open(&mut st);
    assert_eq!(at(&work, REF), Some(head));
    drive(&mut st, Msg::Tabs(TabsMsg::NewTab));
    drive(&mut st, Msg::Repo(RepoMsg::Open(tree.clone())));
    assert_eq!(at(&work, REF), Some(head), "the first tab's panel still shows it");
    cleanup(&[&work, &bare, &tree]);
}
