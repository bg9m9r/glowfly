//! Deleting a staged new file against real repositories: the working copy, its emptied folders
//! and its index entry all go, and every refusal changes nothing.

use super::workfile_tests::{commit, conflicted_repo, index_id, status};
use super::*;
use crate::testutil::*;
use std::path::Path;

/// Neither list from `status_at` mentions `path`.
fn not_in_status(dir: &Path, path: &str) -> bool {
    let (unstaged, staged, _) = status_at(dir).unwrap();
    unstaged.iter().chain(&staged).all(|f| f.path != path)
}

#[test]
fn removes_file_folder_and_index_entry() {
    let dir = scratch_repo("wfrm-basic");
    commit(&dir, &[("a.txt", "a\n")], "one");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    write(&dir, "sub/n.txt", "new\n");
    git(&dir, &["add", "sub/n.txt"]);

    delete_staged_new_file(&dir, "sub/n.txt").unwrap();
    assert!(!dir.join("sub/n.txt").exists());
    assert!(!dir.join("sub").exists(), "the emptied folder is pruned");
    assert_eq!(index_id(&dir, "sub/n.txt"), None);
    assert!(not_in_status(&dir, "sub/n.txt"));
    assert_eq!(status(&dir), "");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn removes_file_with_unstaged_edits() {
    let dir = scratch_repo("wfrm-edits");
    commit(&dir, &[("a.txt", "a\n")], "one");
    write(&dir, "n.txt", "new\n");
    git(&dir, &["add", "n.txt"]);
    write(&dir, "n.txt", "new\nmore\n");

    delete_staged_new_file(&dir, "n.txt").unwrap();
    assert!(!dir.join("n.txt").exists());
    assert_eq!(index_id(&dir, "n.txt"), None);
    assert!(not_in_status(&dir, "n.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unborn_head_counts_every_staged_file_as_new() {
    let dir = scratch_repo("wfrm-unborn");
    write(&dir, "first.txt", "x\n");
    git(&dir, &["add", "first.txt"]);

    delete_staged_new_file(&dir, "first.txt").unwrap();
    assert!(!dir.join("first.txt").exists());
    assert_eq!(index_id(&dir, "first.txt"), None);
    assert!(not_in_status(&dir, "first.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refuses_file_in_the_last_commit() {
    let dir = scratch_repo("wfrm-tracked");
    commit(&dir, &[("t.txt", "one\n")], "one");
    write(&dir, "t.txt", "two\n");
    git(&dir, &["add", "t.txt"]);
    let staged = index_id(&dir, "t.txt");

    let e = delete_staged_new_file(&dir, "t.txt").unwrap_err();
    assert!(e.to_string().contains("last commit"), "{e}");
    assert_eq!(std::fs::read_to_string(dir.join("t.txt")).unwrap(), "two\n");
    assert_eq!(index_id(&dir, "t.txt"), staged);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refuses_untracked_file() {
    let dir = scratch_repo("wfrm-untracked");
    commit(&dir, &[("a.txt", "a\n")], "one");
    write(&dir, "u.txt", "u\n");

    let e = delete_staged_new_file(&dir, "u.txt").unwrap_err();
    assert!(e.to_string().contains("isn't staged"), "{e}");
    assert!(dir.join("u.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_working_copy_keeps_the_index_entry() {
    let dir = scratch_repo("wfrm-missing");
    commit(&dir, &[("a.txt", "a\n")], "one");
    write(&dir, "n.txt", "new\n");
    git(&dir, &["add", "n.txt"]);
    let staged = index_id(&dir, "n.txt");
    std::fs::remove_file(dir.join("n.txt")).unwrap();

    let e = delete_staged_new_file(&dir, "n.txt").unwrap_err();
    assert!(e.is_not_found(), "{e}");
    assert!(staged.is_some());
    assert_eq!(index_id(&dir, "n.txt"), staged, "still staged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refuses_conflicted_path() {
    let dir = conflicted_repo("wfrm-conflict");
    let before = std::fs::read_to_string(dir.join("f.txt")).unwrap();
    let status_before = status(&dir);

    assert!(delete_staged_new_file(&dir, "f.txt").is_err());
    assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap(), before);
    assert_eq!(status(&dir), status_before);
    let _ = std::fs::remove_dir_all(&dir);
}
