//! A worktree made with a new branch: undo removes it (never forced, only while it holds nothing
//! new) and its branch while unchanged; redo makes it again.

use super::tests::{commit, refused, run_plan};
use super::*;
use crate::repo::worktrees_at;
use crate::testutil::*;
use crate::worktree_ops::{RemoveForce, WorktreeBase, add_cmd, remove_cmd};
use git2::Repository;
use std::path::{Path, PathBuf};

/// A fresh folder for the worktree, next to the test's repository.
fn wt_folder(dir: &Path) -> PathBuf {
    let folder = dir.with_file_name(format!("{}-wt", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

/// The worktree at `folder` as git lists it.
fn listed(dir: &Path, folder: &Path) -> Option<PathBuf> {
    worktrees_at(dir).unwrap().into_iter().find(|w| !w.is_main() && crate::paths::same_folder(&w.path, folder)).map(|w| w.path)
}

fn tip(dir: &Path, name: &str) -> Option<Oid> {
    Repository::open(dir).unwrap().find_reference(&format!("refs/heads/{name}")).ok().and_then(|r| r.target())
}

/// Runs `git <a>` as the Create worktree job does, recording `AddWorktree` for `branch` at `folder`.
fn add(dir: &Path, folder: &Path, branch: &str, a: &[&str]) -> Option<Action> {
    let kind = UndoKind::AddWorktree { path: folder.to_owned(), branch: branch.into() };
    let b = before(dir, &kind).unwrap();
    git(dir, a);
    after(dir, kind, b).unwrap()
}

fn add_wt(dir: &Path, folder: &Path) -> Action {
    let f = folder.display().to_string();
    add(dir, folder, "wt", &["worktree", "add", "-b", "wt", &f, "HEAD"]).expect("recorded")
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn undo_removes_the_worktree_and_its_unchanged_branch() {
    let dir = scratch_repo("undo-add-wt");
    let c1 = commit(&dir, "a.txt", "one\n", "c1");
    let folder = wt_folder(&dir);
    let action = add_wt(&dir, &folder);
    assert_eq!(action, Action::AddWorktree { path: folder.clone(), created: Created { name: "wt".into(), oid: c1, upstream: None } });
    let at = listed(&dir, &folder).expect("listed");
    let undo = plan(&dir, &action, false).unwrap();
    assert_eq!(undo, Plan::Run(vec![remove_cmd(&at, RemoveForce::None), args(&["branch", "-D", "wt"])]));
    run_plan(&dir, undo);
    assert!(!folder.exists());
    assert_eq!(tip(&dir, "wt"), None);
    assert_eq!(worktrees_at(&dir).unwrap().len(), 1, "only the main worktree");

    // A branch that existed already wasn't made by it.
    git(&dir, &["branch", "wt"]);
    let f = folder.display().to_string();
    assert_eq!(add(&dir, &folder, "wt", &["worktree", "add", &f, "wt"]), None);
    cleanup(&[&folder, &dir]);
}

#[test]
fn a_moved_branch_is_kept() {
    let dir = scratch_repo("undo-add-wt-moved");
    commit(&dir, "a.txt", "one\n", "c1");
    let folder = wt_folder(&dir);
    let action = add_wt(&dir, &folder);
    let c2 = commit(&folder, "b.txt", "two\n", "c2");
    let at = listed(&dir, &folder).expect("listed");
    let undo = plan(&dir, &action, false).unwrap();
    assert_eq!(undo, Plan::Run(vec![remove_cmd(&at, RemoveForce::None)]));
    run_plan(&dir, undo);
    assert!(!folder.exists());
    assert_eq!(tip(&dir, "wt"), Some(c2), "the branch keeps its new commit");

    // Redo checks the kept branch out again.
    let redo = plan(&dir, &action, true).unwrap();
    assert_eq!(redo, Plan::Run(vec![add_cmd(&folder, &WorktreeBase::Branch("wt".into()), None)]));
    // Checked out in the main worktree meanwhile: git would refuse.
    git(&dir, &["checkout", "-q", "wt"]);
    let r = refused(plan(&dir, &action, true).unwrap());
    assert!(matches!(&r, Refusal::Unsafe(m) if m.contains("checked out in another worktree")), "{r:?}");
    git(&dir, &["checkout", "-q", "main"]);
    run_plan(&dir, redo);
    let wt = worktrees_at(&dir).unwrap().into_iter().find(|w| !w.is_main()).expect("back");
    assert_eq!((wt.branch.as_deref(), wt.head), (Some("wt"), Some(c2)));
    cleanup(&[&folder, &dir]);
}

#[test]
fn refuses_when_the_worktree_has_new_files() {
    let dir = scratch_repo("undo-add-wt-files");
    write(&dir, ".gitignore", "*.log\n");
    commit(&dir, "a.txt", "one\n", "c1");
    let folder = wt_folder(&dir);
    let action = add_wt(&dir, &folder);
    let unsafe_now = |what: &str| {
        let r = refused(plan(&dir, &action, false).unwrap());
        assert!(matches!(&r, Refusal::Unsafe(m) if m.contains("has changes or files made since")), "{what}: {r:?}");
    };
    write(&folder, "new.txt", "new\n");
    unsafe_now("untracked");
    std::fs::remove_file(folder.join("new.txt")).unwrap();
    write(&folder, "build.log", "log\n");
    unsafe_now("ignored");
    std::fs::remove_file(folder.join("build.log")).unwrap();
    write(&folder, "a.txt", "changed\n");
    unsafe_now("modified");
    git(&folder, &["checkout", "--", "a.txt"]);
    assert!(matches!(plan(&dir, &action, false).unwrap(), Plan::Run(_)), "clean again");

    let f = folder.display().to_string();
    git(&dir, &["worktree", "lock", &f]);
    let r = refused(plan(&dir, &action, false).unwrap());
    assert!(matches!(&r, Refusal::Unsafe(m) if m.contains("is locked")), "{r:?}");
    git(&dir, &["worktree", "unlock", &f]);

    std::fs::remove_dir_all(&folder).unwrap();
    assert!(refused(plan(&dir, &action, false).unwrap()).is_stale(), "its folder is gone");
    cleanup(&[&dir]);
}

#[test]
fn redo_makes_it_again() {
    let dir = scratch_repo("undo-add-wt-redo");
    let c1 = commit(&dir, "a.txt", "one\n", "c1");
    let folder = wt_folder(&dir);
    let action = add_wt(&dir, &folder);
    run_plan(&dir, plan(&dir, &action, false).unwrap());

    std::fs::create_dir_all(&folder).unwrap();
    write(&folder, "x.txt", "x\n");
    assert!(refused(plan(&dir, &action, true).unwrap()).is_stale(), "a non-empty folder is in the way");
    std::fs::remove_dir_all(&folder).unwrap();

    let redo = plan(&dir, &action, true).unwrap();
    assert_eq!(redo, Plan::Run(vec![add_cmd(&folder, &WorktreeBase::Commit(c1), Some("wt"))]));
    run_plan(&dir, redo);
    let wt = worktrees_at(&dir).unwrap().into_iter().find(|w| !w.is_main()).expect("back");
    assert_eq!((wt.branch.as_deref(), wt.head), (Some("wt"), Some(c1)));
    assert!(refused(plan(&dir, &action, true).unwrap()).is_stale(), "a worktree is registered there again");

    // With an upstream whose remote-tracking branch is still there, it is set again.
    run_plan(&dir, plan(&dir, &action, false).unwrap());
    git(&dir, &["remote", "add", "origin", "https://example.invalid/repo.git"]);
    git(&dir, &["update-ref", "refs/remotes/origin/wt", &c1.to_string()]);
    let tracked =
        Action::AddWorktree { path: folder.clone(), created: Created { name: "wt".into(), oid: c1, upstream: Some("origin/wt".into()) } };
    let redo = plan(&dir, &tracked, true).unwrap();
    let mut want = vec![add_cmd(&folder, &WorktreeBase::Commit(c1), Some("wt"))];
    want.push(args(&["branch", "--set-upstream-to", "origin/wt", "wt"]));
    assert_eq!(redo, Plan::Run(want));
    run_plan(&dir, redo);
    cleanup(&[&folder, &dir]);
}

#[test]
fn remote_base_records_the_tracking_branch() {
    let (work, bare) = with_origin("undo-add-wt-remote");
    git(&work, &["branch", "x"]);
    git(&work, &["push", "-q", "origin", "x"]);
    git(&work, &["branch", "-D", "x"]);
    let oid = tip(&work, "main").unwrap();
    let folder = wt_folder(&work);
    let f = folder.display().to_string();
    let action = add(&work, &folder, "x", &["worktree", "add", "--track", "-b", "x", &f, "refs/remotes/origin/x"]).expect("recorded");
    assert_eq!(
        action,
        Action::AddWorktree { path: folder.clone(), created: Created { name: "x".into(), oid, upstream: Some("origin/x".into()) } }
    );
    cleanup(&[&folder, &work, &bare]);
}
