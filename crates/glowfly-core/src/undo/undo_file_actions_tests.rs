//! Undo round trips for the file actions that used to leave no undo entry, against real
//! repositories and the stand-in trash: discarding an untracked file, Unstage deleted file, a
//! delete or revert that failed after it changed the file or index, and taking a trashed file
//! back out of the trash (or writing Glowfly's copy when the trash can't give it back).

use super::test_bin::*;
use super::*;
use crate::error::Error;
use crate::repo::{self, Deleted, TrashProblem, TrashRefusal, Untrashed};
use crate::testutil::*;
use std::path::{Path, PathBuf};

fn base(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "f.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    dir
}

/// Plans and carries out the undo of a delete with `t`, returning how the file came back.
fn undo_delete(dir: &Path, a: &Action, t: Trashers) -> PutBackHow {
    match plan(dir, a, false).unwrap() {
        Plan::PutBack { path, content, staged, via } => put_back(dir, &path, content, staged.as_ref(), via, t).unwrap(),
        other => panic!("expected a put-back, got {other:?}"),
    }
}

fn index_entry(dir: &Path, path: &str) -> Option<(git2::Oid, u32)> {
    git2::Repository::open(dir).unwrap().index().unwrap().get_path(Path::new(path), 0).map(|e| (e.id, e.mode))
}

