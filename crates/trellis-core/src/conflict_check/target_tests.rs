//! Choosing what to compare the checked-out branch with, on loaded fixture repositories.

use super::*;
use crate::repo::{LoadOptions, load};
use crate::testutil::{git, remote_commit, scratch_repo, with_origin, write};
use std::path::Path;

fn target(dir: &Path) -> Option<(String, TargetKind)> {
    let snap = load(dir, &LoadOptions::all(50)).unwrap();
    let t = target_for(&snap)?;
    let expected = snap
        .locals
        .iter()
        .find(|b| b.name == t.name)
        .and_then(|b| b.oid)
        .or_else(|| snap.remotes.iter().find_map(|(r, bs)| bs.iter().find(|(b, _)| format!("{r}/{b}") == t.name).map(|(_, o)| *o)));
    assert_eq!(Some(t.oid), expected, "the oid is the target's tip");
    Some((t.name, t.kind))
}

fn named(name: &str, kind: TargetKind) -> Option<(String, TargetKind)> {
    Some((name.to_owned(), kind))
}

/// `with_origin`, then a `feature` branch with one commit, pushed with upstream.
fn feature_pushed(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let (work, bare) = with_origin(name);
    git(&work, &["checkout", "-q", "-b", "feature"]);
    write(&work, "f.txt", "feature\n");
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "feature"]);
    git(&work, &["push", "-q", "-u", "origin", "feature"]);
    (work, bare)
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn upstream_with_new_commits() {
    let (work, bare) = feature_pushed("ct-upstream");
    remote_commit(&bare, "feature", "g.txt", "from elsewhere");
    git(&work, &["fetch", "-q"]);
    assert_eq!(target(&work), named("origin/feature", TargetKind::Upstream));
    cleanup(&[&work, &bare]);
}

#[test]
fn up_to_date_upstream_falls_through_to_the_remote_head() {
    let (work, bare) = feature_pushed("ct-remote-head");
    git(&work, &["push", "-q", "origin", "main:trunk"]);
    git(&work, &["remote", "set-head", "origin", "trunk"]);
    assert_eq!(target(&work), named("origin/trunk", TargetKind::DefaultBranch));
    git(&work, &["remote", "set-head", "origin", "main"]);
    assert_eq!(target(&work), named("origin/main", TargetKind::DefaultBranch));
    cleanup(&[&work, &bare]);
}

#[test]
fn without_a_remote_head_main_then_master() {
    let (work, bare) = feature_pushed("ct-main-master");
    assert_eq!(target(&work), named("origin/main", TargetKind::DefaultBranch));
    git(&work, &["push", "-q", "origin", "main:master"]);
    git(&work, &["update-ref", "-d", "refs/remotes/origin/main"]);
    assert_eq!(target(&work), named("origin/master", TargetKind::DefaultBranch));
    cleanup(&[&work, &bare]);
}

#[test]
fn local_default_branch_without_remotes() {
    let dir = scratch_repo("ct-local");
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "a"]);
    assert_eq!(target(&dir), None, "on main with nothing else to compare with");
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    assert_eq!(target(&dir), named("main", TargetKind::DefaultBranch));
    git(&dir, &["checkout", "-q", "--detach"]);
    assert_eq!(target(&dir), None, "detached");
    git(&dir, &["checkout", "-q", "--orphan", "fresh"]);
    assert_eq!(target(&dir), None, "unborn");
    cleanup(&[&dir]);
}

#[test]
fn main_tracking_an_up_to_date_upstream_has_no_target() {
    let (work, bare) = with_origin("ct-main-only");
    assert_eq!(target(&work), None);
    git(&work, &["remote", "set-head", "origin", "main"]);
    assert_eq!(target(&work), None, "the remote's HEAD is the upstream itself");
    remote_commit(&bare, "main", "b.txt", "new on main");
    git(&work, &["fetch", "-q"]);
    assert_eq!(target(&work), named("origin/main", TargetKind::Upstream), "once it has new commits");
    cleanup(&[&work, &bare]);
}

#[test]
fn unborn_repository_has_no_target() {
    let dir = scratch_repo("ct-unborn");
    assert_eq!(target(&dir), None);
    cleanup(&[&dir]);
}
