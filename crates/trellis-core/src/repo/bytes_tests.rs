//! Paths that aren't UTF-8. The trees and index entries are built from bytes with `git mktree`
//! and `git update-index --index-info`, so no such file has to exist on disk and the tests run
//! on every OS (git2's `DiffFile::path()` panics on Windows for these names).

use super::*;
use crate::testutil::*;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// "café.txt" in Latin-1: not UTF-8.
const CAFE: &[u8] = b"caf\xe9.txt";
const CAFE_SHOWN: &str = "caf\u{FFFD}.txt";

/// Runs `git <a>` in `dir` with `input` on stdin; returns its stdout as bytes.
pub(super) fn git_bytes(dir: &Path, a: &[&str], input: &[u8]) -> Vec<u8> {
    let mut child = Command::new("git").args(a).current_dir(dir).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "git {a:?} failed");
    out.stdout
}

/// `git_bytes` for output that is text (ids), trimmed.
pub(super) fn git_in(dir: &Path, a: &[&str], input: &[u8]) -> String {
    String::from_utf8(git_bytes(dir, a, input)).unwrap().trim().to_owned()
}

pub(super) fn blob(dir: &Path, text: &str) -> String {
    git_in(dir, &["hash-object", "-w", "--stdin"], text.as_bytes())
}

/// The index's paths, as bytes.
pub(super) fn index_paths(dir: &Path) -> Vec<Vec<u8>> {
    git_bytes(dir, &["ls-files", "-z"], b"").split(|&b| b == 0).filter(|n| !n.is_empty()).map(<[u8]>::to_vec).collect()
}

const A1: &str = "one\n";
const A2: &str = "two\n";
const C: &str = "café\nlong enough to be found as a rename\n";

/// A tree of (name, blob id) files at the root.
fn tree(dir: &Path, files: &[(&[u8], &str)]) -> String {
    let mut input = Vec::new();
    for (name, id) in files {
        input.extend_from_slice(format!("100644 blob {id}\t").as_bytes());
        input.extend_from_slice(name);
        input.push(b'\n');
    }
    git_in(dir, &["mktree"], &input)
}

fn commit(dir: &Path, tree: &str, parent: Option<&str>, msg: &str) -> Oid {
    let mut a = vec!["commit-tree", tree, "-m", msg];
    if let Some(p) = parent {
        a.extend(["-p", p]);
    }
    Oid::from_str(&git_in(dir, &a, b"")).unwrap()
}

/// A repository whose `main` has a.txt, then a commit that changes a.txt and adds café.txt
/// (Latin-1 bytes), then one that renames café.txt to b.txt. Nothing is checked out. Returns
/// (dir, first, adds café, renames it).
fn fixture(name: &str) -> (std::path::PathBuf, Oid, Oid, Oid) {
    let dir = scratch_repo(name);
    let (a1, a2, c) = (blob(&dir, A1), blob(&dir, A2), blob(&dir, C));
    let first = commit(&dir, &tree(&dir, &[(b"a.txt", &a1)]), None, "first");
    let adds = commit(&dir, &tree(&dir, &[(b"a.txt", &a2), (CAFE, &c)]), Some(&first.to_string()), "adds");
    let renames = commit(&dir, &tree(&dir, &[(b"a.txt", &a2), (b"b.txt", &c)]), Some(&adds.to_string()), "renames");
    git(&dir, &["update-ref", "refs/heads/main", &renames.to_string()]);
    (dir, first, adds, renames)
}

fn not_utf8(f: &FileEntry) -> bool {
    f.path == CAFE_SHOWN && f.hazard == Some(Hazard::NotUtf8(CAFE.to_vec()))
}

