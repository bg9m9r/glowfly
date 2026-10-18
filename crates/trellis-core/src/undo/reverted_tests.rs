//! Undoing and redoing reverts of part of a commit against real repositories: the exact working
//! copy from before comes back, a removed file is recreated, and a file edited since is never
//! overwritten.

use super::*;
use crate::repo::{self, DiffTarget, RevertPart, RevertRequest, Reverted};
use crate::testutil::*;
use std::path::{Path, PathBuf};

fn read(dir: &Path, f: &str) -> Option<Vec<u8>> {
    std::fs::read(dir.join(f)).ok()
}

/// Plans and carries out an undo (or redo) of `a`.
fn apply(dir: &Path, a: &Action, redo: bool) {
    match plan(dir, a, redo).unwrap() {
        Plan::Write { path, content } => write_saved(dir, &path, content).unwrap(),
        other => panic!("expected a write, got {other:?}"),
    }
}

/// `f.txt` committed as "one\n", then with `two` added (HEAD); the target is that commit.
fn added_line(name: &str) -> (PathBuf, DiffTarget) {
    let dir = scratch_repo(name);
    write(&dir, "f.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    write(&dir, "f.txt", "one\ntwo\n");
    git(&dir, &["commit", "-q", "-am", "two"]);
    let head = git2::Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
    (dir, DiffTarget::Commit(head, "f.txt".into()))
}

fn hunk_request(dir: &Path, target: &DiffTarget) -> RevertRequest {
    let expected = repo::load_diff(dir, target).unwrap().hunks[0].clone();
    RevertRequest { target: target.clone(), part: RevertPart::Hunk { index: 0, expected }, dest: None, ignore_whitespace: false }
}

#[test]
fn undo_and_redo_of_a_hunk_revert() {
    let (dir, target) = added_line("rv-hunk");
    // The working copy has other edits the revert keeps, CRLF and all.
    let before: &[u8] = b"zero\r\none\ntwo\n";
    std::fs::write(dir.join("f.txt"), before).unwrap();
    let req = hunk_request(&dir, &target);
    let (r, a) = revert_saving(&dir, "f.txt", Discarded::RevertedHunk, || repo::revert(&dir, &req)).unwrap();
    assert_eq!(r, Reverted::Applied);
    let a = a.expect("undoable");
    let Action::Discard { what: Discarded::RevertedHunk, before: Some(_), after: Some(_), .. } = &a else { panic!("{a:?}") };
    let reverted = read(&dir, "f.txt").unwrap();
    assert_eq!(reverted, b"zero\r\none\n");

    assert!(matches!(plan(&dir, &a, false).unwrap(), Plan::Write { content: Some(_), .. }));
    apply(&dir, &a, false);
    assert_eq!(read(&dir, "f.txt").unwrap(), before);
    apply(&dir, &a, true);
    assert_eq!(read(&dir, "f.txt").unwrap(), reverted);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_edit_after_the_revert_is_never_overwritten() {
    let (dir, target) = added_line("rv-stale");
    let req = hunk_request(&dir, &target);
    let (_, a) = revert_saving(&dir, "f.txt", Discarded::RevertedHunk, || repo::revert(&dir, &req)).unwrap();
    let a = a.unwrap();
    write(&dir, "f.txt", "one\nedited\n");
    let Plan::Refuse(r) = plan(&dir, &a, false).unwrap() else { panic!("expected a refusal") };
    assert!(r.is_stale());
    assert!(r.message().contains("f.txt has changed since; the overwritten version is kept as blob"), "{}", r.message());
    assert_eq!(read(&dir, "f.txt").unwrap(), b"one\nedited\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_brings_back_a_file_the_revert_removed() {
    let dir = scratch_repo("rv-removed");
    write(&dir, "base.txt", "x\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    write(&dir, "new.txt", "added\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "add"]);
    let head = git2::Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
    let req = hunk_request(&dir, &DiffTarget::Commit(head, "new.txt".into()));
    let (r, a) = revert_saving(&dir, "new.txt", Discarded::RevertedHunk, || repo::revert(&dir, &req)).unwrap();
    assert_eq!(r, Reverted::Removed);
    let a = a.unwrap();
    assert!(matches!(a, Action::Discard { before: Some(_), after: None, .. }), "{a:?}");
    assert!(read(&dir, "new.txt").is_none());
    apply(&dir, &a, false);
    assert_eq!(read(&dir, "new.txt").unwrap(), b"added\n");
    apply(&dir, &a, true);
    assert!(read(&dir, "new.txt").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_changed_records_nothing_and_errors_drop_the_pair() {
    let (dir, _) = added_line("rv-nothing");
    let (r, a) = revert_saving(&dir, "f.txt", Discarded::RevertedFile, || Ok(Reverted::Applied)).unwrap();
    assert_eq!((r, a), (Reverted::Applied, None));
    let e = revert_saving(&dir, "f.txt", Discarded::RevertedFile, || Err(crate::error::Error::NotFound("gone".into())));
    assert!(e.is_err_and(|e| e.is_not_found()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn revert_texts() {
    let a = |what| Action::Discard { path: "a.txt".into(), what, before: None, after: None };
    assert_eq!(a(Discarded::RevertedHunk).describe(), "revert of a hunk in a.txt");
    assert_eq!(a(Discarded::RevertedLines(1)).describe(), "revert of 1 line in a.txt");
    assert_eq!(a(Discarded::RevertedLines(3)).describe(), "revert of 3 lines in a.txt");
    assert_eq!(a(Discarded::RevertedFile).describe(), "revert of the changes to a.txt");
    assert_eq!(a(Discarded::RevertedHunk).wrote(true), "Reverted the hunk in a.txt again");
    assert_eq!(a(Discarded::RevertedLines(3)).wrote(true), "Reverted the lines in a.txt again");
    assert_eq!(a(Discarded::RevertedFile).wrote(true), "Reverted the changes to a.txt again");
    for what in [Discarded::RevertedHunk, Discarded::RevertedLines(2), Discarded::RevertedFile] {
        assert_eq!(a(what).wrote(false), "Restored a.txt");
    }
}
