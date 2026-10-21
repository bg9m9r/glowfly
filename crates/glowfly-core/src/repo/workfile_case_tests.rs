//! Renames that only change case or Unicode form, parent folders spelled differently, the
//! canonicalize fallback and the warnings for new names. Each rule runs on every OS against a
//! stand-in for a case- and normalization-insensitive file system; the same renames also run
//! for real where the test folder ignores case (Windows and macOS by default).

use super::workfile::create_file;
use super::workfile_disk::{Disk, ensure_inside_on, tracked_twin};
use super::workfile_mv::rename_file_staged_on;
use super::workfile_rename::rename_file_on;
use super::workfile_tests::{commit, names, text};
use crate::names::fold;
use crate::testutil::*;
use git2::Repository;
use std::path::{Path, PathBuf};

/// Whether the parent folder lists an entry that `p`'s name folds to, as a case- and
/// normalization-insensitive file system would find it.
fn folded_exists(p: &Path) -> bool {
    let (Some(dir), Some(name)) = (p.parent(), p.file_name().and_then(|n| n.to_str())) else { return false };
    std::fs::read_dir(dir).is_ok_and(|d| d.flatten().any(|e| e.file_name().to_str().is_some_and(|n| fold(n) == fold(name))))
}

fn folded_same(a: &Path, b: &Path) -> bool {
    folded_exists(a)
        && folded_exists(b)
        && a.parent() == b.parent()
        && a.file_name().map(|n| fold(&n.to_string_lossy())) == b.file_name().map(|n| fold(&n.to_string_lossy()))
}

/// A case- and normalization-insensitive file system, on any OS.
const FOLDING: Disk = Disk { exists: folded_exists, same_file: folded_same, canonicalize: canonical };

fn canonical(p: &Path) -> std::io::Result<PathBuf> {
    p.canonicalize()
}

fn real() -> Disk {
    super::workfile_disk::DISK
}

/// Whether `dir`'s file system ignores case.
fn folds_case(dir: &Path) -> bool {
    write(dir, "case-probe", "");
    let folds = dir.join("CASE-PROBE").exists();
    std::fs::remove_file(dir.join("case-probe")).unwrap();
    folds
}

