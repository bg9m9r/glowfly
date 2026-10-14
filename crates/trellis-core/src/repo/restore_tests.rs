//! Restoring a file from a commit against real repositories: the bytes written, the index and
//! HEAD left alone, the parent fallback for a deleted file, and every refusal writing nothing.
//! Also which changed files `commit_detail` and the status say are missing from the working tree.

use super::*;
use crate::gitcli::{args, run};
use crate::testutil::*;
use git2::{Oid, Repository};
use std::path::Path;

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn commit(dir: &Path, files: &[(&str, &str)], msg: &str) -> Oid {
    for (f, t) in files {
        if let Some(parent) = dir.join(f).parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        write(dir, f, t);
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", msg]);
    head(dir)
}

fn text(dir: &Path, path: &str) -> String {
    std::fs::read_to_string(dir.join(path)).unwrap()
}

/// The index's stage-0 blob for `path`, if it has one.
fn index_text(dir: &Path, path: &str) -> Option<String> {
    let repo = Repository::open(dir).unwrap();
    let entry = repo.index().unwrap().get_path(Path::new(path), 0)?;
    Some(String::from_utf8(repo.find_blob(entry.id).unwrap().content().to_vec()).unwrap())
}

fn status(dir: &Path) -> String {
    run(dir, &args(&["status", "--porcelain"])).1
}

#[test]
fn restores_modified_file_from_older_commit() {
    let dir = scratch_repo("restore-older");
    let v1 = commit(&dir, &[("f.txt", "v1\n")], "one");
    let v2 = commit(&dir, &[("f.txt", "v2\n")], "two");
    write(&dir, "f.txt", "v3\n");
    assert_eq!(restore_file(&dir, v1, "f.txt").unwrap(), RestoredFrom::Commit);
    assert_eq!(text(&dir, "f.txt"), "v1\n");
    assert_eq!(index_text(&dir, "f.txt").as_deref(), Some("v2\n"), "the index is untouched");
    assert_eq!(head(&dir), v2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restores_when_missing_from_workdir() {
    let dir = scratch_repo("restore-missing");
    let c = commit(&dir, &[("f.txt", "kept\n")], "one");
    std::fs::remove_file(dir.join("f.txt")).unwrap();
    restore_file(&dir, c, "f.txt").unwrap();
    assert_eq!(text(&dir, "f.txt"), "kept\n");
    assert_eq!(status(&dir), "", "back as committed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deleted_by_commit_falls_back_to_parent() {
    let dir = scratch_repo("restore-deleted");
    let a = commit(&dir, &[("f.txt", "from A\n"), ("other.txt", "x\n")], "A");
    std::fs::remove_file(dir.join("f.txt")).unwrap();
    let b = commit(&dir, &[], "B deletes f.txt");
    assert_eq!(restore_file(&dir, b, "f.txt").unwrap(), RestoredFrom::Parent(a));
    assert_eq!(text(&dir, "f.txt"), "from A\n");
    assert_eq!(index_text(&dir, "f.txt"), None, "not added to the index");
    assert!(status(&dir).contains("?? f.txt"), "{}", status(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn not_in_commit_or_parent_is_not_found() {
    let dir = scratch_repo("restore-never");
    commit(&dir, &[("a.txt", "a\n")], "one");
    let c = commit(&dir, &[("a.txt", "b\n")], "two");
    let e = restore_file(&dir, c, "ghost.txt").unwrap_err();
    assert!(e.is_not_found(), "{e}");
    assert!(!dir.join("ghost.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refuses_conflicted_path() {
    let dir = scratch_repo("restore-conflict");
    let base = commit(&dir, &[("f.txt", "base\n")], "base");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    commit(&dir, &[("f.txt", "side\n")], "side");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, &[("f.txt", "main\n")], "main");
    let (ok, _) = run(&dir, &args(&["merge", "side"]));
    assert!(!ok, "the merge conflicts");
    let before = text(&dir, "f.txt");
    assert!(before.contains("<<<<<<<"));
    let e = restore_file(&dir, base, "f.txt").unwrap_err();
    assert!(e.to_string().contains("conflicts"), "{e}");
    assert_eq!(text(&dir, "f.txt"), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refuses_when_folder_in_the_way() {
    let dir = scratch_repo("restore-folder");
    let c = commit(&dir, &[("f.txt", "file\n")], "one");
    std::fs::remove_file(dir.join("f.txt")).unwrap();
    std::fs::create_dir_all(dir.join("f.txt")).unwrap();
    write(&dir, "f.txt/x", "inside\n");
    assert!(restore_file(&dir, c, "f.txt").is_err());
    assert_eq!(text(&dir, "f.txt/x"), "inside\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refuses_when_a_parent_is_a_file() {
    let dir = scratch_repo("restore-parent-file");
    let c = commit(&dir, &[("d/f.txt", "nested\n")], "one");
    std::fs::remove_dir_all(dir.join("d")).unwrap();
    write(&dir, "d", "a file now\n");
    let e = restore_file(&dir, c, "d/f.txt").unwrap_err();
    assert!(e.to_string().contains("d is a file"), "{e}");
    assert_eq!(text(&dir, "d"), "a file now\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refuses_a_folder_entry() {
    let dir = scratch_repo("restore-tree");
    let c = commit(&dir, &[("d/f.txt", "nested\n")], "one");
    assert!(matches!(restore_file(&dir, c, "d"), Err(crate::error::Error::Unsupported(_))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restores_into_missing_subfolder() {
    let dir = scratch_repo("restore-subfolder");
    let c = commit(&dir, &[("dir/sub/f.txt", "deep\n")], "one");
    std::fs::remove_dir_all(dir.join("dir")).unwrap();
    restore_file(&dir, c, "dir/sub/f.txt").unwrap();
    assert_eq!(text(&dir, "dir/sub/f.txt"), "deep\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn keeps_exec_bit() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch_repo("restore-exec");
    write(&dir, "run.sh", "#!/bin/sh\n");
    std::fs::set_permissions(dir.join("run.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let c = commit(&dir, &[], "script");
    std::fs::remove_file(dir.join("run.sh")).unwrap();
    restore_file(&dir, c, "run.sh").unwrap();
    let mode = std::fs::metadata(dir.join("run.sh")).unwrap().permissions().mode();
    assert_ne!(mode & 0o111, 0, "executable again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn untracked_file_at_path_is_overwritten() {
    let dir = scratch_repo("restore-untracked");
    let c = commit(&dir, &[("f.txt", "committed\n")], "one");
    git(&dir, &["rm", "-q", "--cached", "f.txt"]);
    write(&dir, "f.txt", "edited while untracked\n");
    restore_file(&dir, c, "f.txt").unwrap();
    assert_eq!(text(&dir, "f.txt"), "committed\n");
    assert_eq!(index_text(&dir, "f.txt"), None, "still out of the index");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn commit_detail_reports_absent() {
    let dir = scratch_repo("detail-absent");
    let c = commit(&dir, &[("a.txt", "a\n"), ("b.txt", "b\n")], "two files");
    std::fs::remove_file(dir.join("b.txt")).unwrap();
    let d = commit_detail(&dir, c).unwrap();
    assert_eq!(d.absent, HashSet::from(["b.txt".to_owned()]));
    assert!(d.in_workdir("a.txt"));
    assert!(!d.in_workdir("b.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn snapshot_missing_from_workdir() {
    let dir = scratch_repo("missing-from-workdir");
    commit(&dir, &[("a.txt", "a\n"), ("b.txt", "b\n"), ("c.txt", "c\n"), ("d.txt", "d\n")], "four files");
    std::fs::remove_file(dir.join("a.txt")).unwrap();
    git(&dir, &["rm", "-q", "b.txt"]);
    git(&dir, &["rm", "-q", "c.txt"]);
    write(&dir, "c.txt", "back, untracked\n");
    write(&dir, "d.txt", "modified\n");
    let s = load(&dir, &LoadOptions::all(10)).unwrap();
    assert!(s.missing_from_workdir("a.txt"), "unstaged deletion");
    assert!(s.missing_from_workdir("b.txt"), "staged deletion");
    assert!(!s.missing_from_workdir("c.txt"), "deleted in the index, re-created since");
    assert!(!s.missing_from_workdir("d.txt"), "modified");
    let _ = std::fs::remove_dir_all(&dir);
}
