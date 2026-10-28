//! Undoing and redoing Delete and Unstage and delete against real repositories, with a stand-in
//! trash folder: the exact bytes come back (out of the trash when they went there), nothing at
//! the path is ever overwritten, a staged new file or staged rename is staged again, and redo
//! deletes the way the first delete went.

use super::test_bin::*;
use super::*;
use crate::repo::{self, Deleted};
use crate::testutil::*;
use std::path::{Path, PathBuf};

fn base(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "f.txt", "one\ntwo\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    dir
}

fn undo(dir: &Path, a: &Action) {
    match plan(dir, a, false).unwrap() {
        Plan::PutBack { path, content, staged, via } => {
            put_back(dir, &path, content, staged.as_ref(), via, T).unwrap();
        }
        other => panic!("expected a put-back, got {other:?}"),
    }
}

fn redo(dir: &Path, a: &Action) -> Deleted {
    match plan(dir, a, true).unwrap() {
        Plan::DeleteAgain { path, via, staged } => delete_again(dir, &path, via, staged, T).unwrap(),
        other => panic!("expected a delete, got {other:?}"),
    }
}

fn refusal(p: Plan) -> Refusal {
    match p {
        Plan::Refuse(r) => r,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_deleted_file_comes_back_byte_for_byte_and_goes_again() {
    let dir = base("udel-bytes");
    let edited: &[u8] = b"one\r\ntwo changed\r\n\xff not utf-8";
    std::fs::write(dir.join("f.txt"), edited).unwrap();
    let before = in_bin("f.txt");
    let d = delete_saving(&dir, "f.txt", DeleteVia::Trash, T).unwrap();
    assert_eq!((d.how, d.too_large), (Deleted::Trashed, false));
    let a = d.undo.expect("undoable");
    assert!(matches!(&a, Action::DeleteFile { path, via: DeleteVia::Trash, staged: None, .. } if path == "f.txt"), "{a:?}");
    assert_eq!(a.describe(), "delete of f.txt");
    assert!(!dir.join("f.txt").exists());
    assert_eq!(in_bin("f.txt"), before + 1);

    undo(&dir, &a);
    assert_eq!(std::fs::read(dir.join("f.txt")).unwrap(), edited);
    assert_eq!(in_bin("f.txt"), before, "taken back out of the trash");

    assert_eq!(redo(&dir, &a), Deleted::Trashed);
    assert!(!dir.join("f.txt").exists());
    assert_eq!(in_bin("f.txt"), before + 1, "redo used the trash again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_permanent_delete_comes_back_and_redo_removes_it_for_good() {
    let dir = base("udel-permanent");
    std::fs::create_dir_all(dir.join("sub/deep")).unwrap();
    std::fs::write(dir.join("sub/deep/u.txt"), "untracked\n").unwrap();
    let a = delete_saving(&dir, "sub/deep/u.txt", DeleteVia::Permanent, T).unwrap().undo.unwrap();
    assert!(!dir.join("sub").exists(), "the emptied folders were pruned");
    undo(&dir, &a);
    assert_eq!(std::fs::read_to_string(dir.join("sub/deep/u.txt")).unwrap(), "untracked\n");
    assert_eq!(a.wrote(false), "Put back sub/deep/u.txt");
    assert_eq!(redo(&dir, &a), Deleted::Permanently);
    assert!(!dir.join("sub").exists());
    assert_eq!(a.wrote(true), "Deleted sub/deep/u.txt again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_never_overwrites_and_redo_never_deletes_edits() {
    let dir = base("udel-refused");
    let a = delete_saving(&dir, "f.txt", DeleteVia::Permanent, T).unwrap().undo.unwrap();
    let Action::DeleteFile { before, .. } = &a else { unreachable!() };
    write(&dir, "f.txt", "someone else's\n");
    let r = refusal(plan(&dir, &a, false).unwrap());
    assert!(matches!(r, Refusal::Unsafe(_)), "kept, not stale: {r:?}");
    assert!(r.message().contains(&format!("git cat-file -p {}", before.blob)), "{}", r.message());
    assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap(), "someone else's\n");

    std::fs::remove_file(dir.join("f.txt")).unwrap();
    undo(&dir, &a);
    write(&dir, "f.txt", "edited after the undo\n");
    let r = refusal(plan(&dir, &a, true).unwrap());
    assert!(r.is_stale(), "{r:?}");
    assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap(), "edited after the undo\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn links_and_large_files_get_no_undo() {
    let dir = base("udel-nokeep");
    write(&dir, "ten.txt", "0123456789");
    let objects = || {
        git2::Repository::open(&dir).unwrap().odb().unwrap().exists(git2::Oid::hash_object(git2::ObjectType::Blob, b"0123456789").unwrap())
    };
    let d = super::deleted::delete_saving_with(&dir, "ten.txt", DeleteVia::Permanent, T, 4).unwrap();
    assert!(d.too_large && d.undo.is_none(), "{d:?}");
    assert!(!objects(), "no blob was written");

    let target = dir.join("f.txt");
    let link = dir.join("link.txt");
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(&target, &link).is_ok();
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_file(&target, &link).is_ok();
    if !made {
        eprintln!("skipped the link case: symbolic links are unavailable here");
        return;
    }
    let d = delete_saving(&dir, "link.txt", DeleteVia::Trash, T).unwrap();
    assert_eq!((d.how, d.undo.is_none(), d.too_large), (Deleted::LinkRemoved, true, false));
    assert!(target.exists(), "the target is untouched");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_and_delete_of_a_new_file_is_staged_again() {
    let dir = base("udel-staged");
    write(&dir, "n.txt", "new\n");
    git(&dir, &["add", "n.txt"]);
    let blob = |d: &Path| git2::Repository::open(d).unwrap().index().unwrap().get_path(Path::new("n.txt"), 0).map(|e| (e.id, e.mode));
    let staged_before = blob(&dir);
    let a = delete_staged_saving(&dir, "n.txt", DeleteVia::Trash, T).unwrap().undo.unwrap();
    assert_eq!(blob(&dir), None);

    undo(&dir, &a);
    assert_eq!(blob(&dir), staged_before, "the same blob and mode at stage 0");
    assert_eq!(std::fs::read_to_string(dir.join("n.txt")).unwrap(), "new\n");
    assert_eq!(a.wrote(false), "Put back n.txt and staged it again");

    assert_eq!(redo(&dir, &a), Deleted::Trashed);
    assert_eq!(blob(&dir), None, "redo unstaged it again");
    assert!(!dir.join("n.txt").exists());

    // Staged again meanwhile: the undo would lose that.
    write(&dir, "n.txt", "other\n");
    git(&dir, &["add", "n.txt"]);
    std::fs::remove_file(dir.join("n.txt")).unwrap();
    assert!(refusal(plan(&dir, &a, false).unwrap()).is_stale());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_and_delete_of_a_rename_brings_the_rename_back() {
    let dir = scratch_repo("udel-rename");
    let text = "one\ntwo\nthree\nfour\nfive\n";
    write(&dir, "old.txt", text);
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["mv", "old.txt", "new.txt"]);
    let a = delete_staged_saving(&dir, "new.txt", DeleteVia::Permanent, T).unwrap().undo.unwrap();
    undo(&dir, &a);
    let (_, staged, _) = repo::status_at(&dir, true).unwrap();
    let staged: Vec<_> = staged.into_iter().map(|f| (f.path, f.status)).collect();
    assert_eq!(staged, [("new.txt".to_owned(), 'R')], "the rename pairs again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_staged_undo_waits_for_an_operation_in_progress() {
    let dir = base("udel-busy");
    write(&dir, "n.txt", "new\n");
    git(&dir, &["add", "n.txt"]);
    let staged = delete_staged_saving(&dir, "n.txt", DeleteVia::Permanent, T).unwrap().undo.unwrap();
    write(&dir, "p.txt", "plain\n");
    let plain = delete_saving(&dir, "p.txt", DeleteVia::Permanent, T).unwrap().undo.unwrap();
    // A merge in progress, as git leaves it.
    let head = git2::Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
    std::fs::write(dir.join(".git/MERGE_HEAD"), format!("{head}\n")).unwrap();
    assert!(matches!(refusal(plan(&dir, &staged, false).unwrap()), Refusal::Busy(_)));
    assert!(matches!(plan(&dir, &plain, false).unwrap(), Plan::PutBack { .. }), "a plain delete's undo ignores it");
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn an_executable_file_comes_back_executable() {
    use std::os::unix::fs::PermissionsExt;
    let dir = base("udel-exec");
    set_config(&dir, &[("core.fileMode", "true")]);
    let script = dir.join("run.sh");
    std::fs::write(&script, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let a = delete_saving(&dir, "run.sh", DeleteVia::Permanent, T).unwrap().undo.unwrap();
    undo(&dir, &a);
    let mode = std::fs::metadata(&script).unwrap().permissions().mode();
    assert_ne!(mode & 0o100, 0, "{mode:o}");
    let _ = std::fs::remove_dir_all(&dir);
}
