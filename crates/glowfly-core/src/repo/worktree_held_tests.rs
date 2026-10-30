//! The branch a worktree is rebasing or bisecting with HEAD detached, against real repositories:
//! a linked worktree stopped mid-rebase (merge and apply backends), one mid-bisect from a branch
//! and one from a detached HEAD, the main worktree's seen from a linked one, the refs key
//! following it, and the state files read directly.

use super::*;
use crate::gitcli::{args, run};
use crate::testutil::*;
use std::path::{Path, PathBuf};

/// main: base, then "main" on a.txt; `topic` (from base, "topic" on a.txt, so its rebase onto
/// main conflicts) checked out in a linked worktree beside it. Returns (repo, worktree).
fn conflicting(name: &str) -> (PathBuf, PathBuf) {
    let dir = scratch_repo(name);
    let commit = |at: &Path, text: &str, msg: &str| {
        write(at, "a.txt", text);
        git(at, &["commit", "-q", "-am", msg]);
    };
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    let wt = dir.with_file_name(format!("{}-topic", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&wt);
    git(&dir, &["worktree", "add", "-q", "-b", "topic", &wt.display().to_string()]);
    commit(&dir, "one\nmain\n", "main edit");
    commit(&wt, "one\ntopic\n", "topic edit");
    (dir, wt)
}

fn stops(dir: &Path, a: &[&str]) {
    let (ok, out) = run(dir, &args(a));
    assert!(!ok, "git {a:?} should stop for conflicts: {out}");
}

fn worktrees(dir: &Path) -> Vec<WorktreeInfo> {
    load(dir, &LoadOptions::all(100)).unwrap().worktrees
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn a_worktree_stopped_mid_rebase_holds_its_branch() {
    let (dir, wt) = conflicting("held-rebase");
    let before = refs_key(&dir).unwrap();
    stops(&wt, &["rebase", "main"]);
    let l = worktrees(&dir)[1].clone();
    assert_eq!(l.branch, None, "HEAD is detached while the rebase stops");
    assert_eq!(l.held, Some(HeldBranch { name: "topic".into(), by: BranchUse::Rebasing }));
    assert_eq!(l.uses("topic"), Some(BranchUse::Rebasing));
    assert_eq!(l.uses("main"), None);
    let during = refs_key(&dir).unwrap();
    assert_ne!(before, during);

    git(&wt, &["rebase", "--abort"]);
    let l = worktrees(&dir)[1].clone();
    assert_eq!((l.branch.as_deref(), l.held.as_ref()), (Some("topic"), None));
    assert_eq!(l.uses("topic"), Some(BranchUse::CheckedOut));
    assert_ne!(refs_key(&dir).unwrap(), during);

    stops(&wt, &["rebase", "--apply", "main"]);
    assert!(wt_git_dir(&dir, &wt).join("rebase-apply").is_dir(), "the apply backend");
    assert_eq!(worktrees(&dir)[1].uses("topic"), Some(BranchUse::Rebasing));
    git(&wt, &["rebase", "--abort"]);
    cleanup(&[&wt, &dir]);
}

/// The linked worktree `wt`'s own git dir.
fn wt_git_dir(dir: &Path, wt: &Path) -> PathBuf {
    dir.join(".git").join("worktrees").join(wt.file_name().unwrap())
}

#[test]
fn a_bisect_holds_the_branch_it_started_from_not_a_commit() {
    let (dir, wt) = conflicting("held-bisect");
    for n in 0..3 {
        write(&wt, "b.txt", &format!("{n}\n"));
        git(&wt, &["add", "b.txt"]);
        git(&wt, &["commit", "-q", "-m", &format!("b{n}")]);
    }
    git(&wt, &["bisect", "start", "topic", "topic~4"]);
    let l = worktrees(&dir)[1].clone();
    assert_eq!(l.branch, None, "bisect checked out a commit to test");
    assert_eq!(l.uses("topic"), Some(BranchUse::Bisecting));
    git(&wt, &["bisect", "reset"]);
    assert_eq!(worktrees(&dir)[1].held, None);

    git(&wt, &["checkout", "-q", "--detach"]);
    git(&wt, &["bisect", "start", "HEAD", "HEAD~4"]);
    assert_eq!(worktrees(&dir)[1].held, None, "started detached: BISECT_START holds a commit");
    git(&wt, &["bisect", "reset"]);
    cleanup(&[&wt, &dir]);
}

#[test]
fn the_main_worktree_s_rebase_is_seen_from_a_linked_one() {
    let (dir, wt) = conflicting("held-main");
    git(&dir, &["checkout", "-q", "-b", "mine"]);
    write(&dir, "a.txt", "one\nmine\n");
    git(&dir, &["commit", "-q", "-am", "mine"]);
    let before = refs_key(&wt).unwrap();
    stops(&dir, &["rebase", "topic"]);
    assert_ne!(refs_key(&wt).unwrap(), before, "seen from the linked worktree");
    let list = worktrees(&wt);
    assert!(list[0].is_main() && !list[0].current);
    assert_eq!(list[0].uses("mine"), Some(BranchUse::Rebasing));
    git(&dir, &["rebase", "--abort"]);
    cleanup(&[&wt, &dir]);
}

#[test]
fn the_state_files_are_read_as_git_writes_them() {
    let dir = scratch_repo("held-files");
    let g = dir.join(".git");
    let put = |p: &str, text: &str| {
        let f = g.join(p);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(f, text).unwrap();
    };
    assert_eq!(held_branch(&g), None);
    put("rebase-merge/head-name", "detached HEAD\n");
    assert_eq!(held_branch(&g), None, "a rebase of a detached HEAD holds no branch");
    put("rebase-merge/head-name", "refs/heads/feat/x\n");
    assert_eq!(held_branch(&g), Some(HeldBranch { name: "feat/x".into(), by: BranchUse::Rebasing }));
    std::fs::remove_dir_all(g.join("rebase-merge")).unwrap();

    put("BISECT_START", "topic\n");
    assert_eq!(held_branch(&g), None, "no bisect runs without BISECT_LOG");
    put("BISECT_LOG", "");
    assert_eq!(held_branch(&g), Some(HeldBranch { name: "topic".into(), by: BranchUse::Bisecting }));
    put("BISECT_START", &format!("{}\n", "ab12".repeat(10)));
    assert_eq!(held_branch(&g), None, "a commit");
    cleanup(&[&dir]);
}