#[test]
fn commit_details_and_comparisons_list_names_that_are_not_utf8() {
    let (dir, first, adds, _) = fixture("bytes-detail");
    let detail = commit_detail(&dir, adds).unwrap();
    assert_eq!(detail.files.len(), 2);
    assert!(detail.files.iter().any(not_utf8), "{:?}", detail.files);
    assert!(!detail.in_workdir(CAFE_SHOWN), "a name that isn't UTF-8 can't be opened");
    let cmp = compare(&dir, first, adds).unwrap();
    assert!(cmp.files.iter().any(not_utf8), "{:?}", cmp.files);
    assert!(commit_stats(&dir, &[adds])[0].1 == 2);

    // The other file's diff loads; the shown name matches no file rather than another one.
    let a = load_diff(&dir, &DiffTarget::Commit(adds, "a.txt".into())).unwrap();
    assert_eq!(a.hunks.len(), 1);
    assert!(matches!(load_diff(&dir, &DiffTarget::Commit(adds, CAFE_SHOWN.into())), Err(crate::error::Error::NotFound(_))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_history_stops_at_a_rename_from_a_name_that_is_not_utf8() {
    let (dir, _, adds, renames) = fixture("bytes-history");
    let h = file_history(&dir, "b.txt", &HistoryStart::Head, 100).unwrap();
    assert_eq!(h.entries.iter().map(|e| e.oid).collect::<Vec<_>>(), [renames]);
    let a = file_history(&dir, "a.txt", &HistoryStart::Head, 100).unwrap();
    assert_eq!(a.entries.len(), 2, "a.txt changed in the first commit and in {adds}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn status_lists_a_staged_rename_to_a_name_that_is_not_utf8() {
    let (dir, ..) = fixture("bytes-status");
    // The index renames b.txt back to café.txt; neither is on disk.
    git(&dir, &["read-tree", "main~1"]);
    let (_, staged, _) = status_at(&dir, false).unwrap();
    assert!(staged.iter().any(|f| not_utf8(f) && f.status == 'R'), "{staged:?}");
    // git's own status read reports it the same way.
    let (_, staged_git) = super::status_cli::status(&dir).expect("git ran");
    assert!(staged_git.iter().any(|f| not_utf8(f) && f.status == 'R'), "{staged_git:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstaging_a_rename_from_a_name_that_is_not_utf8_restores_its_entry() {
    let (dir, _, adds, _) = fixture("bytes-unstage-rename");
    // HEAD has café.txt; the index renames it to b.txt, which isn't on disk.
    git(&dir, &["update-ref", "refs/heads/main", &adds.to_string()]);
    let renamed = tree(&dir, &[(b"a.txt", &blob(&dir, A2)), (b"b.txt", &blob(&dir, C))]);
    git(&dir, &["read-tree", &renamed]);
    unstage_gone_new_file(&dir, "b.txt").unwrap();
    assert_eq!(index_paths(&dir), [b"a.txt".to_vec(), CAFE.to_vec()]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A real file whose name isn't UTF-8 (Linux file systems take any bytes; APFS and NTFS don't).
#[cfg(target_os = "linux")]
#[test]
fn an_untracked_file_named_with_bytes_that_are_not_utf8() {
    use std::os::unix::ffi::OsStrExt;
    let dir = scratch_repo("bytes-untracked");
    write(&dir, "t.txt", "t\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    std::fs::write(dir.join(std::ffi::OsStr::from_bytes(CAFE)), "x\n").unwrap();
    let (unstaged, _, _) = status_at(&dir, false).unwrap();
    assert!(unstaged.iter().any(|f| not_utf8(f) && f.status == '?'), "{unstaged:?}");
    let (by_git, _) = super::status_cli::status(&dir).expect("git ran");
    assert_eq!(by_git, unstaged);
    // The shown name is no file: acting on it reaches nothing else.
    assert!(discard_untracked(&dir, CAFE_SHOWN, Removal::Permanent).is_err());
    assert!(dir.join(std::ffi::OsStr::from_bytes(CAFE)).exists());
    let _ = std::fs::remove_dir_all(&dir);
}
