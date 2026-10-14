//! Comparing two commits: the changed files and a file's cumulative diff.

use super::*;
use crate::testutil::*;
use git2::{Oid, Repository};
use std::path::Path;

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().peel_to_commit().unwrap().id()
}

fn commit_all(dir: &Path, message: &str) -> Oid {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    head(dir)
}

fn lines(n: usize) -> String {
    (1..=n).map(|i| format!("line {i}\n")).collect()
}

fn entries(files: &[FileEntry]) -> Vec<(&str, char)> {
    files.iter().map(|f| (f.path.as_str(), f.status)).collect()
}

/// The removed and added lines of `diff`, as `-text` / `+text`.
fn changed_lines(diff: &FileDiff) -> Vec<String> {
    let lines = diff.hunks.iter().flat_map(|h| &h.lines);
    lines.filter(|l| matches!(l.origin, '-' | '+')).map(|l| format!("{}{}", l.origin, l.text)).collect()
}

#[test]
fn compare_files_lists_added_modified_deleted() {
    let dir = scratch_repo("compare-amd");
    write(&dir, "a.txt", "a\n");
    write(&dir, "b.txt", "b\n");
    let c1 = commit_all(&dir, "c1");
    write(&dir, "a.txt", "a\na\n");
    commit_all(&dir, "c2");
    write(&dir, "c.txt", "c\n");
    std::fs::remove_file(dir.join("b.txt")).unwrap();
    let c3 = commit_all(&dir, "c3");

    assert_eq!(entries(&compare_files(&dir, c1, c3).unwrap()), [("a.txt", 'M'), ("b.txt", 'D'), ("c.txt", 'A')]);
    assert_eq!(entries(&compare_files(&dir, c3, c1).unwrap()), [("a.txt", 'M'), ("b.txt", 'A'), ("c.txt", 'D')]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn compare_files_detects_renames() {
    let dir = scratch_repo("compare-rename");
    write(&dir, "old.txt", &lines(10));
    let c1 = commit_all(&dir, "c1");
    write(&dir, "other.txt", "x\n");
    commit_all(&dir, "c2");
    git(&dir, &["mv", "old.txt", "new.txt"]);
    write(&dir, "new.txt", &lines(10).replace("line 4\n", "line four\n"));
    let c3 = commit_all(&dir, "c3");

    assert_eq!(entries(&compare_files(&dir, c1, c3).unwrap()), [("new.txt", 'R'), ("other.txt", 'A')]);
    let d = load_diff(&dir, &DiffTarget::Between(c1, c3, "new.txt".into())).unwrap();
    assert_eq!(changed_lines(&d), ["-line 4", "+line four"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn between_diff_combines_intermediate_commits() {
    let dir = scratch_repo("compare-cumulative");
    write(&dir, "f.txt", &lines(12));
    let c1 = commit_all(&dir, "c1");
    let edit1 = lines(12).replace("line 2\n", "line two\n");
    write(&dir, "f.txt", &edit1);
    commit_all(&dir, "c2");
    let edit2 = edit1.replace("line 11\n", "line eleven\n");
    write(&dir, "f.txt", &edit2);
    let c3 = commit_all(&dir, "c3");

    let target = DiffTarget::Between(c1, c3, "f.txt".into());
    let d = load_diff(&dir, &target).unwrap();
    assert_eq!(changed_lines(&d), ["-line 2", "+line two", "-line 11", "+line eleven"]);

    // A whitespace-only change in between is hidden when whitespace is ignored.
    write(&dir, "f.txt", &edit2.replace("line 6\n", "line   6  \n"));
    let c4 = commit_all(&dir, "c4");
    let ws = DiffTarget::Between(c1, c4, "f.txt".into());
    assert!(changed_lines(&load_diff(&dir, &ws).unwrap()).contains(&"+line   6  ".to_owned()));
    let filtered = load_diff_with(&dir, &ws, LoadDiffOptions { ignore_whitespace: true }).unwrap();
    assert_eq!(changed_lines(&filtered), ["-line 2", "+line two", "-line 11", "+line eleven"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn compare_works_across_branches() {
    let dir = scratch_repo("compare-branches");
    write(&dir, "base.txt", "base\n");
    commit_all(&dir, "base");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    write(&dir, "side.txt", "side\n");
    let side = commit_all(&dir, "side");
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, "main.txt", "main\n");
    let main = commit_all(&dir, "main");

    assert_eq!(entries(&compare_files(&dir, side, main).unwrap()), [("main.txt", 'A'), ("side.txt", 'D')]);
    let d = load_diff(&dir, &DiffTarget::Between(side, main, "main.txt".into())).unwrap();
    assert_eq!(changed_lines(&d), ["+main"]);

    // Same tree, different commits: nothing differs.
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "empty"]);
    let empty = head(&dir);
    assert!(compare_files(&dir, main, empty).unwrap().is_empty());
    let err = load_diff(&dir, &DiffTarget::Between(main, empty, "base.txt".into())).unwrap_err();
    assert!(err.is_not_found(), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn difftarget_helpers() {
    let o = Oid::zero();
    assert_eq!(DiffTarget::Between(o, o, "p.txt".into()).path(), "p.txt");
    assert!(DiffTarget::Unstaged("p".into()).is_working_tree());
    assert!(DiffTarget::Staged("p".into()).is_working_tree());
    assert!(!DiffTarget::Commit(o, "p".into()).is_working_tree());
    assert!(!DiffTarget::Between(o, o, "p".into()).is_working_tree());
}
