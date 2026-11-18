//! libgit2's lock failures on fixture repositories with a stale `index.lock` (stage, unstage and
//! discard through libgit2, staging through git, a linked worktree), and the wordings read from
//! errors made up here (their paths are only parsed, never touched).

use super::{classify_git2, file_name, lock_named, lock_path_git2, locked_write};
use crate::error::{Error, WriteFailed};
use crate::git_errors::Cause;
use crate::git_procs::GitProcesses;
use crate::lock_file::{Found, check_with};
use crate::paths::resolve;
use crate::repo::{self, DiffTarget, HunkOp};
use crate::testutil::{git, scratch_repo, write};
use git2::{ErrorClass, ErrorCode};
use std::path::{Path, PathBuf};

/// A repository with a.txt committed and then changed, not staged.
fn changed(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "a.txt"]);
    git(&dir, &["commit", "-m", "a"]);
    write(&dir, "a.txt", "a\nb\n");
    dir
}

/// Leaves a stale lock beside `dir`'s index and returns it.
fn lock_index(dir: &Path) -> PathBuf {
    let lock = git2::Repository::open(dir).unwrap().path().join("index.lock");
    std::fs::write(&lock, "").unwrap();
    lock
}

fn unstaged_hunk(dir: &Path) -> repo::Hunk {
    repo::load_diff(dir, &DiffTarget::Unstaged("a.txt".into())).unwrap().hunks.remove(0)
}

/// `err`, which must be libgit2's own lock failure naming index.lock.
fn index_locked(err: &Error) -> &git2::Error {
    let Error::Git(e) = err else { panic!("not libgit2's error: {err:?}") };
    assert_eq!(e.code(), ErrorCode::Locked, "{e}");
    assert_eq!(classify_git2(e), Some(Cause::Locked { file: "index.lock".into() }));
    e
}

