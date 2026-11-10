//! Undoing and redoing several files discarded as one entry (`Action::Batch`) against real
//! repositories, with a stand-in trash folder: every file comes back and goes again, a file put
//! back by hand is skipped, and a file changed since refuses the whole entry; the same for
//! several files deleted (`BatchOp::Delete`), a staged-new one staged again.

use super::test_bin::*;
use super::*;
use crate::repo;
use crate::testkit::git_out;
use crate::testutil::*;
use std::path::{Path, PathBuf};

fn read(dir: &Path, f: &str) -> Option<Vec<u8>> {
    std::fs::read(dir.join(f)).ok()
}

/// A repository with a.txt and b.txt committed and edited, and the untracked c.txt.
fn base(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "a\n");
    write(&dir, "b.txt", "b\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    write(&dir, "a.txt", "a edited\n");
    write(&dir, "b.txt", "b edited\n");
    write(&dir, "c.txt", "c new\n");
    dir
}

/// Discards a.txt, b.txt and c.txt the way a multi-file discard does: the entry it records.
fn discard_all(dir: &Path) -> Action {
    let tracked = |p: &str| discard_saving(dir, p, Discarded::File, || repo::discard_file(dir, p)).unwrap().expect("undoable");
    let c = discard_untracked_saving(dir, "c.txt", DeleteVia::Trash, false, T).unwrap().undo.expect("undoable");
    batch(BatchOp::Discard, vec![tracked("a.txt"), tracked("b.txt"), c]).expect("a batch")
}

/// Carries out one part's plan, as the runner does.
fn carry(dir: &Path, p: Plan) {
    match p {
        Plan::Write { path, content } => write_saved(dir, &path, content).unwrap(),
        Plan::PutBack { path, content, staged, via } => {
            put_back(dir, &path, content, staged.as_ref(), via, T).unwrap();
        }
        Plan::DeleteAgain { path, via, staged } => {
            delete_again(dir, &path, via, staged, T).unwrap();
        }
        other => panic!("not a file step: {other:?}"),
    }
}

fn steps(dir: &Path, a: &Action, redo: bool) -> Vec<(Action, Plan)> {
    match plan(dir, a, redo).unwrap() {
        Plan::Each(steps) => steps,
        other => panic!("expected each part's plan, got {other:?}"),
    }
}

