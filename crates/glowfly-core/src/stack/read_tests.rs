//! Reading stacks from real repositories: what counts as the base, former tips from reflogs.

use super::*;
use crate::repo::{LoadOptions, load};
use crate::testutil::*;
use std::path::Path;

/// Commits a new file named after `msg` on the checked-out branch; returns the new tip.
pub(super) fn commit(dir: &Path, msg: &str) -> Oid {
    write(dir, &format!("{msg}.txt"), &format!("{msg}\n"));
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", msg]);
    tip(dir, "HEAD")
}

pub(super) fn tip(dir: &Path, rev: &str) -> Oid {
    git2::Repository::open(dir).unwrap().revparse_single(rev).unwrap().peel_to_commit().unwrap().id()
}

pub(super) fn stacks_of(dir: &Path) -> Stacks {
    load(dir, &LoadOptions::all(1000)).unwrap().stacks
}

fn link_of(branch: &str, on: &str, base: Oid, behind: bool) -> StackLink {
    StackLink { branch: branch.into(), on: on.into(), base, behind }
}

#[test]
fn a_branch_on_another_branch_is_stacked_on_it() {
    let dir = scratch_repo("stack-read-chain");
    commit(&dir, "base");
    git(&dir, &["checkout", "-q", "-b", "a"]);
    let a = commit(&dir, "a1");
    git(&dir, &["checkout", "-q", "-b", "b"]);
    commit(&dir, "b1");
    git(&dir, &["checkout", "-q", "-b", "c", "main"]);
    commit(&dir, "c1");
    let snap = load(&dir, &LoadOptions::all(1000)).unwrap();
    assert_eq!(snap.stacks.links, vec![link_of("b", "a", a, false)], "Snapshot::load fills the stacks");
    let repo = git2::Repository::open(&dir).unwrap();
    assert_eq!(read(&repo, &snap), snap.stacks);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_merged_branch_and_one_built_on_it_are_not_layers() {
    let dir = scratch_repo("stack-read-merged");
    commit(&dir, "base");
    git(&dir, &["checkout", "-q", "-b", "x"]);
    commit(&dir, "x1");
    git(&dir, &["checkout", "-q", "-b", "y"]);
    commit(&dir, "y1");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "m1");
    // x's tip is reached only through the merge's second parent.
    git(&dir, &["merge", "-q", "--no-ff", "-m", "merge x", "x"]);
    assert!(stacks_of(&dir).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn amending_the_lower_branch_leaves_the_upper_one_behind() {
    let dir = scratch_repo("stack-read-amend");
    commit(&dir, "base");
    git(&dir, &["checkout", "-q", "-b", "a"]);
    let a0 = commit(&dir, "a1");
    git(&dir, &["checkout", "-q", "-b", "b"]);
    commit(&dir, "b1");
    git(&dir, &["checkout", "-q", "a"]);
    git(&dir, &["commit", "-q", "--amend", "-m", "a1, amended"]);
    assert_eq!(stacks_of(&dir).links, vec![link_of("b", "a", a0, true)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_new_commit_on_the_lower_branch_leaves_the_upper_one_behind() {
    let dir = scratch_repo("stack-read-advance");
    commit(&dir, "base");
    git(&dir, &["checkout", "-q", "-b", "a"]);
    let a0 = commit(&dir, "a1");
    git(&dir, &["checkout", "-q", "-b", "b"]);
    commit(&dir, "b1");
    git(&dir, &["checkout", "-q", "a"]);
    commit(&dir, "a2");
    assert_eq!(stacks_of(&dir).links, vec![link_of("b", "a", a0, true)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_branch_created_at_a_tip_does_not_become_its_lower_layer() {
    let dir = scratch_repo("stack-read-created");
    commit(&dir, "base");
    git(&dir, &["checkout", "-q", "-b", "x"]);
    let x0 = commit(&dir, "x1");
    git(&dir, &["branch", "y"]);
    git(&dir, &["checkout", "-q", "y"]);
    commit(&dir, "y1");
    git(&dir, &["checkout", "-q", "x"]);
    commit(&dir, "x2");
    // y's "branch: Created" entry at x0 is ignored, so x isn't on y.
    assert_eq!(stacks_of(&dir).links, vec![link_of("y", "x", x0, true)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_main_branch_means_no_stacks() {
    let dir = scratch_repo("stack-read-nomain");
    commit(&dir, "base");
    git(&dir, &["branch", "-m", "main", "trunk"]);
    git(&dir, &["checkout", "-q", "-b", "a"]);
    commit(&dir, "a1");
    git(&dir, &["checkout", "-q", "-b", "b"]);
    commit(&dir, "b1");
    assert!(stacks_of(&dir).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_branch_already_in_the_remote_main_is_merged() {
    let (dir, bare) = with_origin("stack-read-remote");
    git(&dir, &["checkout", "-q", "-b", "a"]);
    commit(&dir, "a1");
    git(&dir, &["checkout", "-q", "-b", "b"]);
    commit(&dir, "b1");
    assert_eq!(stacks_of(&dir).links.len(), 1, "b is on a before a reaches origin/main");
    git(&dir, &["push", "-q", "origin", "a:main"]);
    git(&dir, &["fetch", "-q", "origin"]);
    assert!(stacks_of(&dir).is_empty(), "a is in origin/main though not in local main");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn git_flow_development_is_a_base() {
    let dir = scratch_repo("stack-read-flow");
    commit(&dir, "base");
    git(&dir, &["checkout", "-q", "-b", "develop"]);
    commit(&dir, "d1");
    git(&dir, &["checkout", "-q", "-b", "feature/x"]);
    commit(&dir, "f1");
    assert_eq!(stacks_of(&dir).links.len(), 1, "without Git Flow, the feature is on develop");
    set_config(
        &dir,
        &[
            ("gitflow.branch.master", "main"),
            ("gitflow.branch.develop", "develop"),
            ("gitflow.prefix.feature", "feature/"),
            ("gitflow.prefix.release", "release/"),
            ("gitflow.prefix.hotfix", "hotfix/"),
        ],
    );
    assert!(stacks_of(&dir).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
