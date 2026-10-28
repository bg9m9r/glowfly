//! `unmerged_count` on a fixture: a branch merged into another local branch or pushed to its
//! upstream loses nothing; commits only it has are counted, and tags don't save them.

use super::unmerged_count;
use crate::testutil::{git, with_origin, write};
use git2::{Oid, Repository};
use std::path::Path;

fn tip(dir: &Path, branch: &str) -> Oid {
    Repository::open(dir).unwrap().refname_to_id(&format!("refs/heads/{branch}")).unwrap()
}

fn commit(dir: &Path, file: &str) {
    write(dir, file, file);
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", file]);
}

#[test]
fn counts_commits_on_no_other_branch() {
    let (dir, bare) = with_origin("unmerged-count");
    git(&dir, &["checkout", "-q", "-b", "topic"]);
    commit(&dir, "t1.txt");
    commit(&dir, "t2.txt");
    commit(&dir, "t3.txt");
    git(&dir, &["checkout", "-q", "main"]);
    let count = |b: &str| unmerged_count(&dir, b, tip(&dir, b)).unwrap();
    assert_eq!(count("topic"), 3, "three commits only topic has");
    assert_eq!(count("main"), 0, "main's commits are on origin/main");

    git(&dir, &["tag", "keep", "topic"]);
    assert_eq!(count("topic"), 3, "a tag isn't a branch");

    // On another local branch (not HEAD): nothing is lost.
    git(&dir, &["branch", "copy", "topic~1"]);
    assert_eq!(count("topic"), 1, "the newest commit only");
    git(&dir, &["branch", "-f", "copy", "topic"]);
    assert_eq!(count("topic"), 0);

    // Pushed to its upstream: nothing is lost either.
    git(&dir, &["branch", "-D", "copy"]);
    git(&dir, &["push", "-q", "-u", "origin", "topic"]);
    assert_eq!(count("topic"), 0, "origin/topic has them");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&bare);
}
