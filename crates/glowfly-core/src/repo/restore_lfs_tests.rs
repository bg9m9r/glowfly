//! Restoring a Git LFS file from a commit: the commit stores a pointer, and the working tree
//! gets the file's real contents through git-lfs's smudge filter. Skipped without git-lfs.

use super::*;
use crate::error::Error;
use crate::gitcli::{args, run};
use crate::testutil::{git, lfs_or_skip, scratch_repo, write};
use git2::{Oid, Repository};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// A repository tracking `*.bin` in LFS, where a.bin was "first version" in the returned base
/// commit and "second version" in HEAD.
fn lfs_repo(name: &str) -> Option<(PathBuf, Oid)> {
    lfs_or_skip(name)?;
    let dir = scratch_repo(name);
    git(&dir, &["lfs", "install", "--local"]);
    git(&dir, &["lfs", "track", "*.bin"]);
    write(&dir, "a.bin", "first version\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    let base = Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
    write(&dir, "a.bin", "second version\n");
    git(&dir, &["commit", "-q", "-am", "second"]);
    Some((dir, base))
}

fn read(dir: &Path, file: &str) -> String {
    std::fs::read_to_string(dir.join(file)).unwrap()
}

fn status(dir: &Path) -> String {
    run(dir, &args(&["status", "--porcelain"])).1
}

fn blob_text(repo: &Repository, id: Oid) -> String {
    String::from_utf8_lossy(repo.find_blob(id).unwrap().content()).into_owned()
}

#[test]
fn restoring_an_lfs_file_writes_its_contents_not_the_pointer() {
    let Some((dir, base)) = lfs_repo("restore-lfs") else { return };
    let repo = Repository::open(&dir).unwrap();
    let stored = repo.find_commit(base).unwrap().tree().unwrap().get_path(Path::new("a.bin")).unwrap().id();
    assert!(blob_text(&repo, stored).starts_with("version https://git-lfs.github.com/spec/v1"), "the commit holds a pointer");
    let head_blob = repo.head().unwrap().peel_to_tree().unwrap().get_path(Path::new("a.bin")).unwrap().id();
    drop(repo);
    assert_eq!(restore_file(&dir, base, "a.bin", false).unwrap(), RestoredFrom::Commit);
    assert_eq!(read(&dir, "a.bin"), "first version\n", "smudged, not the pointer text");
    let index_blob = Repository::open(&dir).unwrap().index().unwrap().get_path(Path::new("a.bin"), 0).unwrap().id;
    assert_eq!(index_blob, head_blob, "the index is left alone");
    assert_eq!(status(&dir), " M a.bin");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restoring_an_lfs_file_the_commit_deleted_writes_the_parent_version() {
    let Some((dir, _)) = lfs_repo("restore-lfs-deleted") else { return };
    git(&dir, &["rm", "-q", "a.bin"]);
    git(&dir, &["commit", "-q", "-m", "remove"]);
    let gone = Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
    assert!(matches!(restore_file(&dir, gone, "a.bin", false).unwrap(), RestoredFrom::Parent(_)));
    assert_eq!(read(&dir, "a.bin"), "second version\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_clean_lfs_file_with_stale_stat_data_can_be_restored_without_overwrite() {
    let Some((dir, base)) = lfs_repo("restore-lfs-stale") else { return };
    // Same contents, older mtime: the index's stat data no longer matches the file.
    let file = std::fs::File::options().write(true).open(dir.join("a.bin")).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(3600)).unwrap();
    drop(file);
    assert_eq!(restore_file(&dir, base, "a.bin", false).unwrap(), RestoredFrom::Commit, "a clean file is no reason to ask");
    assert_eq!(read(&dir, "a.bin"), "first version\n");
    write(&dir, "a.bin", "local edit\n");
    assert!(matches!(restore_file(&dir, base, "a.bin", false), Err(Error::WouldOverwrite(_))), "a real edit still asks");
    let _ = std::fs::remove_dir_all(&dir);
}
