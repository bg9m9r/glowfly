//! Undoing and redoing a file restored from a commit: the working copy it replaced comes back
//! byte for byte (or is removed again when there was none), redo writes the restored bytes
//! again, and a file edited since is never overwritten.

use super::*;
use crate::error::Error;
use crate::repo::RestoredFrom;
use crate::testutil::*;
use git2::Repository;
use std::path::{Path, PathBuf};

fn read(dir: &Path, f: &str) -> Option<Vec<u8>> {
    std::fs::read(dir.join(f)).ok()
}

/// Plans and carries out an undo (or redo) of `a`, which must plan a file write.
fn apply(dir: &Path, a: &Action, redo: bool) {
    match plan(dir, a, redo).unwrap() {
        Plan::Write { path, content } => write_saved(dir, &path, content).unwrap(),
        other => panic!("expected a write, got {other:?}"),
    }
}

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn commit(dir: &Path, file: &str, text: &str, msg: &str) -> Oid {
    write(dir, file, text);
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", msg]);
    head(dir)
}

/// `f.txt` committed as "v1\n" and then as "v2\n"; returns the folder and the first commit.
fn two_versions(name: &str) -> (PathBuf, Oid) {
    let dir = scratch_repo(name);
    let v1 = commit(&dir, "f.txt", "v1\n", "v1");
    commit(&dir, "f.txt", "v2\n", "v2");
    (dir, v1)
}

