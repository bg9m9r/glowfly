//! File diffs with whitespace ignored, against real repositories.

use super::*;
use crate::testutil::*;
use git2::{Oid, Repository};
use std::path::Path;

const IGNORE: LoadDiffOptions = LoadDiffOptions { ignore_whitespace: true, whole_file: false };

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().peel_to_commit().unwrap().id()
}

/// With whitespace ignored `target` has no hunks; with every change shown it has some.
fn only_whitespace(dir: &Path, target: &DiffTarget) {
    let filtered = load_diff_with(dir, target, IGNORE).unwrap();
    assert!(!filtered.binary, "{target:?}");
    assert!(filtered.hunks.is_empty(), "{target:?}: {:?}", filtered.hunks);
    assert!(!load_diff(dir, target).unwrap().hunks.is_empty(), "{target:?}");
}

/// The removed and added lines of `diff`, as `-text` / `+text`.
fn changed_lines(diff: &FileDiff) -> Vec<String> {
    let lines = diff.hunks.iter().flat_map(|h| &h.lines);
    lines.filter(|l| matches!(l.origin, '-' | '+')).map(|l| format!("{}{}", l.origin, l.text)).collect()
}

#[test]
fn whitespace_only_changes_have_no_hunks_when_ignored() {
    let dir = scratch_repo("ws-ignore");
    write(&dir, "f.txt", "foo\nbar\n    baz\nqux\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "base"]);

    // Trailing spaces, a trailing tab, a re-indent and a CRLF line ending.
    write(&dir, "f.txt", "foo  \nbar\t\n        baz\r\nqux\n");
    only_whitespace(&dir, &DiffTarget::Unstaged("f.txt".into()));

    git(&dir, &["add", "f.txt"]);
    only_whitespace(&dir, &DiffTarget::Staged("f.txt".into()));

    git(&dir, &["commit", "-am", "ws"]);
    only_whitespace(&dir, &DiffTarget::Commit(head(&dir), "f.txt".into()));
}

#[test]
fn ignoring_whitespace_keeps_real_changes() {
    let dir = scratch_repo("ws-real");
    write(&dir, "f.txt", "a\nb\nc\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "base"]);
    write(&dir, "f.txt", "a  \nB\nc\n");

    let d = load_diff_with(&dir, &DiffTarget::Unstaged("f.txt".into()), IGNORE).unwrap();
    assert_eq!(d.hunks.len(), 1);
    assert_eq!(changed_lines(&d), ["-b", "+B"]);
}

#[test]
fn ignoring_whitespace_still_shows_new_indentation() {
    // git -b treats whitespace inserted where there was none as a change.
    let dir = scratch_repo("ws-indent");
    write(&dir, "f.txt", "bar\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "base"]);
    write(&dir, "f.txt", "  bar\n");

    let d = load_diff_with(&dir, &DiffTarget::Unstaged("f.txt".into()), IGNORE).unwrap();
    assert_eq!(d.hunks.len(), 1);
    assert_eq!(changed_lines(&d), ["-bar", "+  bar"]);
}

#[test]
fn apply_hunk_counts_unfiltered_hunks() {
    // Why hunk staging is off while whitespace is hidden: the indexes differ.
    let dir = scratch_repo("ws-apply");
    let base: String = (1..=30).map(|i| format!("line {i}\n")).collect();
    write(&dir, "f.txt", &base);
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "base"]);
    write(&dir, "f.txt", &base.replace("line 2\n", "line 2   \n").replace("line 28\n", "line 28 changed\n"));

    let target = DiffTarget::Unstaged("f.txt".into());
    assert_eq!(load_diff_with(&dir, &target, IGNORE).unwrap().hunks.len(), 1);
    assert_eq!(load_diff(&dir, &target).unwrap().hunks.len(), 2);

    // Index 0 is the whitespace hunk of the unfiltered diff, not the one hunk shown filtered.
    apply_hunk(&dir, "f.txt", 0, HunkOp::Stage).unwrap();
    let staged = changed_lines(&load_diff(&dir, &DiffTarget::Staged("f.txt".into())).unwrap());
    assert_eq!(staged, ["-line 2", "+line 2   "]);
}
