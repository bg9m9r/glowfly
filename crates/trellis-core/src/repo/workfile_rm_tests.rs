//! Deleting a staged new file (or a staged rename's new path) against real repositories: the
//! working copy, its emptied folders and its index entries all go, a rename's source is unstaged,
//! and every refusal changes nothing.

use super::workfile_tests::{commit, conflicted_repo, index_id, status, text};
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

/// A repository whose last commit has `old.txt` with several lines; returns it and that blob's id.
fn with_old(name: &str) -> (std::path::PathBuf, Option<git2::Oid>) {
    let dir = scratch_repo(name);
    commit(&dir, &[("old.txt", "one\ntwo\nthree\nfour\nfive\n")], "one");
    let head = index_id(&dir, "old.txt");
    assert!(head.is_some());
    (dir, head)
}

#[test]
fn staged_rename_unstages_the_source() {
    let (dir, head) = with_old("wfrm-rename");
    git(&dir, &["mv", "old.txt", "new.txt"]);

    delete_staged_new_file(&dir, "new.txt").unwrap();
    assert!(!dir.join("new.txt").exists());
    assert_eq!(index_id(&dir, "new.txt"), None);
    assert_eq!(index_id(&dir, "old.txt"), head);
    assert_eq!(status(&dir), " D old.txt");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staged_rename_into_folder_prunes_and_unstages() {
    let (dir, head) = with_old("wfrm-rename-sub");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    git(&dir, &["mv", "old.txt", "sub/new.txt"]);

    delete_staged_new_file(&dir, "sub/new.txt").unwrap();
    assert!(!dir.join("sub").exists(), "the emptied folder is pruned");
    assert_eq!(index_id(&dir, "old.txt"), head);
    assert_eq!(status(&dir), " D old.txt");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staged_rename_with_staged_edits() {
    let (dir, head) = with_old("wfrm-rename-edit");
    git(&dir, &["mv", "old.txt", "new.txt"]);
    write(&dir, "new.txt", "one\ntwo\nthree\nfour\nfive\nsix\n");
    git(&dir, &["add", "new.txt"]);
    assert!(status(&dir).starts_with('R'), "status pairs the rename");

    delete_staged_new_file(&dir, "new.txt").unwrap();
    assert_eq!(index_id(&dir, "old.txt"), head);
    assert_eq!(index_id(&dir, "new.txt"), None);
    assert_eq!(status(&dir), " D old.txt");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staged_rename_keeps_a_recreated_source() {
    let (dir, head) = with_old("wfrm-rename-back");
    git(&dir, &["mv", "old.txt", "new.txt"]);
    write(&dir, "old.txt", "something else\n");

    delete_staged_new_file(&dir, "new.txt").unwrap();
    assert_eq!(text(&dir, "old.txt"), "something else\n");
    assert_eq!(index_id(&dir, "old.txt"), head);
    assert_eq!(status(&dir), " M old.txt");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rename_found_with_diff_renames_off() {
    let (dir, head) = with_old("wfrm-rename-cfg");
    git(&dir, &["config", "diff.renames", "false"]);
    git(&dir, &["mv", "old.txt", "new.txt"]);

    delete_staged_new_file(&dir, "new.txt").unwrap();
    assert_eq!(index_id(&dir, "old.txt"), head);
    assert_eq!(status(&dir), " D old.txt");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unrelated_staged_deletion_stays_staged() {
    let dir = scratch_repo("wfrm-unrelated");
    commit(&dir, &[("gone.txt", "alpha\nbeta\ngamma\ndelta\n")], "one");
    git(&dir, &["rm", "-q", "gone.txt"]);
    write(&dir, "new.txt", "something entirely different\n");
    git(&dir, &["add", "new.txt"]);

    delete_staged_new_file(&dir, "new.txt").unwrap();
    assert_eq!(index_id(&dir, "new.txt"), None);
    assert_eq!(index_id(&dir, "gone.txt"), None);
    assert_eq!(status(&dir), "D  gone.txt");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn case_only_staged_rename() {
    let dir = scratch_repo("wfrm-case");
    commit(&dir, &[("readme.md", "hi\nthere\n")], "one");
    let head = index_id(&dir, "readme.md");
    rename_file_staged(&dir, "readme.md", "README.md").unwrap();

    delete_staged_new_file(&dir, "README.md").unwrap();
    let index = git2::Repository::open(&dir).unwrap().index().unwrap();
    let paths: Vec<String> = index.iter().map(|e| String::from_utf8_lossy(&e.path).into_owned()).collect();
    assert_eq!(paths, ["readme.md"]);
    assert_eq!(index_id(&dir, "readme.md"), head);
    assert_eq!(status(&dir), " D readme.md");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `a.txt` has a staged edit and a further unstaged one; returns the staged blob's id.
fn with_staged_edit(dir: &Path) -> Option<git2::Oid> {
    write(dir, "a.txt", "staged\n");
    git(dir, &["add", "a.txt"]);
    write(dir, "a.txt", "worktree\n");
    index_id(dir, "a.txt")
}

#[test]
fn glob_characters_in_the_new_path_are_literal() {
    let dir = scratch_repo("wfrm-glob");
    commit(&dir, &[("a.txt", "one\n")], "one");
    let staged = with_staged_edit(&dir);
    write(&dir, "[ab].txt", "new\n");
    git(&dir, &["--literal-pathspecs", "add", "[ab].txt"]);

    delete_staged_new_file(&dir, "[ab].txt").unwrap();
    assert_eq!(index_id(&dir, "[ab].txt"), None);
    assert_eq!(index_id(&dir, "a.txt"), staged, "a path the name matches as a glob keeps its staged edit");
    assert_eq!(status(&dir), "MM a.txt");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn glob_characters_in_the_rename_source_are_literal() {
    let dir = scratch_repo("wfrm-glob-src");
    commit(&dir, &[("a.txt", "one\n"), ("[ab].txt", "one\ntwo\nthree\nfour\n")], "one");
    let head = index_id(&dir, "[ab].txt");
    let staged = with_staged_edit(&dir);
    git(&dir, &["mv", "[ab].txt", "moved.txt"]);

    delete_staged_new_file(&dir, "moved.txt").unwrap();
    assert_eq!(index_id(&dir, "[ab].txt"), head);
    assert_eq!(index_id(&dir, "moved.txt"), None);
    assert_eq!(index_id(&dir, "a.txt"), staged, "a path the source matches as a glob keeps its staged edit");
    let _ = std::fs::remove_dir_all(&dir);
}
