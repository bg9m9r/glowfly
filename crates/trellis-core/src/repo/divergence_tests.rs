//! `Snapshot::divergence` over loaded fixture histories (linear, diverged with a merge, cut
//! short by the load limit), and `Snapshot::upstream_oid` for remote, local and gone upstreams.

use super::{LoadOptions, Snapshot, load};
use crate::testutil::{git, scratch_repo, with_origin};
use git2::{Oid, Repository, Signature, Time};
use std::path::{Path, PathBuf};

/// Commits an empty change on top of `parents` at `secs` (so the load order is fixed), moving
/// `refs/heads/<branch>` to it.
fn commit(repo: &Repository, branch: &str, parents: &[Oid], msg: &str, secs: i64) -> Oid {
    let sig = Signature::new("T", "t@example.com", &Time::new(secs, 0)).unwrap();
    let tree = repo.find_tree(repo.treebuilder(None).unwrap().write().unwrap()).unwrap();
    let parents: Vec<_> = parents.iter().map(|p| repo.find_commit(*p).unwrap()).collect();
    let refs: Vec<_> = parents.iter().collect();
    let oid = repo.commit(None, &sig, &sig, msg, &tree, &refs).unwrap();
    repo.reference(&format!("refs/heads/{branch}"), oid, true, msg).unwrap();
    oid
}

fn fixture(name: &str) -> (PathBuf, Repository) {
    let dir = scratch_repo(name);
    let repo = Repository::open(&dir).unwrap();
    (dir, repo)
}

fn snap(dir: &Path, max: usize) -> Snapshot {
    load(dir, &LoadOptions::all(max)).unwrap()
}

#[test]
fn linear_history_is_ahead_only() {
    let (dir, repo) = fixture("divergence-linear");
    let c0 = commit(&repo, "main", &[], "c0", 1_000);
    let c1 = commit(&repo, "main", &[c0], "c1", 2_000);
    let c2 = commit(&repo, "main", &[c1], "c2", 3_000);
    repo.reference("refs/heads/old", c0, true, "old").unwrap();
    let s = snap(&dir, 100);
    let d = s.divergence(c2, c0, 5).unwrap();
    assert_eq!((d.ahead, d.behind, d.ahead_total, d.behind_total, d.complete), (vec![c2, c1], vec![], 2, 0, true));
    let same = s.divergence(c2, c2, 5).unwrap();
    assert_eq!((same.ahead_total, same.behind_total, same.complete), (0, 0, true));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn diverged_sides_exclude_shared_history_and_count_merge_parents_once() {
    let (dir, repo) = fixture("divergence-merge");
    let root = commit(&repo, "main", &[], "root", 1_000);
    let base = commit(&repo, "main", &[root], "base", 2_000);
    let a1 = commit(&repo, "main", &[base], "a1", 3_000);
    let a2 = commit(&repo, "main", &[a1], "a2", 4_000);
    let t1 = commit(&repo, "up", &[base], "t1", 3_500);
    let x1 = commit(&repo, "side", &[base], "x1", 3_600);
    let m = commit(&repo, "up", &[t1, x1], "merge", 5_000);
    let s = snap(&dir, 100);
    let d = s.divergence(a2, m, 10).unwrap();
    assert_eq!(d.ahead, vec![a2, a1]);
    let mut behind = d.behind.clone();
    behind.sort();
    let mut want = vec![m, t1, x1];
    want.sort();
    assert_eq!(behind, want, "each side of the merge once, base and root excluded");
    assert_eq!(d.behind[0], m, "newest first");
    assert_eq!((d.ahead_total, d.behind_total, d.complete), (2, 3, true));
    let flipped = s.divergence(m, a2, 10).unwrap();
    assert_eq!((flipped.ahead_total, flipped.behind_total), (3, 2));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limits_cut_the_lists_not_the_totals() {
    let (dir, repo) = fixture("divergence-limit");
    let base = commit(&repo, "main", &[], "base", 1_000);
    let a1 = commit(&repo, "main", &[base], "a1", 2_000);
    let a2 = commit(&repo, "main", &[a1], "a2", 3_000);
    let t1 = commit(&repo, "up", &[base], "t1", 2_500);
    let s = snap(&dir, 100);
    let d = s.divergence(a2, t1, 1).unwrap();
    assert_eq!((d.ahead, d.behind, d.ahead_total, d.behind_total), (vec![a2], vec![t1], 2, 1));
    assert!(s.divergence(a2, Oid::from_bytes(&[7; 20]).unwrap(), 1).is_none(), "an oid outside the graph");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_load_cut_inside_the_range_is_incomplete() {
    let (dir, repo) = fixture("divergence-cut");
    let base = commit(&repo, "main", &[], "base", 1_000);
    let mut tip = base;
    for n in 0..5 {
        tip = commit(&repo, "main", &[tip], &format!("a{n}"), 2_000 + n);
    }
    let t1 = commit(&repo, "up", &[base], "t1", 3_000);
    // The newest four: t1 and a4, a3, a2. a2's parent a1 isn't loaded.
    let s = snap(&dir, 4);
    assert!(s.truncated);
    let d = s.divergence(tip, t1, 5).unwrap();
    assert!(!d.complete);
    assert_eq!((d.ahead_total, d.behind_total), (3, 1), "only what the graph holds");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn upstream_oid_reads_remote_and_local_upstreams() {
    let (work, bare) = with_origin("divergence-upstream");
    let s = load(&work, &LoadOptions::all(10)).unwrap();
    let head = s.head_info().unwrap();
    assert_eq!(s.upstream_oid(head), s.head_oid, "origin/main, just pushed");

    git(&work, &["branch", "local-up"]);
    git(&work, &["branch", "topic"]);
    git(&work, &["branch", "--set-upstream-to=local-up", "topic"]);
    let s = load(&work, &LoadOptions::all(10)).unwrap();
    let topic = s.locals.iter().find(|b| b.name == "topic").unwrap();
    assert_eq!(s.upstream_oid(topic), s.local_oid("local-up"));

    git(&work, &["update-ref", "-d", "refs/remotes/origin/main"]);
    let s = load(&work, &LoadOptions::all(10)).unwrap();
    let head = s.head_info().unwrap();
    assert!(head.tracking.is_some(), "the setting stays");
    assert_eq!(s.upstream_oid(head), None, "its ref is gone");
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}
