//! Line staging that takes a whole file in or out of the index: unstaging every line of an
//! added file untracks it, and staging every line of a deleted file stages the deletion. Files
//! that really are empty keep their entry.

use super::lines_tests::{apply, index_bytes, index_text, pick, read};
use super::*;
use crate::testutil::*;
use std::path::{Path, PathBuf};

/// (path, status) rows of a status list.
type Rows = Vec<(String, char)>;

/// The rows of the unstaged and the staged list.
fn lists(dir: &Path) -> (Rows, Rows) {
    let (unstaged, staged, _) = status_at(dir, false).unwrap();
    let rows = |l: Vec<FileEntry>| l.into_iter().map(|e| (e.path, e.status)).collect();
    (rows(unstaged), rows(staged))
}

fn status_of(list: &[(String, char)], path: &str) -> Option<char> {
    list.iter().find(|(p, _)| p == path).map(|(_, s)| *s)
}

/// A repository with `base.txt` committed.
fn based(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "base.txt", "base\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    dir
}

/// `n.txt` written as `text` and staged as a new file.
fn add_new(dir: &Path, text: &str) {
    write(dir, "n.txt", text);
    git(dir, &["add", "n.txt"]);
}

fn assert_untracked(dir: &Path, path: &str) {
    assert_eq!(index_bytes(dir, path), None, "{path} left the index");
    let (unstaged, staged) = lists(dir);
    assert_eq!(status_of(&staged, path), None, "{path} staged: {staged:?}");
    assert_eq!(status_of(&unstaged, path), Some('?'), "{path} untracked: {unstaged:?}");
}

#[test]
fn unstage_every_line_of_added_file_untracks_it() {
    let dir = based("lines-whole-added");
    add_new(&dir, "a\nb\n");
    apply(&dir, "n.txt", HunkOp::Unstage, &[('+', "a"), ('+', "b")]);
    assert_untracked(&dir, "n.txt");
    assert_eq!(read(&dir, "n.txt"), b"a\nb\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_added_file_line_by_line() {
    let dir = based("lines-whole-added-steps");
    add_new(&dir, "a\nb\nc\n");
    apply(&dir, "n.txt", HunkOp::Unstage, &[('+', "b")]);
    assert_eq!(index_text(&dir, "n.txt"), "a\nc\n");
    assert_eq!(status_of(&lists(&dir).1, "n.txt"), Some('A'), "a partial unstage keeps the entry");
    apply(&dir, "n.txt", HunkOp::Unstage, &[('+', "a"), ('+', "c")]);
    assert_untracked(&dir, "n.txt");
    assert_eq!(read(&dir, "n.txt"), b"a\nb\nc\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_every_line_on_unborn_head() {
    let dir = scratch_repo("lines-whole-unborn");
    add_new(&dir, "a\nb\n");
    apply(&dir, "n.txt", HunkOp::Unstage, &[('+', "a"), ('+', "b")]);
    assert_untracked(&dir, "n.txt");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A repository with `d.txt` committed as "a\nb\nc\n" and then deleted from the working tree.
fn deleted(name: &str) -> PathBuf {
    let dir = based(name);
    write(&dir, "d.txt", "a\nb\nc\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "d"]);
    std::fs::remove_file(dir.join("d.txt")).unwrap();
    dir
}

fn assert_deletion_staged(dir: &Path) {
    assert_eq!(index_bytes(dir, "d.txt"), None, "d.txt left the index");
    let (unstaged, staged) = lists(dir);
    assert_eq!(status_of(&staged, "d.txt"), Some('D'), "{staged:?}");
    assert_eq!(status_of(&unstaged, "d.txt"), None, "{unstaged:?}");
    assert!(!dir.join("d.txt").exists());
}

#[test]
fn stage_every_line_of_deleted_file_stages_the_deletion() {
    let dir = deleted("lines-whole-deleted");
    apply(&dir, "d.txt", HunkOp::Stage, &[('-', "a"), ('-', "b"), ('-', "c")]);
    assert_deletion_staged(&dir);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stage_deleted_file_line_by_line() {
    let dir = deleted("lines-whole-deleted-steps");
    apply(&dir, "d.txt", HunkOp::Stage, &[('-', "a"), ('-', "c")]);
    assert_eq!(index_text(&dir, "d.txt"), "b\n", "a partial stage keeps the entry");
    apply(&dir, "d.txt", HunkOp::Stage, &[('-', "b")]);
    assert_deletion_staged(&dir);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn really_empty_files_keep_their_entry() {
    let dir = scratch_repo("lines-whole-empty");
    write(&dir, "e.txt", "");
    write(&dir, "t.txt", "a\nb\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);

    // Committed empty, given lines and staged, then every line unstaged: back to empty.
    write(&dir, "e.txt", "x\ny\n");
    git(&dir, &["add", "e.txt"]);
    apply(&dir, "e.txt", HunkOp::Unstage, &[('+', "x"), ('+', "y")]);
    assert_eq!(index_bytes(&dir, "e.txt"), Some(Vec::new()));
    assert_eq!(status_of(&lists(&dir).1, "e.txt"), None);

    // Emptied in the working tree (the file is still there): staging every line stages it empty.
    write(&dir, "t.txt", "");
    apply(&dir, "t.txt", HunkOp::Stage, &[('-', "a"), ('-', "b")]);
    assert_eq!(index_bytes(&dir, "t.txt"), Some(Vec::new()));
    assert_eq!(status_of(&lists(&dir).1, "t.txt"), Some('M'));
    assert!(dir.join("t.txt").exists());

    // A new empty file has no lines to select.
    write(&dir, "z.txt", "");
    git(&dir, &["add", "z.txt"]);
    assert!(load_diff(&dir, &DiffTarget::Staged("z.txt".into())).unwrap().hunks.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn whitespace_hidden_path_removes_too() {
    let ignore = LoadDiffOptions { ignore_whitespace: true, whole_file: false };
    let dir = based("lines-whole-shown");
    add_new(&dir, "a\nb\n");
    let staged = DiffTarget::Staged("n.txt".into());
    let hunk = load_diff_with(&dir, &staged, ignore).unwrap().hunks[0].clone();
    apply_shown(&dir, "n.txt", 0, &hunk, None, HunkOp::Unstage).unwrap();
    assert_untracked(&dir, "n.txt");

    std::fs::remove_file(dir.join("base.txt")).unwrap();
    let unstaged = DiffTarget::Unstaged("base.txt".into());
    let (hunk, all) = pick(&dir, &unstaged, &[('-', "base")]);
    assert_eq!(load_diff_with(&dir, &unstaged, ignore).unwrap().hunks[0], hunk);
    apply_shown(&dir, "base.txt", 0, &hunk, Some(&all), HunkOp::Stage).unwrap();
    assert_eq!(index_bytes(&dir, "base.txt"), None);
    assert_eq!(status_of(&lists(&dir).1, "base.txt"), Some('D'));
    let _ = std::fs::remove_dir_all(&dir);
}
