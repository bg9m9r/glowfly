//! The files a stash changes: its working-tree changes, its staged changes and its untracked
//! files, each path once, sorted.

use super::*;
use crate::testutil::{git, scratch_repo, write};

#[test]
fn lists_unstaged_staged_and_untracked_paths_once() {
    let dir = scratch_repo("stash-paths");
    std::fs::create_dir_all(dir.join("d")).unwrap();
    std::fs::create_dir_all(dir.join("new")).unwrap();
    for f in ["a.txt", "b.txt", "c.txt", "d/e.txt"] {
        write(&dir, f, "one\n");
    }
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    write(&dir, "d/e.txt", "changed\n");
    write(&dir, "b.txt", "staged\n");
    git(&dir, &["add", "b.txt"]);
    write(&dir, "b.txt", "staged then changed\n");
    write(&dir, "new/u.txt", "untracked\n");
    git(&dir, &["stash", "push", "-q", "-u", "-m", "wip"]);
    let oid = list(&dir).unwrap()[0].oid;
    assert_eq!(changed_paths(&dir, oid).unwrap(), ["b.txt", "d/e.txt", "new/u.txt"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_staged_only_change_counts() {
    let dir = scratch_repo("stash-paths-staged");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    write(&dir, "n.txt", "new\n");
    git(&dir, &["add", "n.txt"]);
    git(&dir, &["stash", "push", "-q"]);
    let oid = list(&dir).unwrap()[0].oid;
    assert_eq!(changed_paths(&dir, oid).unwrap(), ["n.txt"]);
    assert!(changed_paths(&dir, git2::Oid::zero()).is_err(), "no such stash");
    let _ = std::fs::remove_dir_all(&dir);
}
