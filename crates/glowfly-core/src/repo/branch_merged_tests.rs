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

/// A `main` of `length` commits with `each` merged branches (at commits of main) and `each`
/// unmerged ones (one commit off main, forking all along it), all tracking `origin`. Returns
/// main's tip. Every commit has the empty tree. The history is written by one `git fast-import`
/// into a pack, and the branches' config in one write: thousands of loose objects and a config
/// rewrite per branch took most of the long test's time.
fn many_branches(dir: &Path, length: usize, each: usize) -> Oid {
    use std::fmt::Write as _;
    let repo = Repository::open(dir).unwrap();
    repo.remote("origin", "https://example.com/never-fetched.git").unwrap();
    let mut stream = String::new();
    let commit = |stream: &mut String, branch: &str, mark: usize, from: Option<usize>, time: i64, msg: &str| {
        let from = from.map(|m| format!("from :{m}\n")).unwrap_or_default();
        let who = format!("committer Test <test@example.com> {time} +0000");
        let _ = write!(stream, "commit {branch}\nmark :{mark}\n{who}\ndata {}\n{msg}\n{from}\n", msg.len());
    };
    let reset = |stream: &mut String, r: &str, mark: usize| {
        let _ = write!(stream, "reset {r}\nfrom :{mark}\n\n");
    };
    // Main's commits are marks 1..=length, chained on the branch; the open ones follow.
    for i in 0..length {
        commit(&mut stream, "refs/heads/main", i + 1, None, 1_600_000_000 + i as i64 * 60, &format!("main {i}"));
    }
    let step = length / each;
    let mut config = String::new();
    for k in 0..each {
        let at = k * step + 1;
        let side = length + 1 + k;
        commit(
            &mut stream,
            &format!("refs/heads/open-{k}"),
            side,
            Some(at),
            1_600_000_000 + (k * step) as i64 * 60 + 30,
            &format!("side {k}"),
        );
        for (name, mark) in [(format!("done-{k}"), at), (format!("open-{k}"), side)] {
            reset(&mut stream, &format!("refs/heads/{name}"), mark);
            reset(&mut stream, &format!("refs/remotes/origin/{name}"), mark);
            let _ = write!(config, "[branch \"{name}\"]\n\tremote = origin\n\tmerge = refs/heads/{name}\n");
        }
    }
    let (ok, out) = crate::gitcli::run_with_input(dir, &crate::gitcli::args(&["fast-import", "--quiet"]), &stream, &[]);
    assert!(ok, "git fast-import: {out}");
    let path = dir.join(".git/config");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str(&config);
    std::fs::write(&path, text).unwrap();
    repo.set_head("refs/heads/main").unwrap();
    repo.find_reference("refs/heads/main").unwrap().target().unwrap()
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