#[test]
fn restore_over_local_edits_undoes_byte_for_byte() {
    let (dir, v1) = two_versions("rs-bytes");
    let local: &[u8] = b"local\r\nedits\xff";
    std::fs::write(dir.join("f.txt"), local).unwrap();
    let (from, a) = restore_saving(&dir, v1, "f.txt", true).unwrap();
    assert_eq!(from, RestoredFrom::Commit);
    let a = a.expect("undoable");
    assert!(matches!(&a, Action::Restore { path, oid, .. } if path == "f.txt" && *oid == v1), "{a:?}");
    assert_eq!(a.describe(), "restore of f.txt");
    assert_eq!(read(&dir, "f.txt").unwrap(), b"v1\n");

    assert_eq!(a.wrote(false), "Put back the earlier working copy of f.txt");
    apply(&dir, &a, false);
    assert_eq!(read(&dir, "f.txt").unwrap(), local);
    assert_eq!(a.wrote(true), "Restored f.txt again");
    apply(&dir, &a, true);
    assert_eq!(read(&dir, "f.txt").unwrap(), b"v1\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_of_a_deleted_file_undo_removes_it() {
    let dir = scratch_repo("rs-deleted");
    let added = commit(&dir, "g.txt", "kept\n", "add g");
    git(&dir, &["rm", "-q", "g.txt"]);
    git(&dir, &["commit", "-q", "-m", "remove g"]);
    let (from, a) = restore_saving(&dir, head(&dir), "g.txt", false).unwrap();
    assert_eq!(from, RestoredFrom::Parent(added));
    let a = a.expect("undoable");
    assert!(matches!(&a, Action::Restore { before: None, after: Some(_), .. }), "{a:?}");
    assert_eq!(read(&dir, "g.txt").unwrap(), b"kept\n");

    assert_eq!(a.wrote(false), "Removed g.txt again");
    apply(&dir, &a, false);
    assert!(read(&dir, "g.txt").is_none());
    apply(&dir, &a, true);
    assert_eq!(read(&dir, "g.txt").unwrap(), b"kept\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_edited_since_is_refused_as_stale() {
    let (dir, v1) = two_versions("rs-stale");
    write(&dir, "f.txt", "mine\n");
    let a = restore_saving(&dir, v1, "f.txt", true).unwrap().1.expect("undoable");
    write(&dir, "f.txt", "newer\n");
    let Plan::Refuse(r) = plan(&dir, &a, false).unwrap() else { panic!("expected a refusal") };
    let Action::Restore { before: Some(saved), .. } = &a else { panic!("{a:?}") };
    assert!(r.is_stale(), "{r:?}");
    assert!(r.message().contains("f.txt has changed since"), "{r:?}");
    assert!(r.message().contains("overwritten version is kept as blob"), "{r:?}");
    assert!(r.message().contains(&saved.blob.to_string()), "names the blob to recover from: {r:?}");
    assert_eq!(read(&dir, "f.txt").unwrap(), b"newer\n", "untouched");

    write(&dir, "f.txt", "v1\n");
    apply(&dir, &a, false);
    assert_eq!(read(&dir, "f.txt").unwrap(), b"mine\n");
    write(&dir, "f.txt", "edited after the undo\n");
    let Plan::Refuse(r) = plan(&dir, &a, true).unwrap() else { panic!("expected a refusal") };
    assert!(r.is_stale() && r.message().contains("f.txt has changed since"), "{r:?}");
    assert!(!r.message().contains("blob"), "no blob hint on redo: {r:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_that_changes_nothing_records_nothing() {
    let (dir, _) = two_versions("rs-noop");
    assert_eq!(restore_saving(&dir, head(&dir), "f.txt", false).unwrap(), (RestoredFrom::Commit, None));
    assert_eq!(read(&dir, "f.txt").unwrap(), b"v2\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn would_overwrite_and_failures_record_nothing() {
    let (dir, v1) = two_versions("rs-fail");
    match restore_saving(&dir, v1, "nope.txt", true) {
        Err(RestoreFailed { error: Error::NotFound(_), undo: None }) => {}
        r => panic!("expected NotFound, got {r:?}"),
    }
    assert!(read(&dir, "nope.txt").is_none());
    // Untracked: its removal from the index is staged, and the working copy holds other bytes.
    git(&dir, &["rm", "-q", "--cached", "f.txt"]);
    write(&dir, "f.txt", "local\n");
    match restore_saving(&dir, v1, "f.txt", false) {
        Err(RestoreFailed { error: Error::WouldOverwrite(_), undo: None }) => {}
        r => panic!("expected WouldOverwrite, got {r:?}"),
    }
    assert_eq!(read(&dir, "f.txt").unwrap(), b"local\n", "untouched");
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn restore_over_a_symlink_still_runs_without_undo() {
    let (dir, v1) = two_versions("rs-symlink");
    std::fs::remove_file(dir.join("f.txt")).unwrap();
    std::os::unix::fs::symlink("elsewhere", dir.join("f.txt")).unwrap();
    let (_, a) = restore_saving(&dir, v1, "f.txt", true).unwrap();
    assert_eq!(a, None);
    assert_eq!(read(&dir, "f.txt").unwrap(), b"v1\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_through_a_smudge_filter_undoes() {
    let dir = scratch_repo("rs-filter");
    git(&dir, &["config", "filter.upper.clean", "tr a-z A-Z"]);
    git(&dir, &["config", "filter.upper.smudge", "tr A-Z a-z"]);
    write(&dir, ".gitattributes", "*.up filter=upper\n");
    let base = commit(&dir, "a.up", "line one\n", "base");
    commit(&dir, "a.up", "line two\n", "edit");
    write(&dir, "a.up", "local work\n");
    let a = restore_saving(&dir, base, "a.up", true).unwrap().1.expect("undoable");
    assert_eq!(read(&dir, "a.up").unwrap(), b"line one\n", "written through the smudge");
    apply(&dir, &a, false);
    assert_eq!(read(&dir, "a.up").unwrap(), b"local work\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A required smudge filter that fails: git removes the working copy before the filter runs,
/// then gives up. The failure still comes with an undo, and names the blob that keeps the edits.
#[test]
fn restore_that_fails_after_removing_the_file_can_be_undone() {
    let dir = scratch_repo("rs-failed-filter");
    git(&dir, &["config", "filter.x.clean", "cat"]);
    git(&dir, &["config", "filter.x.smudge", "cat"]);
    git(&dir, &["config", "filter.x.required", "true"]);
    write(
        &dir,
        ".gitattributes",
        "*.up filter=x
",
    );
    let base = commit(
        &dir, "a.up", "one
", "base",
    );
    commit(
        &dir, "a.up", "two
", "edit",
    );
    write(
        &dir,
        "a.up",
        "local edits
",
    );
    git(&dir, &["config", "filter.x.smudge", "false"]);

    let failed = restore_saving(&dir, base, "a.up", true).expect_err("the smudge fails");
    assert!(read(&dir, "a.up").is_none(), "git removed the file before the filter failed");
    let a = *failed.undo.expect("undoable although it failed");
    let Action::Restore { before: Some(saved), after: None, .. } = &a else { panic!("{a:?}") };
    let msg = failed.error.to_string();
    assert!(msg.contains("a.up changed anyway"), "{msg}");
    assert!(msg.contains(&format!("git cat-file -p {}", saved.blob)), "names the blob: {msg}");

    apply(&dir, &a, false);
    assert_eq!(
        read(&dir, "a.up").unwrap(),
        b"local edits
"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A discard that fails after changing the file says where its earlier contents are kept.
#[test]
fn discard_that_fails_after_changing_the_file_names_the_blob() {
    let (dir, _) = two_versions("rs-failed-discard");
    write(
        &dir, "f.txt", "mine
",
    );
    let err = discard_saving(&dir, "f.txt", || {
        std::fs::remove_file(dir.join("f.txt"))?;
        Err(Error::Other("cut short".into()))
    })
    .expect_err("the discard fails");
    let msg = err.to_string();
    assert!(msg.starts_with("cut short. f.txt changed anyway"), "{msg}");
    let blob = Repository::open(&dir)
        .unwrap()
        .blob(
            b"mine
",
        )
        .unwrap();
    assert!(msg.contains(&format!("git cat-file -p {blob}")), "{msg}");
    // A failure that left the file alone comes back unchanged.
    write(
        &dir, "f.txt", "mine
",
    );
    let err = discard_saving(&dir, "f.txt", || Err(Error::Other("refused".into()))).expect_err("fails");
    assert_eq!(err.to_string(), "refused");
    let _ = std::fs::remove_dir_all(&dir);
}