#[test]
fn batch_undo_puts_every_file_back_and_redo_discards_them_again() {
    let dir = base("ubatch-undo");
    let a = discard_all(&dir);
    assert_eq!(read(&dir, "a.txt").unwrap(), b"a\n");
    assert!(read(&dir, "c.txt").is_none());

    let undo = steps(&dir, &a, false);
    assert_eq!(undo.len(), 3);
    undo.into_iter().for_each(|(_, p)| carry(&dir, p));
    assert_eq!(read(&dir, "a.txt").unwrap(), b"a edited\n");
    assert_eq!(read(&dir, "b.txt").unwrap(), b"b edited\n");
    assert_eq!(read(&dir, "c.txt").unwrap(), b"c new\n");

    let redo = steps(&dir, &a, true);
    assert_eq!(redo.len(), 3);
    redo.into_iter().for_each(|(_, p)| carry(&dir, p));
    assert_eq!(read(&dir, "b.txt").unwrap(), b"b\n");
    assert!(read(&dir, "c.txt").is_none(), "discarded again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn batch_skips_a_part_already_put_back() {
    let dir = base("ubatch-skip");
    let a = discard_all(&dir);
    // An earlier undo stopped after a.txt.
    write(&dir, "a.txt", "a edited\n");
    let undo = steps(&dir, &a, false);
    let paths: Vec<String> = undo
        .iter()
        .map(|(part, _)| match part {
            Action::Discard { path, .. } | Action::DeleteFile { path, .. } => path.clone(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(paths, ["b.txt", "c.txt"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn batch_refuses_when_one_file_changed_since() {
    let dir = base("ubatch-changed");
    let a = discard_all(&dir);
    write(&dir, "b.txt", "newer work\n");
    let Plan::Refuse(r) = plan(&dir, &a, false).unwrap() else { panic!("expected a refusal") };
    assert!(r.is_stale() && r.message().contains("b.txt has changed since"), "{r:?}");
    assert_eq!(read(&dir, "b.txt").unwrap(), b"newer work\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn batch_with_every_part_done_is_stale() {
    let dir = base("ubatch-done");
    let a = discard_all(&dir);
    steps(&dir, &a, false).into_iter().for_each(|(_, p)| carry(&dir, p));
    let Plan::Refuse(r) = plan(&dir, &a, false).unwrap() else { panic!("expected a refusal") };
    assert!(r.is_stale() && r.message().contains("Every file already holds"), "{r:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn batch_of_one_is_the_part_itself() {
    let part = Action::Discard { path: "a.txt".into(), what: Discarded::File, before: None, after: None };
    assert_eq!(batch(BatchOp::Discard, Vec::new()), None);
    assert_eq!(batch(BatchOp::Discard, vec![part.clone()]), Some(part.clone()));
    assert!(matches!(batch(BatchOp::Discard, vec![part.clone(), part]), Some(Action::Batch { op: BatchOp::Discard, .. })));
}

#[test]
fn describe_names_the_count_and_batch_is_content() {
    let part = Action::Discard { path: "a.txt".into(), what: Discarded::File, before: None, after: None };
    let a = Action::Batch { op: BatchOp::Discard, parts: vec![part.clone(), part.clone(), part] };
    assert_eq!(a.describe(), "discard in 3 files");
    assert!(a.is_content());
}

/// A repository with a.txt committed and edited, n.txt staged as new and the untracked u.txt.
fn delete_base(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    write(&dir, "a.txt", "a edited\n");
    write(&dir, "n.txt", "n new\n");
    git(&dir, &["add", "n.txt"]);
    write(&dir, "u.txt", "u new\n");
    dir
}

/// Deletes a.txt, n.txt (leaving the index too) and u.txt to the stand-in trash, the way a
/// multi-file delete does: the entry it records.
fn delete_all(dir: &Path) -> Action {
    let gone = |p: &str| delete_saving(dir, p, DeleteVia::Trash, false, T).unwrap().undo.expect("undoable");
    let n = delete_staged_saving(dir, "n.txt", DeleteVia::Trash, false, T).unwrap().undo.expect("undoable");
    batch(BatchOp::Delete, vec![gone("a.txt"), n, gone("u.txt")]).expect("a batch")
}

fn indexed(dir: &Path, f: &str) -> bool {
    git_out(dir, &["ls-files", "--", f]).trim() == f
}

#[test]
fn batch_delete_undo_puts_every_file_back_and_redo_deletes_them_again() {
    let dir = delete_base("ubatch-delete");
    let a = delete_all(&dir);
    assert!(matches!(a, Action::Batch { op: BatchOp::Delete, .. }));
    assert!(["a.txt", "n.txt", "u.txt"].iter().all(|f| read(&dir, f).is_none()));
    assert!(!indexed(&dir, "n.txt"), "the staged-new file left the index");

    let undo = steps(&dir, &a, false);
    assert_eq!(undo.len(), 3);
    assert!(undo.iter().all(|(_, p)| matches!(p, Plan::PutBack { .. })), "{undo:?}");
    undo.into_iter().for_each(|(_, p)| carry(&dir, p));
    assert_eq!(read(&dir, "a.txt").unwrap(), b"a edited\n");
    assert_eq!(read(&dir, "n.txt").unwrap(), b"n new\n");
    assert_eq!(read(&dir, "u.txt").unwrap(), b"u new\n");
    assert!(indexed(&dir, "n.txt"), "staged again");

    let redo = steps(&dir, &a, true);
    assert_eq!(redo.len(), 3);
    assert!(redo.iter().all(|(_, p)| matches!(p, Plan::DeleteAgain { .. })), "{redo:?}");
    redo.into_iter().for_each(|(_, p)| carry(&dir, p));
    assert!(["a.txt", "n.txt", "u.txt"].iter().all(|f| read(&dir, f).is_none()), "deleted again");
    assert!(!indexed(&dir, "n.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn batch_delete_skips_a_file_put_back_by_hand() {
    let dir = delete_base("ubatch-delete-skip");
    let a = delete_all(&dir);
    write(&dir, "u.txt", "u new\n");
    let undo = steps(&dir, &a, false);
    let paths: Vec<String> = undo
        .iter()
        .map(|(part, _)| match part {
            Action::DeleteFile { path, .. } => path.clone(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(paths, ["a.txt", "n.txt"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn batch_delete_refuses_when_a_path_holds_something_else() {
    let dir = delete_base("ubatch-delete-other");
    let a = delete_all(&dir);
    write(&dir, "a.txt", "someone else's\n");
    let Plan::Refuse(r) = plan(&dir, &a, false).unwrap() else { panic!("expected a refusal") };
    assert!(r.message().contains("a.txt"), "{r:?}");
    assert_eq!(read(&dir, "a.txt").unwrap(), b"someone else's\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn batch_op_describes_the_count() {
    assert_eq!(BatchOp::Delete.describe(3), "delete of 3 files");
    assert_eq!(BatchOp::Discard.describe(2), "discard in 2 files");
}