#[test]
fn discarding_an_untracked_file_is_undone_like_a_delete() {
    let dir = base("ufa-discard");
    write(&dir, "u.txt", "untracked work\n");
    let before = in_bin("u.txt");
    let d = discard_untracked_saving(&dir, "u.txt", DeleteVia::Trash, T).unwrap();
    assert_eq!(d.how, Deleted::Trashed);
    let a = d.undo.expect("undoable");
    assert!(matches!(&a, Action::DeleteFile { path, staged: None, .. } if path == "u.txt"), "{a:?}");
    assert_eq!(in_bin("u.txt"), before + 1);

    assert_eq!(undo_delete(&dir, &a, T), PutBackHow::FromTrash);
    assert_eq!(std::fs::read_to_string(dir.join("u.txt")).unwrap(), "untracked work\n");
    assert_eq!(in_bin("u.txt"), before, "out of the trash again");

    // Redo discards it again; a permanent discard comes back from Glowfly's copy.
    assert!(matches!(plan(&dir, &a, true).unwrap(), Plan::DeleteAgain { .. }));
    write(&dir, "p.txt", "for good\n");
    let a = discard_untracked_saving(&dir, "p.txt", DeleteVia::Permanent, T).unwrap().undo.unwrap();
    assert_eq!(undo_delete(&dir, &a, T), PutBackHow::Written { trash: None });
    assert_eq!(std::fs::read_to_string(dir.join("p.txt")).unwrap(), "for good\n");

    // A file tracked by then is refused, keeping nothing to undo.
    let e = discard_untracked_saving(&dir, "f.txt", DeleteVia::Permanent, T).unwrap_err();
    assert!(e.undo.is_none() && dir.join("f.txt").exists(), "{e:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_deleted_file_keeps_its_staged_blob_and_is_undone() {
    let dir = base("ufa-unstage-gone");
    write(&dir, "n.txt", "staged only\n");
    git(&dir, &["add", "n.txt"]);
    std::fs::remove_file(dir.join("n.txt")).unwrap();
    let staged = index_entry(&dir, "n.txt");
    let a = unstage_gone_saving(&dir, "n.txt").unwrap();
    assert_eq!(a.describe(), "unstage of n.txt");
    assert_eq!(index_entry(&dir, "n.txt"), None);

    let Plan::Restage { path, staged: entry } = plan(&dir, &a, false).unwrap() else { panic!("a restage") };
    restage(&dir, &path, &entry).unwrap();
    assert_eq!(index_entry(&dir, "n.txt"), staged, "the same blob and mode");
    assert!(!dir.join("n.txt").exists(), "nothing on disk changes");
    assert_eq!(a.wrote(false), "Staged n.txt again");

    let Plan::UnstageAgain { path } = plan(&dir, &a, true).unwrap() else { panic!("an unstage") };
    unstage_again(&dir, &path).unwrap();
    assert_eq!(index_entry(&dir, "n.txt"), None);
    assert_eq!(a.wrote(true), "Removed n.txt from the staging area again");

    // Staged again meanwhile: undoing would lose that.
    write(&dir, "n.txt", "other\n");
    git(&dir, &["add", "n.txt"]);
    assert!(matches!(plan(&dir, &a, false).unwrap(), Plan::Refuse(r) if r.is_stale()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_deleted_file_of_a_rename_brings_the_staged_rename_back() {
    let dir = scratch_repo("ufa-unstage-rename");
    write(&dir, "old.txt", "one\ntwo\nthree\nfour\nfive\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["mv", "old.txt", "new.txt"]);
    std::fs::remove_file(dir.join("new.txt")).unwrap();
    let a = unstage_gone_saving(&dir, "new.txt").unwrap();
    assert!(index_entry(&dir, "old.txt").is_some(), "the source is back in the index");
    let Plan::Restage { path, staged } = plan(&dir, &a, false).unwrap() else { panic!("a restage") };
    restage(&dir, &path, &staged).unwrap();
    let (_, staged, _) = repo::status_at(&dir, true).unwrap();
    let staged: Vec<_> = staged.into_iter().map(|f| (f.path, f.status)).collect();
    assert_eq!(staged, [("new.txt".to_owned(), 'R')], "the rename pairs again");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Takes the file into the stand-in bin, then reports an error, as a trash that failed halfway.
fn trash_then_fail(full: &Path) -> Result<(), TrashRefusal> {
    to_bin(full)?;
    Err(TrashRefusal::new(TrashProblem::Other, "the bin's record couldn't be written"))
}

/// Takes the file into the stand-in bin and locks the index, so changing it afterwards fails.
fn trash_and_lock_index(full: &Path) -> Result<(), TrashRefusal> {
    let repo = git2::Repository::discover(full.parent().unwrap()).unwrap();
    to_bin(full)?;
    std::fs::write(repo.path().join("index.lock"), "").unwrap();
    Ok(())
}

fn refuse(_: &Path) -> Result<(), TrashRefusal> {
    Err(TrashRefusal::new(TrashProblem::InUse, "open elsewhere"))
}

#[test]
fn a_delete_that_fails_after_the_file_went_keeps_its_undo() {
    let dir = base("ufa-delete-failed");
    write(&dir, "u.txt", "work\n");
    let t = Trashers { system: trash_then_fail, ..T };
    let e = delete_saving(&dir, "u.txt", DeleteVia::Trash, t).unwrap_err();
    assert!(e.error.to_string().contains("was removed, but"), "{}", e.error);
    let a = *e.undo.expect("the file went, so it can be put back");
    assert!(!dir.join("u.txt").exists());
    assert_eq!(undo_delete(&dir, &a, T), PutBackHow::FromTrash);
    assert_eq!(std::fs::read_to_string(dir.join("u.txt")).unwrap(), "work\n");

    // A refusal leaves the file where it was: nothing to undo.
    let e = delete_saving(&dir, "u.txt", DeleteVia::Trash, Trashers { system: refuse, ..T }).unwrap_err();
    assert!(matches!(e.error, Error::TrashRefused(_)) && e.undo.is_none(), "{e:?}");
    assert!(dir.join("u.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_and_delete_that_couldnt_change_the_index_keeps_its_undo() {
    let dir = base("ufa-unstage-failed");
    write(&dir, "n.txt", "new\n");
    git(&dir, &["add", "n.txt"]);
    let staged = index_entry(&dir, "n.txt");
    let t = Trashers { system: trash_and_lock_index, ..T };
    let e = delete_staged_saving(&dir, "n.txt", DeleteVia::Trash, t).unwrap_err();
    assert!(e.error.to_string().contains("still staged"), "{}", e.error);
    std::fs::remove_file(dir.join(".git/index.lock")).unwrap();
    let a = *e.undo.expect("the file went");
    assert!(matches!(&a, Action::DeleteFile { staged: None, .. }), "the index never changed: {a:?}");
    assert_eq!(undo_delete(&dir, &a, T), PutBackHow::FromTrash);
    assert_eq!(std::fs::read_to_string(dir.join("n.txt")).unwrap(), "new\n");
    assert_eq!(index_entry(&dir, "n.txt"), staged, "still staged as it was");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_revert_that_fails_after_changing_the_file_keeps_its_undo() {
    let dir = base("ufa-revert-failed");
    let e = revert_saving(&dir, "f.txt", Discarded::RevertedFile, || {
        std::fs::write(dir.join("f.txt"), "half written").unwrap();
        Err(Error::Other("the filter failed".into()))
    })
    .unwrap_err();
    assert!(e.error.to_string().contains("git cat-file -p"), "names the kept blob: {}", e.error);
    let a = *e.undo.expect("the file changed");
    let Plan::Write { path, content } = plan(&dir, &a, false).unwrap() else { panic!("a write") };
    write_saved(&dir, &path, content).unwrap();
    assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap(), "one\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_trash_that_cant_give_the_file_back_gets_glowflys_copy_and_keeps_its_own() {
    let dir = base("ufa-untrash-fallback");
    write(&dir, "u.txt", "kept\n");
    let before = in_bin("u.txt");
    let a = delete_saving(&dir, "u.txt", DeleteVia::Trash, T).unwrap().undo.unwrap();
    let mac_like = Trashers { restore: unreadable, ..T };
    let how = undo_delete(&dir, &a, mac_like);
    assert_eq!(how, PutBackHow::Written { trash: Some(Untrashed::Unsupported) });
    assert_eq!(std::fs::read_to_string(dir.join("u.txt")).unwrap(), "kept\n");
    assert_eq!(in_bin("u.txt"), before + 1, "the trash's copy stays");
    assert_eq!(
        how.wrote("u.txt", false, DeleteVia::Trash),
        format!("Put back u.txt from Glowfly's copy (its copy in the {} stays there)", repo::TRASH_NAME)
    );

    // Emptied from the trash (or changed there): Glowfly's copy, never other bytes.
    std::fs::remove_file(dir.join("u.txt")).unwrap();
    for e in std::fs::read_dir(bin()).unwrap().flatten() {
        if e.file_name().to_string_lossy().ends_with("-u.txt") {
            std::fs::write(e.path(), "changed in the trash\n").unwrap();
        }
    }
    assert_eq!(undo_delete(&dir, &a, T), PutBackHow::Written { trash: Some(Untrashed::NotFound) });
    assert_eq!(std::fs::read_to_string(dir.join("u.txt")).unwrap(), "kept\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_staged_new_file_comes_back_out_of_the_trash_staged() {
    let dir = base("ufa-untrash-staged");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    write(&dir, "sub/n.txt", "new\n");
    git(&dir, &["add", "sub/n.txt"]);
    let staged = index_entry(&dir, "sub/n.txt");
    let a = delete_staged_saving(&dir, "sub/n.txt", DeleteVia::Trash, T).unwrap().undo.unwrap();
    assert!(!dir.join("sub").exists(), "the emptied folder was pruned");
    let how = undo_delete(&dir, &a, T);
    assert_eq!(how, PutBackHow::FromTrash);
    assert_eq!(
        how.wrote("sub/n.txt", true, DeleteVia::Trash),
        format!("Took sub/n.txt back out of the {} and staged it again", repo::TRASH_NAME)
    );
    assert_eq!(std::fs::read_to_string(dir.join("sub/n.txt")).unwrap(), "new\n");
    assert_eq!(index_entry(&dir, "sub/n.txt"), staged);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_that_came_back_from_the_trash_with_other_bytes_is_left_alone() {
    let dir = base("ufa-untrash-other");
    write(&dir, "u.txt", "kept\n");
    let a = delete_saving(&dir, "u.txt", DeleteVia::Trash, T).unwrap().undo.unwrap();
    let Action::DeleteFile { before, .. } = &a else { unreachable!() };
    // A trash that puts something else there (another program won the race).
    fn other(full: &Path, _: &dyn Fn(&Path) -> bool) -> Untrashed {
        std::fs::write(full, "someone else's\n").unwrap();
        Untrashed::Restored
    }
    let e = repo::untrash_file(&dir, "u.txt", before.blob, None, other).unwrap_err();
    assert!(e.to_string().contains(&format!("git cat-file -p {}", before.blob)), "{e}");
    assert_eq!(std::fs::read_to_string(dir.join("u.txt")).unwrap(), "someone else's\n", "left as it is");
    let _ = std::fs::remove_dir_all(&dir);
}
