//! Unstaging a staged new file (or a staged rename's new path) whose working copy is already
//! gone, against real repositories: only the index changes, and every refusal changes nothing.

use super::workfile_tests::{commit, conflicted_repo, index_id, status, text};
use super::*;
use crate::testutil::*;

#[test]
fn unstage_gone_removes_the_entry() {
    let dir = scratch_repo("wfgone-basic");
    commit(&dir, &[("a.txt", "a\n")], "one");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    write(&dir, "sub/n.txt", "new\n");
    git(&dir, &["add", "sub/n.txt"]);
    std::fs::remove_file(dir.join("sub/n.txt")).unwrap();

    unstage_gone_new_file(&dir, "sub/n.txt").unwrap();
    assert_eq!(index_id(&dir, "sub/n.txt"), None);
    assert_eq!(status(&dir), "");
    assert!(dir.join("sub").exists(), "nothing on disk is touched");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_gone_rename_restores_the_source() {
    let dir = scratch_repo("wfgone-rename");
    commit(&dir, &[("old.txt", "one\ntwo\nthree\n")], "one");
    let head = index_id(&dir, "old.txt");
    git(&dir, &["mv", "old.txt", "new.txt"]);
    std::fs::remove_file(dir.join("new.txt")).unwrap();

    unstage_gone_new_file(&dir, "new.txt").unwrap();
    assert_eq!(index_id(&dir, "old.txt"), head);
    assert_eq!(index_id(&dir, "new.txt"), None);
    assert_eq!(status(&dir), " D old.txt");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_gone_unborn_head() {
    let dir = scratch_repo("wfgone-unborn");
    write(&dir, "first.txt", "x\n");
    git(&dir, &["add", "first.txt"]);
    std::fs::remove_file(dir.join("first.txt")).unwrap();

    unstage_gone_new_file(&dir, "first.txt").unwrap();
    assert_eq!(git2::Repository::open(&dir).unwrap().index().unwrap().len(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_gone_refuses_when_the_file_is_back() {
    let dir = scratch_repo("wfgone-back");
    commit(&dir, &[("a.txt", "a\n")], "one");
    write(&dir, "n.txt", "new\n");
    git(&dir, &["add", "n.txt"]);
    let staged = index_id(&dir, "n.txt");

    let e = unstage_gone_new_file(&dir, "n.txt").unwrap_err();
    assert!(e.to_string().contains("working tree again"), "{e}");
    assert_eq!(index_id(&dir, "n.txt"), staged);
    assert_eq!(text(&dir, "n.txt"), "new\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_gone_ignores_a_folder_at_the_path() {
    let dir = scratch_repo("wfgone-folder");
    commit(&dir, &[("a.txt", "a\n")], "one");
    write(&dir, "n", "new\n");
    git(&dir, &["add", "n"]);
    std::fs::remove_file(dir.join("n")).unwrap();
    std::fs::create_dir(dir.join("n")).unwrap();

    unstage_gone_new_file(&dir, "n").unwrap();
    assert_eq!(index_id(&dir, "n"), None);
    assert!(dir.join("n").is_dir());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_gone_refusals() {
    let dir = scratch_repo("wfgone-refuse");
    commit(&dir, &[("t.txt", "one\n")], "one");
    let e = unstage_gone_new_file(&dir, "u.txt").unwrap_err();
    assert!(e.to_string().contains("isn't staged"), "{e}");

    write(&dir, "t.txt", "two\n");
    git(&dir, &["add", "t.txt"]);
    let staged = index_id(&dir, "t.txt");
    std::fs::remove_file(dir.join("t.txt")).unwrap();
    let before = status(&dir);
    let e = unstage_gone_new_file(&dir, "t.txt").unwrap_err();
    assert!(e.to_string().contains("last commit"), "{e}");
    assert_eq!(index_id(&dir, "t.txt"), staged);
    assert_eq!(status(&dir), before);
    let _ = std::fs::remove_dir_all(&dir);

    let dir = conflicted_repo("wfgone-conflict");
    let before = status(&dir);
    assert!(unstage_gone_new_file(&dir, "f.txt").is_err());
    assert_eq!(status(&dir), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_gone_treats_glob_characters_literally() {
    let dir = scratch_repo("wfgone-glob");
    commit(&dir, &[("a.txt", "one\n"), ("b.txt", "one\n")], "one");
    write(&dir, "a.txt", "staged\n");
    write(&dir, "b.txt", "staged\n");
    git(&dir, &["add", "a.txt", "b.txt"]);
    let (a, b) = (index_id(&dir, "a.txt"), index_id(&dir, "b.txt"));
    write(&dir, "[ab].txt", "new\n");
    git(&dir, &["--literal-pathspecs", "add", "[ab].txt"]);
    std::fs::remove_file(dir.join("[ab].txt")).unwrap();

    unstage_gone_new_file(&dir, "[ab].txt").unwrap();
    assert_eq!(index_id(&dir, "[ab].txt"), None);
    assert_eq!((index_id(&dir, "a.txt"), index_id(&dir, "b.txt")), (a, b), "other staged edits stay");
    let _ = std::fs::remove_dir_all(&dir);
}
