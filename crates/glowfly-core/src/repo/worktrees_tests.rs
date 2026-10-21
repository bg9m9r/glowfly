//! Worktree listing against real repositories: none linked, a linked branch, a detached one,
//! locks with and without a reason, a deleted folder, loading from inside a linked worktree and
//! an unborn main branch.

use super::*;
use crate::testutil::*;
use git2::Repository;
use std::path::{Path, PathBuf};

fn one_commit(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    dir
}

fn list(dir: &Path) -> Vec<WorktreeInfo> {
    load(dir, &LoadOptions::all(100)).unwrap().worktrees
}

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

/// A sibling folder for a linked worktree of `main`.
fn sibling(main: &Path, suffix: &str) -> PathBuf {
    let p = main.with_file_name(format!("{}-wt-{suffix}", main.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&p);
    p
}

fn add(main: &Path, a: &[&str]) {
    git(main, &[&["worktree", "add", "-q"][..], a].concat());
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn only_the_main_worktree() {
    let dir = one_commit("wt-main-only");
    let list = list(&dir);
    assert_eq!(list.len(), 1, "{list:?}");
    let m = &list[0];
    assert!(m.is_main() && m.current);
    assert_eq!(m.branch.as_deref(), Some("main"));
    assert_eq!(m.head, Some(head(&dir)));
    assert!(m.locked.is_none() && !m.missing && !m.prunable);
    assert_eq!(std::fs::canonicalize(&m.path).unwrap(), std::fs::canonicalize(&dir).unwrap());
    assert_eq!(worktrees_at(&dir).unwrap(), list);
    cleanup(&[&dir]);
}

#[test]
fn linked_branch_worktree() {
    let dir = one_commit("wt-linked");
    let wt = sibling(&dir, "feat");
    add(&dir, &["-b", "feat", &wt.display().to_string()]);
    let list = list(&dir);
    assert_eq!(list.len(), 2, "{list:?}");
    assert!(list[0].is_main() && list[0].current);
    let l = &list[1];
    assert_eq!(l.name.as_deref(), Some(wt.file_name().unwrap().to_str().unwrap()));
    assert_eq!(l.branch.as_deref(), Some("feat"));
    assert_eq!(l.head, Some(Repository::open(&dir).unwrap().refname_to_id("refs/heads/feat").unwrap()));
    assert!(!l.current && !l.missing && !l.prunable && l.locked.is_none());
    assert_eq!(std::fs::canonicalize(&l.path).unwrap(), std::fs::canonicalize(&wt).unwrap());
    let text = l.path.display().to_string();
    assert!(!text.ends_with('/') && !text.ends_with('\\'), "{text}");
    if cfg!(windows) {
        assert!(!text.contains('/'), "{text}");
    }
    assert_eq!(l.folder_name(), wt.file_name().unwrap().to_string_lossy());
    cleanup(&[&wt, &dir]);
}

#[test]
fn detached_worktree() {
    let dir = one_commit("wt-detached");
    let wt = sibling(&dir, "det");
    add(&dir, &["--detach", &wt.display().to_string(), "HEAD~0"]);
    let l = &list(&dir)[1];
    assert_eq!(l.branch, None);
    assert_eq!(l.head, Some(head(&dir)));
    cleanup(&[&wt, &dir]);
}

#[test]
fn locked_worktree() {
    let dir = one_commit("wt-locked");
    let wt = sibling(&dir, "lock");
    let w = wt.display().to_string();
    add(&dir, &["-b", "lk", &w]);
    git(&dir, &["worktree", "lock", "--reason", "on usb", &w]);
    assert_eq!(list(&dir)[1].locked.as_deref(), Some("on usb"));
    git(&dir, &["worktree", "unlock", &w]);
    git(&dir, &["worktree", "lock", &w]);
    assert_eq!(list(&dir)[1].locked.as_deref(), Some(""));
    git(&dir, &["worktree", "unlock", &w]);
    assert_eq!(list(&dir)[1].locked, None);
    cleanup(&[&wt, &dir]);
}

#[test]
fn missing_folder() {
    let dir = one_commit("wt-missing");
    let wt = sibling(&dir, "gone");
    let w = wt.display().to_string();
    add(&dir, &["-b", "gone", &w]);
    std::fs::remove_dir_all(&wt).unwrap();
    let l = list(&dir)[1].clone();
    assert!(l.missing && l.prunable, "{l:?}");
    assert_eq!(l.branch.as_deref(), Some("gone"), "read from the HEAD file");
    git(&dir, &["worktree", "lock", &w]);
    let l = list(&dir)[1].clone();
    assert!(l.missing && !l.prunable, "{l:?}");
    cleanup(&[&wt, &dir]);
}

#[test]
fn loaded_from_a_linked_worktree() {
    let dir = one_commit("wt-from-linked");
    let wt = sibling(&dir, "inside");
    add(&dir, &["-b", "inside", &wt.display().to_string()]);
    let snap = load(&wt, &LoadOptions::all(100)).unwrap();
    assert_eq!(std::fs::canonicalize(&snap.workdir).unwrap(), std::fs::canonicalize(&wt).unwrap());
    assert_eq!(snap.worktrees.len(), 2, "{:?}", snap.worktrees);
    assert!(snap.worktrees[0].is_main() && !snap.worktrees[0].current);
    assert!(!snap.worktrees[1].is_main() && snap.worktrees[1].current);
    cleanup(&[&wt, &dir]);
}

#[test]
fn unborn_main_branch() {
    let dir = scratch_repo("wt-unborn");
    let list = list(&dir);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].branch.as_deref(), Some("main"));
    assert_eq!(list[0].head, None);
    cleanup(&[&dir]);
}
