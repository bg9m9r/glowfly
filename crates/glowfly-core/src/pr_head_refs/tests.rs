use super::*;
use crate::gitcli;
use crate::repo::{LoadOptions, load, refs_key};
use crate::testkit::git_cli;
use crate::testutil::*;
use std::path::PathBuf;

fn head_of(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().peel_to_commit().unwrap().id()
}

fn at(dir: &Path, name: &str) -> Option<Oid> {
    Repository::open(dir).unwrap().refname_to_id(name).ok()
}

/// A commit no branch reaches (made on a branch that is then deleted).
fn stray_commit(dir: &Path) -> Oid {
    git(dir, &["checkout", "-q", "-b", "stray"]);
    write(dir, "stray.txt", "x\n");
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "stray"]);
    let oid = head_of(dir);
    git(dir, &["checkout", "-q", "main"]);
    git(dir, &["branch", "-D", "stray"]);
    oid
}

/// A bare origin whose `refs/pull/7/head` is a commit on a branch that no longer exists there.
fn origin_with_pull_head(name: &str) -> (PathBuf, PathBuf, Oid) {
    let (work, bare) = with_origin(name);
    git(&bare, &["branch", "tmp", "main"]);
    remote_commit(&bare, "tmp", "pr.txt", "pr head");
    git(&bare, &["update-ref", "refs/pull/7/head", "refs/heads/tmp"]);
    git(&bare, &["branch", "-D", "tmp"]);
    let head = Repository::open(&bare).unwrap().refname_to_id("refs/pull/7/head").unwrap();
    (work, bare, head)
}

fn fetch_head_ref(work: &Path) {
    let cmd = crate::remote_ops::fetch_ref_cmd("origin", "refs/pull/7/head", &pr_head_ref(7), false);
    let (ok, out) = gitcli::run(work, &cmd);
    assert!(ok, "{out}");
}

#[test]
fn fetch_into_the_ref_survives_gc() {
    let (work, _bare, head) = origin_with_pull_head("pr-ref-gc");
    fetch_head_ref(&work);
    assert_eq!(at(&work, "refs/glowfly/pr/7"), Some(head));
    assert_eq!(at(&work, "refs/remotes/origin/tmp"), None, "no remote-tracking ref changes");
    git_cli(&work, &["gc", "--prune=now", "-q"]);
    assert!(Repository::open(&work).unwrap().find_commit(head).is_ok(), "gc kept the head");
}

#[test]
fn fetch_into_the_ref_follows_a_force_push() {
    let (work, bare, first) = origin_with_pull_head("pr-ref-force");
    fetch_head_ref(&work);
    git(&bare, &["branch", "tmp2", "main"]);
    remote_commit(&bare, "tmp2", "other.txt", "forced head");
    git(&bare, &["update-ref", "refs/pull/7/head", "refs/heads/tmp2"]);
    let second = Repository::open(&bare).unwrap().refname_to_id("refs/pull/7/head").unwrap();
    assert_ne!(first, second);
    fetch_head_ref(&work);
    assert_eq!(at(&work, "refs/glowfly/pr/7"), Some(second));
}

#[test]
fn keep_remove_and_sweep() {
    let dir = scratch_repo("pr-ref-keep");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "c1"]);
    set_config(&dir, &[("core.logAllRefUpdates", "always")]);
    let c1 = head_of(&dir);
    let repo = Repository::open(&dir).unwrap();
    let entries = |name: &str| repo.reflog(name).map(|l| l.len()).unwrap_or(0);

    keep_head(&dir, 7, c1).unwrap();
    assert_eq!(at(&dir, "refs/glowfly/pr/7"), Some(c1));
    assert_eq!(entries("refs/glowfly/pr/7"), 1);
    keep_head(&dir, 7, c1).unwrap();
    assert_eq!(entries("refs/glowfly/pr/7"), 1, "an unchanged ref isn't written again");
    let blob = repo.blob(b"not a commit").unwrap();
    assert!(keep_head(&dir, 8, blob).is_err());
    assert_eq!(at(&dir, "refs/glowfly/pr/8"), None);

    remove_heads(&dir, &[7, 99]).unwrap();
    assert_eq!(at(&dir, "refs/glowfly/pr/7"), None);
    assert_eq!(entries("refs/glowfly/pr/7"), 0, "the reflog goes with the ref");

    for n in ["3", "5", "9", "x"] {
        repo.reference(&format!("refs/glowfly/pr/{n}"), c1, true, "test").unwrap();
    }
    repo.reference("refs/glowfly/discarded/k", c1, true, "test").unwrap();
    assert_eq!(sweep_heads(&dir, &[3]).unwrap(), 3);
    assert_eq!(at(&dir, "refs/glowfly/pr/3"), Some(c1));
    for n in ["5", "9", "x"] {
        assert_eq!(at(&dir, &format!("refs/glowfly/pr/{n}")), None, "{n}");
    }
    assert_eq!(at(&dir, "refs/glowfly/discarded/k"), Some(c1));
    assert_eq!(at(&dir, "refs/heads/main"), Some(c1));

    // A ref packed by gc is found and deleted too.
    git_cli(&dir, &["pack-refs", "--all"]);
    assert_eq!(at(&dir, "refs/glowfly/pr/3"), Some(c1));
    assert_eq!(sweep_heads(&dir, &[]).unwrap(), 1);
    assert_eq!(at(&dir, "refs/glowfly/pr/3"), None);
    assert_eq!(at(&dir, "refs/glowfly/discarded/k"), Some(c1));
}

#[test]
fn pr_head_refs_are_invisible_to_loads() {
    let dir = scratch_repo("pr-ref-invisible");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "c1"]);
    let stray = stray_commit(&dir);
    let before = refs_key(&dir).unwrap();
    keep_head(&dir, 7, stray).unwrap();
    assert_eq!(refs_key(&dir).unwrap(), before, "writing one is no change");

    let snap = load(&dir, &LoadOptions::all(100)).unwrap();
    assert!(snap.refs.values().flatten().all(|l| !l.full.starts_with("refs/glowfly/")));
    assert!(!snap.row_of.contains_key(&stray) && snap.commits.iter().all(|c| c.oid != stray));
    assert_eq!(snap.locals.len(), 1);
    assert!(snap.tags.is_empty() && snap.remotes.is_empty());

    remove_heads(&dir, &[7]).unwrap();
    assert_eq!(refs_key(&dir).unwrap(), before, "nor is deleting it");
}
