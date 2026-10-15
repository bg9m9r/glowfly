//! Headless tests for removing, locking, unlocking and pruning worktrees with real git: Remove
//! (clean, with changes, locked, with the branch deleted or not), the reports for a folder left
//! behind and a failed branch delete, the refusals, Lock / Unlock and Prune.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::worktree_manage::{LockForm, RemoveForm, needs_force_text, prune_blocked};
use crate::state::worktrees::find;
use crate::state::{AppState, Modal, RunningJob};
use crate::worktree::{RemoveRequest, WorktreeMsg};
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo::WorktreeInfo;
use trellis_core::worktree_ops::{Refusal, RemoveForce, RemoveOutcome};

/// A repository opened in `st` and the folder linked worktrees go in (emptied first).
fn opened(name: &str) -> (AppState, PathBuf, PathBuf) {
    let dir = scratch_repo(name);
    let root = dir.with_file_name(format!("{}-worktrees", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&root);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir, root)
}

/// Adds a linked worktree on a new branch `branch` in `<root>/<branch>` and refreshes.
fn add(st: &mut AppState, dir: &Path, root: &Path, branch: &str) -> PathBuf {
    let path = root.join(branch);
    git(dir, &["worktree", "add", "-q", "-b", branch, &path.display().to_string()]);
    drive(st, Msg::Repo(RepoMsg::Refresh));
    path
}

fn cleanup(dir: &Path, root: &Path) {
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(dir);
}

fn wt(m: WorktreeMsg) -> Msg {
    Msg::Worktree(m)
}

fn entry(st: &AppState, path: &Path) -> Option<WorktreeInfo> {
    find(st.snap.as_ref().unwrap(), path).cloned()
}

fn remove_form(st: &mut AppState) -> &mut RemoveForm {
    match st.modal.as_mut() {
        Some(Modal::RemoveWorktree(f)) => f,
        _ => panic!("the Remove worktree dialog is not open"),
    }
}

fn last_log(st: &AppState) -> (bool, String) {
    st.log.last().map(|l| (l.ok, l.text.clone())).expect("a log entry")
}

fn last_toast(st: &AppState) -> (bool, String) {
    st.toasts.last().map(|t| (t.ok, t.text.clone())).expect("a toast")
}

fn has_branch(st: &AppState, b: &str) -> bool {
    st.snap.as_ref().unwrap().locals.iter().any(|l| l.name == b)
}

fn job(dir: &Path, label: &str) -> RunningJob {
    RunningJob {
        label: label.into(),
        workdir: Some(dir.to_path_buf()),
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        keeps_undo: true,
    }
}

#[test]
fn worktree_commands_keep_the_undo_history() {
    let keeps = |a: &[&str]| super::undo::keeps_history(&[trellis_core::gitcli::args(a)]);
    assert!(keeps(&["worktree", "remove", "--force", "C:/w/x"]));
    assert!(keeps(&["worktree", "lock", "--reason", "usb", "C:/w/x"]));
    assert!(keeps(&["worktree", "unlock", "C:/w/x"]));
    assert!(keeps(&["worktree", "prune", "--verbose"]));
}

#[test]
fn remove_clean_and_delete_branch() {
    let (mut st, dir, root) = opened("wtm-clean");
    let linked = add(&mut st, &dir, &root, "wt1");
    drive(&mut st, wt(WorktreeMsg::RemoveDialog(linked.clone())));
    let f = remove_form(&mut st);
    assert_eq!((f.refusal.clone(), f.branch.as_deref()), (None, Some("wt1")));
    f.delete_branch = true;
    assert_eq!(f.button(), "Remove and delete branch");
    drive(&mut st, wt(WorktreeMsg::Remove));
    assert!(st.modal.is_none());
    assert!(!linked.exists());
    assert_eq!(st.snap.as_ref().unwrap().worktrees.len(), 1);
    assert!(!has_branch(&st, "wt1"));
    assert_eq!(last_toast(&st), (true, "Removed worktree wt1 and deleted branch wt1".to_owned()));
    assert_eq!(st.undo.undo.len(), 1, "the branch delete is undoable");

    drive(&mut st, Msg::Undo);
    assert!(has_branch(&st, "wt1"), "undo restored the branch");
    cleanup(&dir, &root);
}

#[test]
fn remove_dirty_offers_force() {
    let (mut st, dir, root) = opened("wtm-dirty");
    let linked = add(&mut st, &dir, &root, "dirty");
    std::fs::write(linked.join("new.txt"), "x\n").unwrap();
    drive(&mut st, wt(WorktreeMsg::RemoveDialog(linked.clone())));
    drive(&mut st, wt(WorktreeMsg::Remove));
    assert!(linked.join("new.txt").exists());
    let (ok, text) = last_log(&st);
    assert!(!ok && text.contains(&needs_force_text("dirty", &Refusal::Changes(1))), "{text}");
    assert!(text.contains("dirty wasn't removed: it has 1 changed or untracked files"), "{text}");
    assert!(!text.contains("$ git"), "no git ran: {text}");
    let f = remove_form(&mut st);
    assert_eq!(f.refusal, Some(Refusal::Changes(1)));
    assert_eq!(f.button(), "Force remove");

    drive(&mut st, wt(WorktreeMsg::Remove));
    assert!(!linked.exists() && entry(&st, &linked).is_none());
    assert!(last_log(&st).1.contains("worktree remove --force"), "{}", last_log(&st).1);
    cleanup(&dir, &root);
}

