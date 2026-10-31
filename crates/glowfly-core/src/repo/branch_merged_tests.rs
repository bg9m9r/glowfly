//! `branch_merged`: which local branches HEAD contains, answered from the loaded graph and the
//! previous head's answers, and timed on a long history with many branches.

use super::*;
use crate::testutil::*;
use git2::{Oid, Repository, Signature, Time};
use std::path::Path;
use std::time::Instant;

/// A commit on `parent` (with the empty tree) at `time`, not moving any ref.
fn commit_at(repo: &Repository, parent: Option<Oid>, time: i64, msg: &str) -> Oid {
    let sig = Signature::new("Test", "test@example.com", &Time::new(time, 0)).unwrap();
    let tree = repo.find_tree(repo.treebuilder(None).unwrap().write().unwrap()).unwrap();
    let parents: Vec<_> = parent.map(|p| repo.find_commit(p).unwrap()).into_iter().collect();
    let refs: Vec<_> = parents.iter().collect();
    repo.commit(None, &sig, &sig, msg, &tree, &refs).unwrap()
}

/// Local branch `name` at `tip`, tracking `origin/<name>` at the same commit.
fn tracked(repo: &Repository, name: &str, tip: Oid) {
    repo.reference(&format!("refs/heads/{name}"), tip, true, "test").unwrap();
    repo.reference(&format!("refs/remotes/origin/{name}"), tip, true, "test").unwrap();
    let mut cfg = repo.config().unwrap();
    cfg.set_str(&format!("branch.{name}.remote"), "origin").unwrap();
    cfg.set_str(&format!("branch.{name}.merge"), &format!("refs/heads/{name}")).unwrap();
}

/// A `main` of `length` commits with `each` merged branches (at commits of main) and `each`
/// unmerged ones (one commit off main, forking all along it), all tracking `origin`. Returns
/// main's tip.
fn many_branches(dir: &Path, length: usize, each: usize) -> Oid {
    let repo = Repository::open(dir).unwrap();
    repo.remote("origin", "https://example.com/never-fetched.git").unwrap();
    let mut chain = Vec::with_capacity(length);
    let mut tip = None;
    for i in 0..length {
        let c = commit_at(&repo, tip, 1_600_000_000 + i as i64 * 60, &format!("main {i}"));
        chain.push(c);
        tip = Some(c);
    }
    let step = length / each;
    for k in 0..each {
        let at = chain[k * step];
        tracked(&repo, &format!("done-{k}"), at);
        let side = commit_at(&repo, Some(at), 1_600_000_000 + (k * step) as i64 * 60 + 30, &format!("side {k}"));
        tracked(&repo, &format!("open-{k}"), side);
    }
    let tip = tip.unwrap();
    repo.reference("refs/heads/main", tip, true, "test").unwrap();
    repo.set_head("refs/heads/main").unwrap();
    tip
}

/// Moves `main` (checked out) on by one commit, as a commit in the app would.
fn commit_on_main(dir: &Path, time: i64) -> Oid {
    let repo = Repository::open(dir).unwrap();
    let head = repo.head().unwrap().target().unwrap();
    let c = commit_at(&repo, Some(head), time, "new work");
    repo.reference("refs/heads/main", c, true, "test").unwrap();
    c
}

fn merged(s: &Snapshot, name: &str) -> bool {
    s.locals.iter().find(|b| b.name == name).unwrap().merged
}

#[test]
fn many_branches_are_not_walked_again_after_a_commit() {
    let (length, each) = (6000, 60);
    let dir = scratch_repo("merged-many");
    many_branches(&dir, length, each);
    // The graph shows only the newest commits, so most branch tips are outside it.
    let opts = LoadOptions { max_commits: 500, ..LoadOptions::default() };
    let cache = GraphCache::default();

    let started = Instant::now();
    let first = load_cached(&dir, &opts, &cache).unwrap();
    let cold = started.elapsed();
    assert!(first.truncated);
    assert!((0..each).all(|k| merged(&first, &format!("done-{k}"))), "every done branch is in main");
    assert!((0..each).all(|k| !merged(&first, &format!("open-{k}"))), "no open branch is");

    let started = Instant::now();
    let again = load_cached(&dir, &opts, &cache).unwrap();
    let unchanged = started.elapsed();

    let walks = cache.walks();
    commit_on_main(&dir, 1_600_000_000 + length as i64 * 60);
    let started = Instant::now();
    let moved = load_cached(&dir, &opts, &cache).unwrap();
    let after_commit = started.elapsed();
    assert!((0..each).all(|k| merged(&moved, &format!("done-{k}"))));
    assert!((0..each).all(|k| !merged(&moved, &format!("open-{k}"))));
    assert_eq!(merged(&again, "open-0"), merged(&moved, "open-0"));
    assert_eq!(cache.walks(), walks, "answered from the last HEAD's answers, without a walk per branch");

    eprintln!("{} branches on {length} commits: cold load {cold:?}, unchanged {unchanged:?}, after a commit {after_commit:?}", 2 * each);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_new_head_contains_what_it_merged_and_what_the_old_one_did() {
    let dir = scratch_repo("merged-moved");
    let tip = many_branches(&dir, 40, 4);
    let opts = LoadOptions { max_commits: 5, ..LoadOptions::default() };
    let cache = GraphCache::default();
    let first = load_cached(&dir, &opts, &cache).unwrap();
    assert!(first.truncated && merged(&first, "done-0") && !merged(&first, "open-1"));

    // A merge of open-1 into main: the walk since the old HEAD finds its tip.
    let repo = Repository::open(&dir).unwrap();
    let side = repo.find_reference("refs/heads/open-1").unwrap().target().unwrap();
    let sig = Signature::new("Test", "test@example.com", &Time::new(1_700_000_000, 0)).unwrap();
    let tree = repo.find_commit(tip).unwrap().tree().unwrap();
    let parents = [&repo.find_commit(tip).unwrap(), &repo.find_commit(side).unwrap()];
    let merge = repo.commit(None, &sig, &sig, "merge open-1", &tree, &parents).unwrap();
    repo.reference("refs/heads/main", merge, true, "test").unwrap();
    let walks = cache.walks();
    let moved = load_cached(&dir, &opts, &cache).unwrap();
    assert!(merged(&moved, "open-1") && merged(&moved, "done-0") && !merged(&moved, "open-2"));
    assert_eq!(cache.walks(), walks);

    // Checking out an old branch: no earlier HEAD is in its history, so tips outside the
    // loaded rows are walked.
    repo.set_head("refs/heads/done-3").unwrap();
    let back = load_cached(&dir, &opts, &cache).unwrap();
    assert!(merged(&back, "done-0") && !merged(&back, "open-1"));
    assert!(merged(&back, "done-1") && merged(&back, "done-2") && !merged(&back, "open-3") && !merged(&back, "main"));
    assert!(cache.walks() > walks);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_whole_history_in_the_rows_needs_no_walk() {
    let dir = scratch_repo("merged-whole");
    many_branches(&dir, 30, 3);
    let cache = GraphCache::default();
    let s = load_cached(&dir, &LoadOptions::all(1000), &cache).unwrap();
    assert!(!s.truncated);
    assert!((0..3).all(|k| merged(&s, &format!("done-{k}")) && !merged(&s, &format!("open-{k}"))));
    assert_eq!(cache.walks(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}
