//! `load_refs`, through a whole load: remote branches group under the longest remote name they
//! start with, the tags split into those on commits (lightweight ones named) and the others, a
//! detached HEAD's label comes first and a hidden ref has none, and a tracking branch counts
//! its commits ahead and behind.

use crate::repo::*;
use crate::testutil::*;
use git2::{Oid, Repository};
use std::path::Path;

fn commit(dir: &Path, msg: &str) -> Oid {
    git(dir, &["commit", "-q", "--allow-empty", "-m", msg]);
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn snap(dir: &Path) -> Snapshot {
    load(dir, &LoadOptions::all(50)).unwrap()
}

#[test]
fn remote_branches_group_under_the_longest_remote_name() {
    let dir = scratch_repo("load-refs-nested-remotes");
    let c = commit(&dir, "c");
    git(&dir, &["remote", "add", "a", "https://example.invalid/a.git"]);
    git(&dir, &["remote", "add", "a/b", "https://example.invalid/ab.git"]);
    git(&dir, &["update-ref", "refs/remotes/a/b/x", "HEAD"]);
    git(&dir, &["update-ref", "refs/remotes/a/y", "HEAD"]);
    let mut remotes = snap(&dir).remotes;
    remotes.sort();
    assert_eq!(remotes, vec![("a".to_owned(), vec![("y".to_owned(), c)]), ("a/b".to_owned(), vec![("x".to_owned(), c)])]);
}

#[test]
fn tags_on_commits_are_listed_and_the_lightweight_ones_named() {
    let dir = scratch_repo("load-refs-tags");
    let c = commit(&dir, "c");
    git(&dir, &["tag", "light"]);
    git(&dir, &["tag", "-a", "ann", "-m", "a note"]);
    // A tag on the commit's tree names no commit.
    let repo = Repository::open(&dir).unwrap();
    let tree = repo.find_commit(c).unwrap().tree_id();
    repo.reference("refs/tags/on-tree", tree, false, "a tag on a tree").unwrap();
    let s = snap(&dir);
    let mut tags = s.tags.clone();
    tags.sort();
    assert_eq!(tags, vec![("ann".to_owned(), c), ("light".to_owned(), c)]);
    assert_eq!(s.lightweight_tags.into_iter().collect::<Vec<_>>(), vec!["light".to_owned()]);
    assert_eq!(s.other_tags, vec!["on-tree".to_owned()]);
}

#[test]
fn a_detached_head_is_labelled_first_and_a_hidden_ref_not_at_all() {
    let dir = scratch_repo("load-refs-detached");
    let c = commit(&dir, "c");
    git(&dir, &["branch", "hid"]);
    git(&dir, &["branch", "shown"]);
    git(&dir, &["checkout", "-q", "--detach"]);
    let opts = LoadOptions { hidden: vec!["refs/heads/hid".to_owned()], ..LoadOptions::all(50) };
    let s = load(&dir, &opts).unwrap();
    assert_eq!(s.head_branch, None);
    let labels = &s.refs[&c];
    assert!(labels[0].kind == RefKind::Head && labels[0].is_head);
    let names: Vec<&str> = labels.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["HEAD", "main", "shown"]);
}

#[test]
fn a_tracking_branch_counts_its_commits_ahead_and_behind() {
    let dir = scratch_repo("load-refs-tracking");
    commit(&dir, "base");
    git(&dir, &["checkout", "-q", "-b", "other"]);
    commit(&dir, "theirs");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "mine 1");
    commit(&dir, "mine 2");
    git(&dir, &["remote", "add", "origin", "https://example.invalid/r.git"]);
    git(&dir, &["update-ref", "refs/remotes/origin/main", "refs/heads/other"]);
    git(&dir, &["config", "branch.main.remote", "origin"]);
    git(&dir, &["config", "branch.main.merge", "refs/heads/main"]);
    let s = snap(&dir);
    let main = s.locals.iter().find(|b| b.name == "main").unwrap();
    assert!(main.is_head);
    assert_eq!((main.ahead, main.behind), (2, 1));
    assert_eq!(main.upstream.as_deref(), Some("origin/main"));
    assert_eq!(main.upstream_ref.as_deref(), Some("refs/remotes/origin/main"));
}
