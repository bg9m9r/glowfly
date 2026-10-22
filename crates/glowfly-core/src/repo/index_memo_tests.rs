//! Which index changes the memo sees: one fixture per kind of change.

use super::*;
use crate::testutil::{git, scratch_repo, set_config, write};
use std::path::Path;

fn memo(dir: &Path) -> IndexMemo {
    let repo = git2::Repository::open(dir).unwrap();
    IndexMemo::of(&repo.index().unwrap()).expect("small index")
}

/// A repository with a.txt and b.txt committed.
fn two_files(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "one\n");
    write(&dir, "b.txt", "two\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    dir
}

/// The paths `change` makes the memo report, after it ran in `dir`.
fn delta_of(dir: &Path, change: impl FnOnce(&Path)) -> IndexDelta {
    let before = memo(dir);
    change(dir);
    before.changed(&memo(dir))
}

fn paths(d: &IndexDelta) -> Vec<&str> {
    d.paths.iter().map(String::as_str).collect()
}

#[test]
fn entry_changes_name_their_paths() {
    let dir = two_files("memo-entries");
    assert_eq!(memo(&dir).len(), 2);
    let d = delta_of(&dir, |d| {
        write(d, "a.txt", "edited\n");
        git(d, &["add", "a.txt"]);
    });
    assert_eq!(d, IndexDelta { paths: vec!["a.txt".into()], conflicted: false, not_utf8: false });

    let d = delta_of(&dir, |d| git(d, &["rm", "-q", "--cached", "a.txt"]));
    assert_eq!(paths(&d), ["a.txt"], "removed");
    let d = delta_of(&dir, |d| git(d, &["add", "a.txt"]));
    assert_eq!(paths(&d), ["a.txt"], "added back");
    let d = delta_of(&dir, |d| git(d, &["update-index", "--chmod=+x", "a.txt"]));
    assert_eq!(paths(&d), ["a.txt"], "the mode");
    let d = delta_of(&dir, |d| git(d, &["update-index", "--skip-worktree", "b.txt"]));
    assert_eq!(paths(&d), ["b.txt"], "skip-worktree");
    let d = delta_of(&dir, |d| {
        write(d, "new.txt", "new\n");
        git(d, &["add", "-N", "new.txt"]);
    });
    assert_eq!(paths(&d), ["new.txt"], "intent to add");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stat_only_refresh_changes_nothing() {
    let dir = two_files("memo-stat");
    let d = delta_of(&dir, |d| {
        // The same content written again: a new modification time, the same blob.
        write(d, "a.txt", "one\n");
        git(d, &["update-index", "--refresh"]);
        git(d, &["status", "--porcelain"]);
    });
    assert_eq!(d, IndexDelta::default());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_conflict_is_flagged() {
    let dir = two_files("memo-conflict");
    git(&dir, &["checkout", "-q", "-b", "other"]);
    write(&dir, "a.txt", "theirs\n");
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, "a.txt", "ours\n");
    git(&dir, &["commit", "-q", "-am", "ours"]);
    let d = delta_of(&dir, |d| {
        let (ok, _) = crate::gitcli::run(d, &crate::gitcli::args(&["merge", "-q", "other"]));
        assert!(!ok, "the merge conflicts");
    });
    assert!(d.conflicted && paths(&d) == ["a.txt"], "{d:?}");
    let d = delta_of(&dir, |d| git(d, &["add", "a.txt"]));
    assert!(d.conflicted, "resolving drops the conflict stages: {d:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// macOS file systems refuse names that aren't UTF-8.
#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn a_name_that_is_not_utf8_is_flagged() {
    use std::os::unix::ffi::OsStrExt;
    let dir = two_files("memo-bytes");
    let name = std::ffi::OsStr::from_bytes(b"caf\xe9.txt");
    if std::fs::write(dir.join(name), "x\n").is_err() {
        return; // the file system refuses the name
    }
    let d = delta_of(&dir, |d| git(d, &["add", "."]));
    assert!(d.not_utf8 && d.paths.is_empty(), "{d:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn folded_order_reads_the_same_twice() {
    let dir = scratch_repo("memo-case");
    set_config(&dir, &[("core.ignorecase", "true")]);
    write(&dir, "B.txt", "b\n");
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "."]);
    let (first, second) = (memo(&dir), memo(&dir));
    assert_eq!(first.len(), 2);
    assert_eq!(first.changed(&second), IndexDelta::default());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_large_index_gives_no_memo() {
    let dir = two_files("memo-cap");
    let repo = git2::Repository::open(&dir).unwrap();
    let index = repo.index().unwrap();
    assert!(IndexMemo::of_with(&index, 1).is_none());
    assert_eq!(IndexMemo::of_with(&index, 2).map(|m| m.len()), Some(2));
    assert_eq!(format!("{:?}", memo(&dir)), "IndexMemo(2 entries)");
    let _ = std::fs::remove_dir_all(&dir);
}
