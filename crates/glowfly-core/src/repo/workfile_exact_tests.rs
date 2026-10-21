//! Renaming and deleting act on exactly the file that was picked: an existing path is never
//! trimmed or rewritten, and a move never replaces a file already at the new name.

use super::noclobber::rename_noclobber;
use super::*;
use crate::testutil::*;
use std::io::ErrorKind;
use std::path::Path;

fn text(dir: &Path, path: &str) -> String {
    std::fs::read_to_string(dir.join(path)).unwrap()
}

/// " notes.txt" (a legal name everywhere) next to "notes.txt".
fn spaced_repo(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    write(&dir, " notes.txt", "spaced\n");
    write(&dir, "notes.txt", "plain\n");
    dir
}

#[test]
fn delete_removes_the_listed_name_not_a_trimmed_one() {
    let dir = spaced_repo("wf-exact-delete");
    assert_eq!(delete_file(&dir, " notes.txt", Removal::Permanent).unwrap(), Deleted::Permanently);
    assert!(dir.join(" notes.txt").symlink_metadata().is_err());
    assert_eq!(text(&dir, "notes.txt"), "plain\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rename_moves_the_listed_name_not_a_trimmed_one() {
    let dir = spaced_repo("wf-exact-rename");
    rename_file(&dir, " notes.txt", "moved.txt").unwrap();
    assert_eq!(text(&dir, "moved.txt"), "spaced\n");
    assert_eq!(text(&dir, "notes.txt"), "plain\n");
    assert!(dir.join(" notes.txt").symlink_metadata().is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn malformed_existing_paths_are_refused() {
    let dir = spaced_repo("wf-exact-malformed");
    for p in ["", "/notes.txt", "./notes.txt", "a//notes.txt", "notes.txt/", "../notes.txt", ".git/config"] {
        assert!(delete_file(&dir, p, Removal::Permanent).is_err(), "{p:?}");
        assert!(rename_file(&dir, p, "moved.txt").is_err(), "{p:?}");
    }
    assert_eq!(text(&dir, "notes.txt"), "plain\n");
    assert!(dir.join(".git/config").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A backslash is part of a file name outside Windows, not a separator.
#[cfg(unix)]
#[test]
fn backslash_names_the_file_it_spells() {
    let dir = scratch_repo("wf-exact-backslash");
    std::fs::create_dir_all(dir.join("a")).unwrap();
    write(&dir, "a/b", "nested\n");
    write(&dir, "a\\b", "flat\n");
    write(&dir, "c\\d", "other\n");
    assert_eq!(delete_file(&dir, "a\\b", Removal::Permanent).unwrap(), Deleted::Permanently);
    assert!(dir.join("a\\b").symlink_metadata().is_err());
    rename_file(&dir, "c\\d", "e.txt").unwrap();
    assert_eq!(text(&dir, "e.txt"), "other\n");
    assert_eq!(text(&dir, "a/b"), "nested\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Names Windows would resolve to another file are refused rather than acted on.
#[cfg(windows)]
#[test]
fn names_windows_would_rewrite_are_refused() {
    let dir = spaced_repo("wf-exact-windows");
    std::fs::create_dir_all(dir.join("a")).unwrap();
    write(&dir, "a/b", "nested\n");
    for p in ["a\\b", "notes.txt ", "notes.txt.", "notes.txt:x", "C:notes.txt"] {
        assert!(delete_file(&dir, p, Removal::Permanent).is_err(), "{p:?}");
        assert!(rename_file(&dir, p, "moved.txt").is_err(), "{p:?}");
    }
    assert_eq!(text(&dir, "a/b"), "nested\n");
    assert_eq!(text(&dir, "notes.txt"), "plain\n");
    assert!(!dir.join("moved.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rename_noclobber_refuses_an_existing_target() {
    // No check beforehand: the move itself must refuse, so a file that appears at the last
    // moment is never replaced.
    let dir = scratch_repo("wf-exact-noclobber");
    write(&dir, "a.txt", "a\n");
    write(&dir, "b.txt", "b\n");
    let e = rename_noclobber(&dir.join("a.txt"), &dir.join("b.txt")).unwrap_err();
    assert_eq!(e.kind(), ErrorKind::AlreadyExists, "{e}");
    assert_eq!((text(&dir, "a.txt"), text(&dir, "b.txt")), ("a\n".into(), "b\n".into()));
    rename_noclobber(&dir.join("a.txt"), &dir.join("c.txt")).unwrap();
    assert_eq!(text(&dir, "c.txt"), "a\n");
    assert!(!dir.join("a.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rename_noclobber_handles_long_paths() {
    let dir = scratch_repo("wf-exact-long");
    let deep = (0..12).map(|i| format!("folder-number-{i:02}-with-a-long-name")).collect::<Vec<_>>().join("/");
    std::fs::create_dir_all(dir.join(&deep)).unwrap();
    let (from, to) = (dir.join(format!("{deep}/a.txt")), dir.join(format!("{deep}/b.txt")));
    assert!(to.as_os_str().len() > 260);
    std::fs::write(&from, "a\n").unwrap();
    rename_noclobber(&from, &to).unwrap();
    assert_eq!(std::fs::read_to_string(&to).unwrap(), "a\n");
    let _ = std::fs::remove_dir_all(&dir);
}