#[test]
fn a_libgit2_stage_names_the_index_lock_that_may_be_deleted() {
    let dir = changed("lock-errors-stage");
    let hunk = unstaged_hunk(&dir);
    let lock = lock_index(&dir);
    let err = repo::apply_hunk(&dir, "a.txt", 0, &hunk, HunkOp::Stage).unwrap_err();
    index_locked(&err);
    let named = lock_named(&dir, &err).expect("a lock path");
    assert_eq!(resolve(Path::new(&named)), resolve(&lock));
    assert!(matches!(check_with(&dir, &named, || GitProcesses::None), Ok(Found::Stale(_))));
    assert!(matches!(locked_write(&dir, err), WriteFailed::Locked { said, .. } if said.starts_with("the index is locked")));
    std::fs::remove_file(&lock).unwrap();
    repo::apply_hunk(&dir, "a.txt", 0, &hunk, HunkOp::Stage).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstaging_and_a_libgit2_discard_hit_the_same_lock() {
    let dir = changed("lock-errors-unstage");
    git(&dir, &["add", "a.txt"]);
    write(&dir, "a.txt", "a\nb\nc\n");
    let hunk = unstaged_hunk(&dir);
    let lock = lock_index(&dir);
    let err = repo::unstage_paths(&dir, &["a.txt".into()]).unwrap_err();
    index_locked(&err);
    assert_eq!(resolve(Path::new(&lock_named(&dir, &err).unwrap())), resolve(&lock));
    let err = repo::apply_hunk(&dir, "a.txt", 0, &hunk, HunkOp::Discard).unwrap_err();
    index_locked(&err);
    assert_eq!(resolve(Path::new(&lock_named(&dir, &err).unwrap())), resolve(&lock));
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "a\nb\nc\n", "nothing was discarded");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staging_lines_and_a_whitespace_hidden_hunk_hit_the_same_lock() {
    let dir = changed("lock-errors-lines");
    let hunk = unstaged_hunk(&dir);
    let added = hunk.lines.iter().position(|l| l.origin == '+').expect("the added line");
    let lock = lock_index(&dir);
    let by_lines = repo::apply_lines(&dir, "a.txt", 0, &hunk, &[added], HunkOp::Stage).unwrap_err();
    index_locked(&by_lines);
    let shown = repo::apply_shown(&dir, "a.txt", 0, &hunk, None, HunkOp::Stage).unwrap_err();
    index_locked(&shown);
    for err in [&by_lines, &shown] {
        assert_eq!(resolve(Path::new(&lock_named(&dir, err).unwrap())), resolve(&lock));
    }
    std::fs::remove_file(&lock).unwrap();
    let staged = repo::load_diff(&dir, &DiffTarget::Staged("a.txt".into()));
    assert!(staged.map_or(true, |d| d.hunks.is_empty()), "nothing was staged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_git_stage_names_the_lock_git_printed() {
    let dir = changed("lock-errors-git");
    lock_index(&dir);
    let err = repo::stage_file(&dir, "a.txt").unwrap_err();
    let Error::Other(text) = &err else { panic!("not git's output: {err:?}") };
    let from_git = crate::git_errors::lock_path(text);
    assert!(from_git.is_some(), "{text}");
    assert_eq!(lock_named(&dir, &err), from_git);
    match locked_write(&dir, err) {
        WriteFailed::Locked { lock, said } => {
            assert_eq!(Some(lock), from_git);
            assert!(said.contains("File exists"), "{said}");
        }
        other => panic!("not Locked: {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_linked_worktree_names_its_own_index_lock() {
    let dir = changed("lock-errors-worktree");
    git(&dir, &["commit", "-am", "b"]);
    let wt = dir.with_file_name(format!("{}-wt", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&wt);
    git(&dir, &["worktree", "add", "-b", "side", &wt.to_string_lossy()]);
    write(&wt, "a.txt", "w\n");
    git(&wt, &["add", "a.txt"]);
    let lock = lock_index(&wt);
    assert!(!dir.join(".git/index.lock").exists());
    let err = repo::unstage_paths(&wt, &["a.txt".into()]).unwrap_err();
    index_locked(&err);
    let named = lock_named(&wt, &err).unwrap();
    assert_eq!(resolve(Path::new(&named)), resolve(&lock));
    assert!(resolve(&lock).starts_with(resolve(&dir.join(".git/worktrees"))), "{}", lock.display());
    assert!(matches!(check_with(&wt, &named, || GitProcesses::None), Ok(Found::Stale(_))));
    let _ = std::fs::remove_dir_all(&wt);
    let _ = std::fs::remove_dir_all(&dir);
}

fn made(code: ErrorCode, class: ErrorClass, msg: &str) -> git2::Error {
    git2::Error::new(code, class, msg)
}

#[test]
fn only_a_nameable_lock_file_is_a_lock() {
    let locked = |class, msg| classify_git2(&made(ErrorCode::Locked, class, msg));
    let file = |f: &str| Some(Cause::Locked { file: f.into() });
    assert_eq!(locked(ErrorClass::Os, "failed to lock file '/r/.git/refs/heads/main.lock' for writing"), file("main.lock"));
    assert_eq!(locked(ErrorClass::Os, "failed to create locked file '/r/.git/index.lock'"), file("index.lock"));
    assert_eq!(locked(ErrorClass::Index, "the index is locked; this might be due to a concurrent or crashed process"), file("index.lock"));
    // Without code Locked it is another failure (a missing folder, say).
    let msg = "failed to create locked file '/r/.git/index.lock'";
    assert_eq!(classify_git2(&made(ErrorCode::GenericError, ErrorClass::Os, msg)), None);
    assert_eq!(classify_git2(&made(ErrorCode::NotFound, ErrorClass::Os, msg)), None);
    // A file another program holds open, and a locked worktree: no lock file to delete.
    assert_eq!(locked(ErrorClass::Os, "failed to unlink - '/r/a.txt' is locked"), None);
    assert_eq!(locked(ErrorClass::Worktree, "working tree '/r/wt' is locked"), None);
    // The index-locked wording counts only from the index.
    assert_eq!(locked(ErrorClass::Os, "the index is locked; this might be due to a concurrent or crashed process"), None);
    // A lock file's name only when it ends in .lock.
    assert_eq!(locked(ErrorClass::Os, "failed to lock file '/r/.git/config' for writing"), None);
}

#[test]
fn a_quoted_path_holding_a_quote_is_read_whole() {
    let dir = Path::new("/nowhere");
    let e = made(ErrorCode::Locked, ErrorClass::Os, "failed to lock file '/r/it's/.git/index.lock' for writing");
    assert_eq!(lock_path_git2(dir, &e).as_deref(), Some("/r/it's/.git/index.lock"));
    let e = made(ErrorCode::Locked, ErrorClass::Os, "failed to create locked file '/r/it's/.git/HEAD.lock'");
    assert_eq!(lock_path_git2(dir, &e).as_deref(), Some("/r/it's/.git/HEAD.lock"));
    assert_eq!(classify_git2(&e), Some(Cause::Locked { file: "HEAD.lock".into() }));
}

#[test]
fn other_errors_name_no_lock() {
    let dir = Path::new("/nowhere");
    let io = Error::Io(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "index.lock"));
    assert_eq!(lock_named(dir, &io), None);
    assert!(matches!(locked_write(dir, io), WriteFailed::Said { .. }));
    let other = Error::Other("could not write the merged file".into());
    assert_eq!(lock_named(dir, &other), None);
    assert!(matches!(locked_write(dir, other), WriteFailed::Other { .. }));
    assert_eq!(file_name("C:\\r\\.git\\index.lock"), "index.lock");
    assert_eq!(file_name("/r/.git/refs/heads/main.lock"), "main.lock");
}
