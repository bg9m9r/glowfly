//! Undoing and redoing single-file discards against real repositories: the exact bytes come
//! back, a deleted file is recreated, and a file edited since is never overwritten.

use super::*;
use crate::repo::{self, DiffTarget, HunkOp};
use crate::testutil::*;
use std::path::Path;

fn read(dir: &Path, f: &str) -> Option<Vec<u8>> {
    std::fs::read(dir.join(f)).ok()
}

/// Plans and carries out an undo (or redo) of `a`, which must be a discard.
fn apply(dir: &Path, a: &Action, redo: bool) {
    match plan(dir, a, redo).unwrap() {
        Plan::Write { path, content } => write_saved(dir, &path, content).unwrap(),
        other => panic!("expected a write, got {other:?}"),
    }
}

fn base(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "f.txt", "one\ntwo\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    dir
}

#[test]
fn undo_brings_a_discarded_file_back_byte_for_byte() {
    let dir = base("sv-file");
    let edited: &[u8] = b"one\r\ntwo changed\r\n\xff no newline";
    std::fs::write(dir.join("f.txt"), edited).unwrap();
    let a = discard_saving(&dir, "f.txt", || repo::discard_file(&dir, "f.txt")).unwrap().expect("undoable");
    assert_eq!(a.describe(), "discard in f.txt");
    assert_eq!(read(&dir, "f.txt").unwrap(), b"one\ntwo\n");

    apply(&dir, &a, false);
    assert_eq!(read(&dir, "f.txt").unwrap(), edited);
    apply(&dir, &a, true);
    assert_eq!(read(&dir, "f.txt").unwrap(), b"one\ntwo\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_of_a_discarded_deletion_deletes_again() {
    let dir = base("sv-deleted");
    std::fs::remove_file(dir.join("f.txt")).unwrap();
    let a = discard_saving(&dir, "f.txt", || repo::discard_file(&dir, "f.txt")).unwrap().expect("undoable");
    assert!(read(&dir, "f.txt").is_some(), "the discard restored it");
    apply(&dir, &a, false);
    assert!(read(&dir, "f.txt").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_of_a_hunk_discard() {
    let dir = base("sv-hunk");
    write(&dir, "f.txt", "one\ntwo\nthree\n");
    let d = repo::load_diff(&dir, &DiffTarget::Unstaged("f.txt".into())).unwrap();
    let a = discard_saving(&dir, "f.txt", || repo::apply_hunk(&dir, "f.txt", 0, &d.hunks[0], HunkOp::Discard)).unwrap().unwrap();
    assert_eq!(read(&dir, "f.txt").unwrap(), b"one\ntwo\n");
    apply(&dir, &a, false);
    assert_eq!(read(&dir, "f.txt").unwrap(), b"one\ntwo\nthree\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_edited_since_is_not_overwritten() {
    let dir = base("sv-edited");
    write(&dir, "f.txt", "lost?\n");
    let a = discard_saving(&dir, "f.txt", || repo::discard_file(&dir, "f.txt")).unwrap().unwrap();
    write(&dir, "f.txt", "newer work\n");
    let Plan::Refuse(r) = plan(&dir, &a, false).unwrap() else { panic!("expected a refusal") };
    assert!(r.is_stale() && r.message().contains("f.txt has changed since"), "{r:?}");
    let Action::Discard { before: Some(saved), .. } = &a else { panic!() };
    assert!(r.message().contains(&saved.blob.to_string()), "names the blob to recover from: {r:?}");
    assert!(r.message().contains("discarded version"), "{r:?}");
    assert_eq!(read(&dir, "f.txt").unwrap(), b"newer work\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_to_undo_when_nothing_changed_or_the_discard_failed() {
    let dir = base("sv-noop");
    assert_eq!(discard_saving(&dir, "f.txt", || repo::discard_file(&dir, "f.txt")).unwrap(), None);
    write(&dir, "f.txt", "kept\n");
    let failed = discard_saving(&dir, "f.txt", || Err(crate::error::Error::Other("no".into())));
    assert!(failed.is_err());
    assert_eq!(read(&dir, "f.txt").unwrap(), b"kept\n");
    let _ = std::fs::remove_dir_all(&dir);
}

fn set_readonly(path: &Path, on: bool) {
    let mut p = std::fs::metadata(path).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    p.set_readonly(on);
    std::fs::set_permissions(path, p).unwrap();
}

#[test]
fn undo_and_redo_replace_a_read_only_file() {
    let dir = base("sv-readonly");
    let f = dir.join("f.txt");
    std::fs::write(&f, "edited\n").unwrap();
    let a = discard_saving(&dir, "f.txt", || repo::discard_file(&dir, "f.txt")).unwrap().expect("undoable");
    // Read-only by the time of the undo (a tool or the user marked it so).
    set_readonly(&f, true);
    apply(&dir, &a, false);
    assert_eq!(read(&dir, "f.txt").unwrap(), b"edited\n");
    assert!(std::fs::metadata(&f).unwrap().permissions().readonly(), "still read-only");
    apply(&dir, &a, true);
    assert_eq!(read(&dir, "f.txt").unwrap(), b"one\ntwo\n");
    set_readonly(&f, false);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_deletes_a_read_only_file() {
    let dir = base("sv-readonly-delete");
    let f = dir.join("f.txt");
    std::fs::remove_file(&f).unwrap();
    let a = discard_saving(&dir, "f.txt", || repo::discard_file(&dir, "f.txt")).unwrap().expect("undoable");
    set_readonly(&f, true);
    apply(&dir, &a, false);
    assert!(read(&dir, "f.txt").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn undo_puts_the_executable_bit_back_unless_git_ignores_it() {
    use std::os::unix::fs::PermissionsExt;
    let mode = |dir: &Path| std::fs::metadata(dir.join("f.txt")).unwrap().permissions().mode() & 0o111;
    for file_mode in [true, false] {
        let dir = base(&format!("sv-exec-{file_mode}"));
        git(&dir, &["config", "core.fileMode", if file_mode { "true" } else { "false" }]);
        let saved = Saved { blob: git2::Repository::open(&dir).unwrap().blob(b"#!/bin/sh\n").unwrap(), executable: true };
        write_saved(&dir, "f.txt", Some(saved)).unwrap();
        assert_eq!(read(&dir, "f.txt").unwrap(), b"#!/bin/sh\n");
        assert_eq!(mode(&dir) != 0, file_mode, "core.fileMode={file_mode}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
