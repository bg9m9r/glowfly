//! Deleting to the trash against real repositories, with stand-in trashes: the file goes to the
//! trash, a refusal changes nothing (working tree, folders or index), links never reach the trash,
//! and the real trash is never used by these tests.

use super::workfile_tests::{commit, index_id, status, text};
use super::*;
use crate::error::Error;
use crate::testutil::*;
use std::path::{Path, PathBuf};

pub(super) fn test_bin() -> PathBuf {
    crate::paths::temp_dir().join(format!("trellis-test-bin-{}", std::process::id()))
}

/// Moves the file into this process's stand-in bin under a unique name.
pub(super) fn fake_bin(full: &Path) -> Result<(), TrashRefusal> {
    let other = |e: std::io::Error| TrashRefusal::new(TrashProblem::Other, e.to_string());
    let bin = test_bin();
    std::fs::create_dir_all(&bin).map_err(other)?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let name = full.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    std::fs::rename(full, bin.join(format!("{nanos}-{name}"))).map_err(other)
}

pub(super) fn no_bin(_: &Path) -> Result<(), TrashRefusal> {
    Err(TrashRefusal::new(TrashProblem::NoTrash, "no bin here"))
}

fn removes_then_errors(full: &Path) -> Result<(), TrashRefusal> {
    std::fs::remove_file(full).map_err(|e| TrashRefusal::new(TrashProblem::Other, e.to_string()))?;
    Err(TrashRefusal::new(TrashProblem::Other, "lost track"))
}

pub(super) fn never_called(full: &Path) -> Result<(), TrashRefusal> {
    panic!("the trash was asked to take {}", full.display())
}

/// Whether the stand-in bin holds a file named `name` with `contents`.
pub(super) fn in_bin(name: &str, contents: &str) -> bool {
    std::fs::read_dir(test_bin()).into_iter().flatten().flatten().any(|e| {
        e.file_name().to_string_lossy().ends_with(&format!("-{name}")) && std::fs::read_to_string(e.path()).is_ok_and(|t| t == contents)
    })
}

