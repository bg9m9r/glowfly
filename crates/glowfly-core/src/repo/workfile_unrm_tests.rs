//! Putting a deleted file back for undo against real repositories: the staged entry kept before
//! Unstage and delete, the exact bytes and pruned folders written back, nothing ever
//! overwritten, and the staged new file or staged rename shown again.

use super::workfile_tests::{commit, index_id, status};
use super::*;
use crate::testutil::*;
use std::path::Path;

fn staged(dir: &Path) -> Vec<(String, char)> {
    let (_, staged, _) = status_at(dir, true).unwrap();
    staged.into_iter().map(|f| (f.path, f.status)).collect()
}

#[test]
fn the_staged_entry_of_a_new_file_and_of_a_rename() {
    let dir = scratch_repo("wfunrm-staged");
    commit(&dir, &[("old.txt", "one\ntwo\nthree\nfour\nfive\n")], "one");
    write(&dir, "n.txt", "new\n");
    git(&dir, &["add", "n.txt"]);
    let s = staged_new(&dir, "n.txt").unwrap();
    assert_eq!((Some(s.blob), s.mode, s.source.clone()), (index_id(&dir, "n.txt"), 0o100644, None));
    assert!(staged_new_matches(&dir, "n.txt", &s).unwrap());
    assert!(!index_clear_for(&dir, "n.txt", &s).unwrap(), "still staged");

    git(&dir, &["mv", "old.txt", "moved.txt"]);
    let r = staged_new(&dir, "moved.txt").unwrap();
    assert_eq!(r.source.as_deref(), Some(&b"old.txt"[..]));
    assert!(staged_new_matches(&dir, "moved.txt", &r).unwrap());
    assert!(staged_new(&dir, "old.txt").is_err(), "the source isn't staged as new");
    assert!(staged_new(&dir, "untracked.txt").is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undelete_recreates_folders_and_exact_bytes() {
    let dir = scratch_repo("wfunrm-bytes");
    commit(&dir, &[("a.txt", "a\n")], "one");
    let bytes = b"crlf\r\nand \xff\xfe not utf-8\r\n";
    undelete_file(&dir, "sub/deep/f.bin", bytes, Some(false), None).unwrap();
    assert_eq!(std::fs::read(dir.join("sub/deep/f.bin")).unwrap(), bytes);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undelete_never_overwrites() {
    let dir = scratch_repo("wfunrm-exists");
    commit(&dir, &[("a.txt", "a\n")], "one");
    write(&dir, "f.txt", "someone else's\n");
    let e = undelete_file(&dir, "f.txt", b"mine\n", None, None).unwrap_err();
    assert!(e.to_string().contains("is in your working tree again"), "{e}");
    assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap(), "someone else's\n");
    // A refused write leaves no folder behind either.
    std::fs::create_dir(dir.join("d")).unwrap();
    write(&dir, "d/x.txt", "there\n");
    assert!(undelete_file(&dir, "d/x.txt", b"mine\n", None, None).is_err());
    assert_eq!(std::fs::read_to_string(dir.join("d/x.txt")).unwrap(), "there\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undelete_stages_a_new_file_again() {
    let dir = scratch_repo("wfunrm-new");
    commit(&dir, &[("a.txt", "a\n")], "one");
    std::fs::create_dir(dir.join("sub")).unwrap();
    write(&dir, "sub/n.txt", "new\n");
    git(&dir, &["add", "sub/n.txt"]);
    let s = staged_new(&dir, "sub/n.txt").unwrap();
    delete_staged_new_file(&dir, "sub/n.txt", Removal::Permanent).unwrap();
    assert!(index_clear_for(&dir, "sub/n.txt", &s).unwrap());

    undelete_file(&dir, "sub/n.txt", b"new\n", None, Some(&s)).unwrap();
    assert_eq!(std::fs::read_to_string(dir.join("sub/n.txt")).unwrap(), "new\n");
    assert_eq!(index_id(&dir, "sub/n.txt"), Some(s.blob));
    assert_eq!(staged(&dir), [("sub/n.txt".to_owned(), 'A')]);
    assert_eq!(status(&dir), "A  sub/n.txt");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undelete_brings_back_a_staged_rename() {
    let dir = scratch_repo("wfunrm-rename");
    let text = "one\ntwo\nthree\nfour\nfive\n";
    commit(&dir, &[("old.txt", text)], "one");
    git(&dir, &["mv", "old.txt", "new.txt"]);
    let s = staged_new(&dir, "new.txt").unwrap();
    delete_staged_new_file(&dir, "new.txt", Removal::Permanent).unwrap();
    assert_eq!(status(&dir), " D old.txt");
    assert!(index_clear_for(&dir, "new.txt", &s).unwrap());

    undelete_file(&dir, "new.txt", text.as_bytes(), None, Some(&s)).unwrap();
    assert_eq!(staged(&dir), [("new.txt".to_owned(), 'R')]);
    assert_eq!(index_id(&dir, "old.txt"), None, "the source's deletion is staged again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_restaged_path_or_changed_source_is_not_clear() {
    let dir = scratch_repo("wfunrm-clear");
    commit(&dir, &[("old.txt", "one\ntwo\nthree\nfour\nfive\n")], "one");
    git(&dir, &["mv", "old.txt", "new.txt"]);
    let s = staged_new(&dir, "new.txt").unwrap();
    delete_staged_new_file(&dir, "new.txt", Removal::Permanent).unwrap();
    // The source restored and edited, then staged: no longer what the delete left.
    write(&dir, "old.txt", "edited\n");
    git(&dir, &["add", "old.txt"]);
    assert!(!index_clear_for(&dir, "new.txt", &s).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}
