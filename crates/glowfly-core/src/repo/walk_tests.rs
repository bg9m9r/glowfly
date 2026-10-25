use super::walk::children_first;
use super::*;
use crate::testutil::*;
use git2::Oid;
use std::path::Path;

fn info(n: u8, parents: &[u8]) -> CommitInfo {
    let oid = |n: u8| Oid::from_bytes(&[n; 20]).unwrap();
    CommitInfo {
        oid: oid(n),
        summary: Default::default(),
        body: Default::default(),
        author: crate::repo::Author::new("", ""),
        time: 0,
        parents: parents.iter().map(|&p| oid(p)).collect(),
    }
}

fn ids(list: &[CommitInfo]) -> Vec<u8> {
    list.iter().map(|c| c.oid.as_bytes()[0]).collect()
}

#[test]
fn children_first_keeps_an_ordered_list() {
    let list = vec![info(1, &[2]), info(3, &[2]), info(2, &[4]), info(5, &[9])];
    assert_eq!(ids(&children_first(list)), [1, 3, 2, 5]);
}

#[test]
fn children_first_moves_a_parent_below_its_children() {
    // 2 came out before its child 3 (3's clock was behind), and 4 before its child 2.
    let list = vec![info(1, &[2]), info(4, &[]), info(2, &[4]), info(5, &[]), info(3, &[2])];
    assert_eq!(ids(&children_first(list)), [1, 5, 3, 2, 4]);
}

/// A commit on `main` with committer time `time`.
fn commit_at(dir: &Path, msg: &str, time: i64) {
    write(dir, "f.txt", msg);
    git(dir, &["add", "."]);
    let date = format!("{} +0000", 1_600_000_000 + time);
    let env = [("GIT_COMMITTER_DATE", date.as_str()), ("GIT_AUTHOR_DATE", date.as_str())];
    let (ok, out) = crate::gitcli::run_with_input(dir, &crate::gitcli::args(&["commit", "-q", "-m", msg]), "", &env);
    assert!(ok, "{out}");
}

#[test]
fn load_stops_at_the_limit_and_keeps_skewed_children_first() {
    let dir = scratch_repo("walk-skew");
    commit_at(&dir, "base", 500);
    commit_at(&dir, "parent", 10_000);
    git(&dir, &["checkout", "-q", "-b", "side"]);
    // The clock went back: "child" is far older than its parent.
    commit_at(&dir, "child", 100);
    git(&dir, &["checkout", "-q", "main"]);
    commit_at(&dir, "tip", 5_000);

    let s = load(&dir, &LoadOptions::all(100)).unwrap();
    let order: Vec<&str> = s.commits.iter().map(|c| &*c.summary).collect();
    // By time alone "parent" would come right after "tip", above its child.
    assert_eq!(order, ["tip", "child", "parent", "base"]);
    assert!(!s.truncated);

    let s = load(&dir, &LoadOptions::all(2)).unwrap();
    let order: Vec<&str> = s.commits.iter().map(|c| &*c.summary).collect();
    assert_eq!(order, ["tip", "parent"], "the newest two by time; the old child is past the limit");
    assert!(s.truncated);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn containment_is_remembered_between_loads() {
    let (work, bare) = with_origin("walk-cache");
    git(&work, &["branch", "old"]);
    git(&work, &["push", "-q", "-u", "origin", "old"]);
    write(&work, "b.txt", "two\n");
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "second"]);
    let cache = GraphCache::default();
    let s = load_cached(&work, &LoadOptions::all(10), &cache).unwrap();
    let old = s.locals.iter().find(|b| b.name == "old").unwrap();
    assert!(old.merged);
    assert!(cache.has(s.head_oid.unwrap(), old.oid.unwrap()), "asked once per pair of commits");
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn commit_stats_stop_between_commits() {
    let dir = scratch_repo("stats-until");
    for (i, msg) in ["one", "two", "three"].iter().enumerate() {
        commit_at(&dir, msg, i as i64 * 10);
    }
    let s = load(&dir, &LoadOptions::all(10)).unwrap();
    let oids: Vec<_> = s.commits.iter().map(|c| c.oid).collect();
    let asked = std::cell::Cell::new(0);
    let stop = || {
        asked.set(asked.get() + 1);
        asked.get() >= 2
    };
    let (done, gone_through) = commit_stats_until(&dir, &oids, &stop);
    assert_eq!((done.len(), gone_through), (2, 2), "stops before the third");
    assert_eq!(commit_stats(&dir, &oids).len(), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_walk_keeps_the_commit_it_stopped_at() {
    use super::load_more_tests::{point, raw_commit};
    use super::walk::newest_commits;
    use super::walk_order::TimeQueue;
    let dir = scratch_repo("walk-frontier");
    let repo = git2::Repository::open(&dir).unwrap();
    // Two tips over one root, every commit at the same time: only the push order breaks ties.
    let t = 1_600_000_000;
    let root = raw_commit(&repo, &[], t, "root");
    let mut tips = Vec::new();
    for side in ["one", "two"] {
        let mut at = root;
        for i in 0..3 {
            at = raw_commit(&repo, &[at], t, &format!("{side} {i}"));
        }
        point(&repo, &format!("refs/heads/{side}"), at);
        tips.push(at);
    }
    let total = 7;
    for k in 0..total {
        let w = newest_commits(&repo, &tips, k, None).unwrap();
        let next = newest_commits(&repo, &tips, k + 1, None).unwrap();
        let loaded: Vec<Oid> = w.commits.iter().map(|c| c.oid).collect();
        let want: Vec<Oid> = next.commits.iter().map(|c| c.oid).filter(|o| !loaded.contains(o)).collect();
        let frontier = w.frontier.expect("cut short");
        let mut q = TimeQueue::resume(&frontier, loaded.iter().copied());
        assert_eq!(q.pop(), Some(want[0]), "commit {k} of the walk");
        assert_eq!(next.truncated, k + 1 < total);
        assert_eq!(next.frontier.as_ref().map(|f| f.len()).unwrap_or(0) > 0, k + 1 < total);
    }
    let all = newest_commits(&repo, &tips, total, None).unwrap();
    assert!(!all.truncated && all.frontier.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
