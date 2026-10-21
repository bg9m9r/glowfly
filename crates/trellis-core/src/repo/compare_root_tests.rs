//! Comparisons from or to the empty tree (a line of commits reaching the repository's first
//! commit): the file list both ways, a file's diff, reverting it, and the diff tool's sides.

use super::*;
use crate::testutil::*;
use git2::{Oid, Repository};
use std::path::{Path, PathBuf};

fn commit_all(dir: &Path, message: &str) -> Oid {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

/// c1 (the root: a.txt and b.txt), then c2 editing a.txt.
fn two_commits(name: &str) -> (PathBuf, Oid) {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "a\n");
    write(&dir, "b.txt", "b\n");
    commit_all(&dir, "c1");
    write(&dir, "a.txt", "a\nmore\n");
    let c2 = commit_all(&dir, "c2");
    (dir, c2)
}

fn entries(c: &Comparison) -> Vec<(&str, char)> {
    c.files.iter().map(|f| (f.path.as_str(), f.status)).collect()
}

#[test]
fn compares_from_and_to_the_empty_tree() {
    let (dir, c2) = two_commits("compare-root");
    assert!(is_empty_tree(empty_tree()) && !is_empty_tree(c2));
    let c = compare(&dir, empty_tree(), c2).unwrap();
    assert_eq!(entries(&c), [("a.txt", 'A'), ("b.txt", 'A')]);
    let c = compare(&dir, c2, empty_tree()).unwrap();
    assert_eq!(entries(&c), [("a.txt", 'D'), ("b.txt", 'D')]);

    let diff = load_diff(&dir, &DiffTarget::Between(empty_tree(), c2, "a.txt".into())).unwrap();
    assert_eq!(diff.hunks.len(), 1);
    let lines = &diff.hunks[0].lines;
    assert_eq!(lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), ["a", "more"]);
    assert!(lines.iter().all(|l| l.origin == '+' && l.old.is_none()), "{lines:?}");

    let s = diff_sides(&dir, &DiffTarget::Between(empty_tree(), c2, "a.txt".into())).unwrap();
    assert_eq!(s.old, SideSource::Empty);
    assert_eq!(s.old_label, "empty");
    assert_eq!(s.new_label, c2.to_string()[..7]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reverting_a_file_the_range_added_removes_it() {
    let (dir, c2) = two_commits("compare-root-revert");
    let target = DiffTarget::Between(empty_tree(), c2, "b.txt".into());
    let expected = load_diff(&dir, &target).unwrap().hunks[0].clone();
    let req = RevertRequest { target, part: RevertPart::Hunk { index: 0, expected }, dest: None, ignore_whitespace: false };
    assert_eq!(revert(&dir, &req).map_err(|e| e.to_string()), Ok(Reverted::Removed));
    assert!(!dir.join("b.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}