#[test]
fn remove_locked_opens_in_force_mode() {
    let (mut st, dir, root) = opened("wtm-locked");
    let linked = add(&mut st, &dir, &root, "usb");
    git(&dir, &["worktree", "lock", "--reason", "usb", &linked.display().to_string()]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, wt(WorktreeMsg::RemoveDialog(linked.clone())));
    assert_eq!(remove_form(&mut st).refusal, Some(Refusal::Locked("usb".into())));
    assert_eq!(remove_form(&mut st).force(), RemoveForce::Lock);
    drive(&mut st, wt(WorktreeMsg::Remove));
    assert!(last_log(&st).1.contains("worktree remove --force --force"), "{}", last_log(&st).1);
    assert!(entry(&st, &linked).is_none() && !linked.exists());
    cleanup(&dir, &root);
}

#[test]
fn locked_after_the_dialog_opened_asks_again() {
    let (mut st, dir, root) = opened("wtm-locked-late");
    let linked = add(&mut st, &dir, &root, "late");
    drive(&mut st, wt(WorktreeMsg::RemoveDialog(linked.clone())));
    git(&dir, &["worktree", "lock", &linked.display().to_string()]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(update(&mut st, wt(WorktreeMsg::Remove), Instant::now()).is_empty(), "force is confirmed first");
    assert_eq!(remove_form(&mut st).refusal, Some(Refusal::Locked(String::new())));
    assert!(linked.exists());
    cleanup(&dir, &root);
}

#[test]
fn branch_delete_failure_reported_separately() {
    let (mut st, dir, root) = opened("wtm-unmerged");
    let linked = add(&mut st, &dir, &root, "wip");
    git(&linked, &["commit", "--allow-empty", "-q", "-m", "unmerged"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, wt(WorktreeMsg::RemoveDialog(linked.clone())));
    remove_form(&mut st).delete_branch = true;
    let undo_before = st.undo.undo.len();
    drive(&mut st, wt(WorktreeMsg::Remove));
    assert!(entry(&st, &linked).is_none() && !linked.exists());
    assert!(has_branch(&st, "wip"));
    let (ok, toast) = last_toast(&st);
    assert!(!ok && toast.starts_with("Removed worktree wip, but branch wip wasn't deleted: "), "{toast}");
    assert!(toast.contains("not fully merged"), "{toast}");
    assert!(!last_log(&st).0);
    assert_eq!(st.undo.undo.len(), undo_before, "no undo entry");
    cleanup(&dir, &root);
}

#[test]
fn folder_left_and_branch_failed() {
    let (mut st, dir, root) = opened("wtm-left");
    let linked = root.join("busy");
    st.job = Some(job(&dir, "Remove worktree"));
    let request = RemoveRequest {
        repo: dir.clone(),
        path: linked.clone(),
        name: "busy".into(),
        force: RemoveForce::None,
        delete_branch: Some("busy".into()),
    };
    let msg = WorktreeMsg::Removed {
        request,
        outcome: RemoveOutcome::FolderLeft,
        output: "$ git worktree remove x\nerror: failed to delete".into(),
        branch: Some(Err("not fully merged".into())),
        undo: None,
    };
    update(&mut st, wt(msg), Instant::now());
    assert!(st.job.is_none());
    let n = st.log.len();
    let left = format!(
        "Removed worktree busy from git, but its folder {} couldn't be fully deleted. Close programs using it, then delete the folder yourself.",
        linked.display()
    );
    assert!(!st.log[n - 2].ok && st.log[n - 2].text.contains(&left), "{}", st.log[n - 2].text);
    assert_eq!(st.log[n - 1].text, "Removed worktree busy, but branch busy wasn't deleted: not fully merged");
    assert_eq!(last_toast(&st), (false, left));
    cleanup(&dir, &root);
}

#[test]
fn refusals() {
    let (mut st, dir, root) = opened("wtm-refusals");
    let linked = add(&mut st, &dir, &root, "side");
    for m in [WorktreeMsg::RemoveDialog(dir.clone()), WorktreeMsg::LockDialog(dir.clone())] {
        drive(&mut st, wt(m));
        assert!(st.modal.is_none());
        assert!(last_log(&st).1.contains("The main worktree can't be"), "{}", last_log(&st).1);
    }

    // Open in this tab, then in another tab.
    drive(&mut st, wt(WorktreeMsg::Open(linked.clone())));
    assert_eq!(st.active_tab, 1);
    for m in [WorktreeMsg::RemoveDialog(linked.clone()), WorktreeMsg::LockDialog(linked.clone())] {
        drive(&mut st, wt(m));
        assert!(st.modal.is_none());
        assert!(last_log(&st).1.ends_with("This worktree is open in this tab."), "{}", last_log(&st).1);
    }
    drive(&mut st, Msg::SelectTab(0));
    drive(&mut st, wt(WorktreeMsg::RemoveDialog(linked.clone())));
    assert!(st.modal.is_none());
    assert!(last_log(&st).1.ends_with("It is open in another tab. Close that tab first."));
    let form = RemoveForm::new(&entry(&st, &linked).unwrap());
    st.modal = Some(Modal::RemoveWorktree(form.clone()));
    assert!(update(&mut st, wt(WorktreeMsg::Remove), Instant::now()).is_empty());
    assert!(last_log(&st).1.ends_with("It is open in another tab. Close that tab first."));
    assert!(st.modal.is_some(), "the dialog stays");

    // Without the other tab, a running job still blocks, and keeps the dialog.
    st.tabs.truncate(1);
    st.job = Some(job(&dir, "Fetch"));
    assert!(update(&mut st, wt(WorktreeMsg::Remove), Instant::now()).is_empty());
    assert_eq!(last_log(&st).1, "Another operation is still running.");
    assert!(st.modal.is_some());
    st.job = None;

    // The worktree vanished.
    st.modal = Some(Modal::RemoveWorktree(RemoveForm { path: root.join("nope"), ..form }));
    assert!(update(&mut st, wt(WorktreeMsg::Remove), Instant::now()).is_empty());
    assert!(last_log(&st).1.ends_with("is no longer a worktree of this repository."));
    assert!(st.modal.is_none());
    st.modal = Some(Modal::LockWorktree(LockForm { path: root.join("nope"), name: "nope".into(), reason: String::new() }));
    assert!(update(&mut st, wt(WorktreeMsg::Lock), Instant::now()).is_empty());
    assert!(last_log(&st).1.ends_with("is no longer a worktree of this repository."));
    assert!(st.modal.is_none());
    cleanup(&dir, &root);
}

#[test]
fn lock_unlock() {
    let (mut st, dir, root) = opened("wtm-lock");
    let linked = add(&mut st, &dir, &root, "side");
    drive(&mut st, wt(WorktreeMsg::LockDialog(linked.clone())));
    match st.modal.as_mut() {
        Some(Modal::LockWorktree(f)) => f.reason = "on usb".into(),
        _ => panic!("the Lock dialog is not open"),
    }
    drive(&mut st, wt(WorktreeMsg::Lock));
    assert!(st.modal.is_none());
    assert_eq!(entry(&st, &linked).unwrap().locked.as_deref(), Some("on usb"));
    drive(&mut st, wt(WorktreeMsg::LockDialog(linked.clone())));
    assert!(st.modal.is_none() && last_log(&st).1.ends_with("It is already locked."));

    drive(&mut st, wt(WorktreeMsg::Unlock(linked.clone())));
    assert_eq!(entry(&st, &linked).unwrap().locked, None);

    drive(&mut st, wt(WorktreeMsg::LockDialog(linked.clone())));
    drive(&mut st, wt(WorktreeMsg::Lock));
    assert_eq!(entry(&st, &linked).unwrap().locked.as_deref(), Some(""), "a blank reason sends none");
    drive(&mut st, wt(WorktreeMsg::Unlock(linked.clone())));

    assert!(update(&mut st, wt(WorktreeMsg::Unlock(linked.clone())), Instant::now()).is_empty());
    assert!(last_log(&st).1.ends_with("It isn't locked."));
    assert!(st.job.is_none());
    cleanup(&dir, &root);
}

#[test]
fn prune() {
    let (mut st, dir, root) = opened("wtm-prune");
    let gone = add(&mut st, &dir, &root, "gone");
    let kept = add(&mut st, &dir, &root, "kept");
    git(&dir, &["worktree", "lock", &kept.display().to_string()]);
    std::fs::remove_dir_all(&gone).unwrap();
    std::fs::remove_dir_all(&kept).unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(entry(&st, &gone).unwrap().prunable);
    assert!(!entry(&st, &kept).unwrap().prunable);

    drive(&mut st, wt(WorktreeMsg::Prune));
    let log = st.log.iter().rev().find(|l| l.text.starts_with("Prune worktrees")).map(|l| l.text.clone()).unwrap();
    assert!(log.contains("$ git worktree prune --verbose"), "{log}");
    assert!(entry(&st, &gone).is_none());
    assert!(entry(&st, &kept).is_some(), "a locked missing worktree survives");

    assert!(update(&mut st, wt(WorktreeMsg::Prune), Instant::now()).is_empty());
    let why = prune_blocked(st.snap.as_ref().unwrap()).unwrap();
    assert_eq!(last_log(&st).1, format!("Can't prune: {why}"), "the refusal uses the disabled menu item's words");
    assert!(why.starts_with("No worktree records to prune"), "{why}");
    cleanup(&dir, &root);
}