#[test]
fn case_only_rename_of_a_non_ascii_name() {
    let dir = scratch_repo("wfc-umlaut");
    commit(&dir, &[("\u{c4}rger.txt", "x\n")], "one");
    let w = rename_file_on(&dir, "\u{c4}rger.txt", "\u{e4}rger.txt", &FOLDING).unwrap();
    assert!(w.warnings.is_empty(), "{:?}", w.warnings);
    let listed = names(&dir);
    assert!(listed.contains(&"\u{e4}rger.txt".to_owned()) && !listed.contains(&"\u{c4}rger.txt".to_owned()), "{listed:?}");
    assert_eq!(text(&dir, "\u{e4}rger.txt"), "x\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn decomposed_name_renamed_to_composed() {
    let dir = scratch_repo("wfc-nfd");
    let (nfd, nfc) = ("cafe\u{301}.txt", "caf\u{e9}.txt");
    write(&dir, nfd, "x\n");
    rename_file_on(&dir, nfd, nfc, &FOLDING).unwrap();
    let listed = names(&dir);
    assert!(listed.contains(&nfc.to_owned()) && !listed.contains(&nfd.to_owned()), "{listed:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn another_file_under_the_folded_name_is_not_replaced() {
    // The stand-in says the target exists, but it isn't the source by identity.
    let not_same = Disk { same_file: |_, _| false, ..FOLDING };
    let dir = scratch_repo("wfc-not-same");
    commit(&dir, &[("readme.md", "hi\n")], "one");
    let e = rename_file_on(&dir, "readme.md", "README.md", &not_same).unwrap_err();
    assert!(e.to_string().contains("README.md already exists"), "{e}");
    assert_eq!(names(&dir), [".git", "readme.md"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_change_of_case_in_a_parent_folder_is_refused() {
    let dir = scratch_repo("wfc-parent");
    commit(&dir, &[("docs/a.md", "a\n")], "one");
    for (disk, staged) in [(FOLDING, false), (FOLDING, true)] {
        let e = if staged {
            rename_file_staged_on(&dir, "docs/a.md", "Docs/a.md", &disk).unwrap_err()
        } else {
            rename_file_on(&dir, "docs/a.md", "Docs/a.md", &disk).unwrap_err()
        };
        assert_eq!(e.to_string(), "the folder Docs already exists as docs; use that spelling, or rename the folder first");
    }
    assert_eq!(names(&dir.join("docs")), ["a.md"]);
    // Where the file system keeps case apart, Docs is a new folder (with a warning).
    if !folds_case(&dir) {
        let w = rename_file_on(&dir, "docs/a.md", "Docs/a.md", &real()).unwrap();
        assert_eq!(text(&dir, "Docs/a.md"), "a\n");
        assert!(w.warnings.is_empty(), "docs/a.md was the only file in docs: {:?}", w.warnings);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staged_case_only_rename_of_a_non_ascii_name() {
    let dir = scratch_repo("wfc-staged-umlaut");
    commit(&dir, &[("\u{c4}.txt", "x\n")], "one");
    rename_file_staged_on(&dir, "\u{c4}.txt", "\u{e4}.txt", &FOLDING).unwrap();
    let index = Repository::open(&dir).unwrap().index().unwrap();
    let paths: Vec<String> = index.iter().map(|e| String::from_utf8(e.path).unwrap()).collect();
    assert_eq!(paths, ["\u{e4}.txt"]);
    assert!(names(&dir).contains(&"\u{e4}.txt".to_owned()));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The same renames on the real file system, where it ignores case (Windows and macOS on CI).
#[test]
fn real_case_insensitive_volume() {
    let dir = scratch_repo("wfc-real");
    if !folds_case(&dir) {
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    commit(&dir, &[("\u{c4}rger.txt", "x\n"), ("docs/a.md", "a\n"), ("\u{d6}l.txt", "o\n")], "one");
    super::rename_file(&dir, "\u{c4}rger.txt", "\u{e4}rger.txt").unwrap();
    assert!(names(&dir).contains(&"\u{e4}rger.txt".to_owned()), "{:?}", names(&dir));
    let e = super::rename_file(&dir, "docs/a.md", "Docs/a.md").unwrap_err();
    assert!(e.to_string().contains("already exists as docs"), "{e}");
    super::rename_file_staged(&dir, "\u{d6}l.txt", "\u{f6}l.txt").unwrap();
    let index = Repository::open(&dir).unwrap().index().unwrap();
    let paths: Vec<String> = index.iter().map(|e| String::from_utf8(e.path).unwrap()).collect();
    assert!(paths.contains(&"\u{f6}l.txt".to_owned()) && !paths.contains(&"\u{d6}l.txt".to_owned()), "{paths:?}");
    let e = create_file(&dir, "DOCS/new.md").unwrap_err();
    assert!(e.to_string().contains("already exists as docs"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// macOS (APFS) also ignores Unicode form: a decomposed name renamed to its composed form.
#[cfg(target_os = "macos")]
#[test]
fn real_normalization_insensitive_volume() {
    let dir = scratch_repo("wfc-real-nfd");
    let (nfd, nfc) = ("cafe\u{301}.txt", "caf\u{e9}.txt");
    write(&dir, nfd, "x\n");
    if !dir.join(nfc).exists() {
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    super::rename_file(&dir, nfd, nfc).unwrap();
    let listed = names(&dir);
    assert!(listed.iter().any(|n| fold(n) == fold(nfc)) && !listed.contains(&nfd.to_owned()), "{listed:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

fn no_canonicalize(_: &Path) -> std::io::Result<PathBuf> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Incorrect function."))
}

#[test]
fn inside_check_without_canonicalize() {
    let disk = Disk { canonicalize: no_canonicalize, ..real() };
    let dir = scratch_repo("wfc-canon");
    std::fs::create_dir_all(dir.join("a/b")).unwrap();
    assert!(ensure_inside_on(&dir, &dir.join("a/b"), &disk).is_ok());
    assert!(ensure_inside_on(&dir, &dir, &disk).is_ok());
    assert!(ensure_inside_on(&dir, &dir.join("a/../.."), &disk).is_err(), "no .. walk-outs");
    assert!(ensure_inside_on(&dir, dir.parent().unwrap(), &disk).is_err());
    let outside = scratch_repo("wfc-canon-out");
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(&outside, dir.join("link")).is_ok();
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_dir(&outside, dir.join("link")).is_ok();
    if linked {
        let e = ensure_inside_on(&dir, &dir.join("link"), &disk).unwrap_err();
        assert!(e.to_string().contains("is a link"), "{e}");
    }
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&outside);
}

#[test]
fn tracked_twins_are_found() {
    let tracked = |paths: &[&str], to: &str, from: Option<&str>| tracked_twin(paths.iter().map(|p| p.to_string()), to, from);
    assert_eq!(
        tracked(&["A.txt"], "a.txt", None).as_deref(),
        Some("A.txt is also tracked; Windows and macOS see it and a.txt as one file.")
    );
    assert_eq!(tracked(&["caf\u{e9}.txt"], "CAFE\u{301}.txt", None).as_deref().map(|w| w.starts_with("caf\u{e9}.txt")), Some(true));
    assert_eq!(
        tracked(&["docs/a.md", "b.md"], "Docs/new.md", None).as_deref(),
        Some("The folder docs is tracked; Windows and macOS see it and Docs as one folder.")
    );
    assert_eq!(tracked(&["a.txt"], "a.txt", None), None, "the same path");
    assert_eq!(tracked(&["A.txt"], "a.txt", Some("A.txt")), None, "a rename's own source");
    assert_eq!(tracked(&["docs/a.md"], "docs/b.md", None), None);
    assert_eq!(tracked(&["x/docs/a.md"], "Docs/b.md", None), None, "another depth");
}

#[test]
fn a_new_file_that_shares_a_name_with_a_tracked_one_is_warned_about() {
    let dir = scratch_repo("wfc-twin");
    commit(&dir, &[("Notes.txt", "n\n")], "one");
    std::fs::remove_file(dir.join("Notes.txt")).unwrap();
    let w = create_file(&dir, "notes.txt").unwrap();
    assert_eq!(w.warnings, ["Notes.txt is also tracked; Windows and macOS see it and notes.txt as one file."]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(not(windows))]
#[test]
fn a_name_windows_refuses_is_created_with_a_warning() {
    let dir = scratch_repo("wfc-aux");
    assert_eq!(super::check_new_path("src/aux.c"), Ok("src/aux.c".into()));
    let w = create_file(&dir, "src/aux.c").unwrap();
    assert_eq!(w.warnings, ["Windows reserves aux for a device, so aux.c won't check out on Windows."]);
    let _ = std::fs::remove_dir_all(&dir);
}
