//! `head_ahead`: the first-parent commits ahead of the upstream stop where the upstream's
//! history starts, also past a local merge of a branch forked from the upstream.

use crate::repo::*;
use crate::testutil::*;
use git2::{Oid, Repository};
use std::path::{Path, PathBuf};

fn commit(dir: &Path, msg: &str) -> Oid {
    git(dir, &["commit", "-q", "--allow-empty", "-m", msg]);
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

/// A repository whose `main` tracks `origin/main`, both at "u0", "u1"; returns (dir, u1).
fn tracked(name: &str) -> (PathBuf, Oid) {
    let dir = scratch_repo(name);
    commit(&dir, "u0");
    let u = commit(&dir, "u1");
    git(&dir, &["remote", "add", "origin", "https://example.invalid/r.git"]);
    git(&dir, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    git(&dir, &["config", "branch.main.remote", "origin"]);
    git(&dir, &["config", "branch.main.merge", "refs/heads/main"]);
    (dir, u)
}

fn head_ahead(dir: &Path) -> Option<Vec<Oid>> {
    load(dir, &LoadOptions::all(50)).unwrap().head_ahead
}

#[test]
fn a_straight_branch_lists_its_commits_ahead_newest_first() {
    let (dir, _) = tracked("head-ahead-straight");
    let a = commit(&dir, "a");
    let b = commit(&dir, "b");
    assert_eq!(head_ahead(&dir), Some(vec![b, a]));
}

#[test]
fn a_branch_level_with_its_upstream_has_none_ahead() {
    let (dir, _) = tracked("head-ahead-level");
    assert_eq!(head_ahead(&dir), Some(vec![]));
}

#[test]
fn a_branch_without_an_upstream_has_no_answer() {
    let dir = scratch_repo("head-ahead-no-upstream");
    commit(&dir, "a");
    assert_eq!(head_ahead(&dir), None);
}

#[test]
fn a_local_merge_of_a_side_branch_stops_at_the_upstream() {
    // upstream U; local A, then M merging F1..F3 forked from U: `ahead` is 5, but only M and A
    // are first parents ahead, and U and its history are pushed.
    let (dir, u) = tracked("head-ahead-merge");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    for f in ["f1", "f2", "f3"] {
        commit(&dir, f);
    }
    git(&dir, &["checkout", "-q", "main"]);
    let a = commit(&dir, "a");
    git(&dir, &["merge", "-q", "--no-ff", "-m", "merge side", "side"]);
    let m = Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
    let snap = load(&dir, &LoadOptions::all(50)).unwrap();
    assert_eq!(snap.locals.iter().find(|b| b.is_head).unwrap().ahead, 5);
    assert_eq!(snap.head_ahead, Some(vec![m, a]));
    assert!(!snap.head_ahead.unwrap().contains(&u));
}

#[test]
fn merging_the_moved_upstream_in_stops_before_its_commits() {
    // Local L on U0; the upstream moves to U1 and is merged in: M's first parents are M, L, U0,
    // and U0 is pushed (the merge base is U1, which the first-parent walk never meets).
    let (dir, _) = tracked("head-ahead-merged-upstream");
    git(&dir, &["checkout", "-q", "-b", "up", "origin/main"]);
    let u1 = commit(&dir, "upstream moved");
    git(&dir, &["update-ref", "refs/remotes/origin/main", &u1.to_string()]);
    git(&dir, &["checkout", "-q", "main"]);
    let l = commit(&dir, "local");
    git(&dir, &["merge", "-q", "--no-ff", "-m", "merge upstream", "origin/main"]);
    let m = Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
    assert_eq!(head_ahead(&dir), Some(vec![m, l]));
}