#[test]
fn trash_mode_hands_the_file_to_the_trasher() {
    let dir = scratch_repo("rm-trash");
    commit(&dir, &[("keep.txt", "k\n"), ("sub/deep/a.txt", "trash me rm-trash\n")], "one");
    let how = delete_file(&dir, "sub/deep/a.txt", Removal::Trash(fake_bin)).unwrap();
    assert_eq!(how, Deleted::Trashed);
    assert!(!dir.join("sub/deep/a.txt").exists());
    assert!(in_bin("a.txt", "trash me rm-trash\n"));
    assert!(!dir.join("sub").exists(), "emptied folders are pruned");
    assert_eq!(status(&dir), " D sub/deep/a.txt");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn trash_refusal_changes_nothing() {
    let dir = scratch_repo("rm-refuse");
    commit(&dir, &[("sub/deep/a.txt", "a\n")], "one");
    let before = status(&dir);
    match delete_file(&dir, "sub/deep/a.txt", Removal::Trash(no_bin)) {
        Err(Error::TrashRefused(r)) => assert_eq!(r, TrashRefusal::new(TrashProblem::NoTrash, "no bin here")),
        other => panic!("expected TrashRefused, got {other:?}"),
    }
    assert_eq!(text(&dir, "sub/deep/a.txt"), "a\n");
    assert!(dir.join("sub/deep").is_dir());
    assert_eq!(status(&dir), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn trasher_that_removes_then_errors() {
    let dir = scratch_repo("rm-partial");
    commit(&dir, &[("keep.txt", "k\n"), ("sub/deep/a.txt", "a\n")], "one");
    match delete_file(&dir, "sub/deep/a.txt", Removal::Trash(removes_then_errors)) {
        Err(Error::Other(m)) => {
            assert!(m.contains("sub/deep/a.txt") && m.contains("lost track"), "{m}");
        }
        other => panic!("expected Other, got {other:?}"),
    }
    assert!(!dir.join("sub").exists(), "emptied folders are still pruned");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn links_skip_the_trash() {
    let dir = scratch_repo("rm-link");
    commit(&dir, &[("target.txt", "t\n")], "one");
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(dir.join("target.txt"), dir.join("link")).is_ok();
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_file(dir.join("target.txt"), dir.join("link")).is_ok();
    if made {
        assert_eq!(delete_file(&dir, "link", Removal::Trash(never_called)).unwrap(), Deleted::LinkRemoved);
        assert!(dir.join("link").symlink_metadata().is_err());
        assert_eq!(text(&dir, "target.txt"), "t\n");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staged_new_file_to_trash() {
    let dir = scratch_repo("rm-staged-new");
    commit(&dir, &[("a.txt", "a\n")], "one");
    write(&dir, "n.txt", "new rm-staged-new\n");
    git(&dir, &["add", "n.txt"]);
    let before = status(&dir);

    match delete_staged_new_file(&dir, "n.txt", Removal::Trash(no_bin)) {
        Err(Error::TrashRefused(_)) => {}
        other => panic!("expected TrashRefused, got {other:?}"),
    }
    assert!(index_id(&dir, "n.txt").is_some(), "still staged");
    assert_eq!(text(&dir, "n.txt"), "new rm-staged-new\n");
    assert_eq!(status(&dir), before);

    assert_eq!(delete_staged_new_file(&dir, "n.txt", Removal::Trash(fake_bin)).unwrap(), Deleted::Trashed);
    assert_eq!(index_id(&dir, "n.txt"), None);
    assert!(in_bin("n.txt", "new rm-staged-new\n"));
    assert_eq!(status(&dir), "");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staged_rename_refusal_keeps_rename() {
    let dir = scratch_repo("rm-rename");
    commit(&dir, &[("old.txt", "one\ntwo\nthree\n")], "one");
    git(&dir, &["mv", "old.txt", "new.txt"]);
    let before = status(&dir);
    assert!(before.starts_with('R'), "{before}");

    assert!(matches!(delete_staged_new_file(&dir, "new.txt", Removal::Trash(no_bin)), Err(Error::TrashRefused(_))));
    assert_eq!(status(&dir), before, "the rename is still staged");
    assert_eq!(index_id(&dir, "old.txt"), None, "the source isn't back in the index");
    assert_eq!(text(&dir, "new.txt"), "one\ntwo\nthree\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn os_trash_refuses_under_test() {
    if std::env::var_os("TRELLIS_REAL_TRASH").is_some() {
        return;
    }
    let dir = scratch_repo("rm-os-guard");
    write(&dir, "a.txt", "a\n");
    assert!(os_trash(&dir.join("a.txt")).is_err());
    assert!(dir.join("a.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Moves a file to the real Recycle Bin / Trash: check it arrived there by hand. Run with
/// `TRELLIS_REAL_TRASH=1 cargo test -p glowfly-core real_os_trash -- --ignored --nocapture`.
#[test]
#[ignore = "uses the real trash; run by hand with TRELLIS_REAL_TRASH=1"]
fn real_os_trash() {
    let dir = scratch_repo("rm-os-real");
    write(&dir, "trellis-real-trash-check.txt", "check\n");
    os_trash(&dir.join("trellis-real-trash-check.txt")).unwrap();
    assert!(!dir.join("trellis-real-trash-check.txt").exists());
    println!("look for trellis-real-trash-check.txt in the trash");
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(windows)]
#[test]
fn volume_roots() {
    use super::trash::volume_root_of;
    let root = |p: &str| volume_root_of(Path::new(p)).map(|r| r.to_string_lossy().into_owned());
    assert_eq!(root(r"C:\repo\a.txt").as_deref(), Some(r"C:\"));
    assert_eq!(root("D:/repo/sub/a.txt").as_deref(), Some(r"D:\"));
    assert_eq!(root(r"\\server\share\repo\a.txt").as_deref(), Some(r"\\server\share\"));
    assert_eq!(root(r"\\?\C:\repo\a.txt").as_deref(), Some(r"C:\"));
    assert_eq!(root(r"\\?\UNC\server\share\repo\a.txt").as_deref(), Some(r"\\server\share\"));
    assert_eq!(root(r"repo\a.txt"), None);
}
