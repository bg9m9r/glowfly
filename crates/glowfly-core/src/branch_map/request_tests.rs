//! The plan made from a snapshot: the empty cases, the main branch (local, else remote-tracking)
//! and the checked-out branch.

use super::tests::commit;
use super::*;
use crate::repo::{LoadOptions, load};
use crate::testutil::{git, scratch_repo, with_origin};
use std::path::Path;

fn plan(dir: &Path) -> Result<Request, Empty> {
    request(&load(dir, &LoadOptions::all(100)).unwrap())
}

fn local(name: &str) -> BranchRef {
    BranchRef::Local(name.into())
}

#[test]
fn an_empty_repository_has_nothing_to_measure() {
    let dir = scratch_repo("bm-req-empty");
    assert_eq!(plan(&dir), Err(Empty::NoCommits));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_the_main_branch_is_one_branch() {
    let dir = scratch_repo("bm-req-one");
    commit(&dir, "a.txt", "a\n", "base");
    assert_eq!(plan(&dir), Err(Empty::OneBranch));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn without_a_main_or_master_there_is_no_main_branch() {
    let dir = scratch_repo("bm-req-nomain");
    commit(&dir, "a.txt", "a\n", "base");
    git(&dir, &["branch", "-m", "main", "trunk"]);
    git(&dir, &["branch", "dev"]);
    assert_eq!(plan(&dir), Err(Empty::NoMain));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn main_and_a_feature() {
    let dir = scratch_repo("bm-req-feature");
    let base = commit(&dir, "a.txt", "a\n", "base");
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    let tip = commit(&dir, "b.txt", "b\n", "feature");
    let r = plan(&dir).unwrap();
    assert_eq!(r.main, Tip { branch: local("main"), oid: base });
    assert_eq!(r.branches, vec![Tip { branch: local("feature"), oid: tip }]);
    assert_eq!(r.current, Some(local("feature")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unborn_branch_is_left_out() {
    let dir = scratch_repo("bm-req-orphan");
    commit(&dir, "a.txt", "a\n", "base");
    git(&dir, &["branch", "feature"]);
    git(&dir, &["checkout", "-q", "--orphan", "new"]);
    let r = plan(&dir).unwrap();
    assert_eq!(r.branches.iter().map(|t| t.branch.clone()).collect::<Vec<_>>(), vec![local("feature")]);
    assert_eq!(r.current, None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_remote_main_stands_in_for_a_missing_local_one() {
    let (dir, bare) = with_origin("bm-req-remote");
    git(&dir, &["checkout", "-q", "-b", "work"]);
    let tip = commit(&dir, "b.txt", "b\n", "work");
    git(&dir, &["branch", "-D", "main"]);
    let r = plan(&dir).unwrap();
    assert_eq!(r.main.branch, BranchRef::Remote("origin/main".into()));
    assert_eq!(r.branches, vec![Tip { branch: local("work"), oid: tip }]);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn a_detached_head_marks_no_branch() {
    let dir = scratch_repo("bm-req-detached");
    let base = commit(&dir, "a.txt", "a\n", "base");
    git(&dir, &["branch", "feature"]);
    git(&dir, &["checkout", "-q", "--detach", &base.to_string()]);
    assert_eq!(plan(&dir).unwrap().current, None);
    let _ = std::fs::remove_dir_all(&dir);
}
