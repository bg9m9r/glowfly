//! Headless tests for Remove worktree's extras: deleting the branch on its remote too (merged:
//! all three go; not merged: offered disabled and a stale request refused; a failed push:
//! reported on its own, the local delete still undoable), and a worktree open in another tab
//! (that tab closes first, also when it was opened after the dialog, and nothing closes while
//! another job runs).

use super::branch_tests::with_origin;
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::effect::WorktreeEffect;
use crate::msg::{Msg, RepoMsg, TabsMsg};
use crate::state::worktree_manage::RemoveForm;
use crate::state::{AppState, Modal, RunningJob};
use crate::worktree::WorktreeMsg;
use glowfly_core::gitcli::{args, run};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn wt(m: WorktreeMsg) -> Msg {
    Msg::Worktree(m)
}

fn root_of(dir: &Path) -> PathBuf {
    dir.with_file_name(format!("{}-worktrees", dir.file_name().unwrap().to_string_lossy()))
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(root_of(d));
        let _ = std::fs::remove_dir_all(d);
    }
}

fn form(st: &mut AppState) -> &mut RemoveForm {
    match st.modal.as_mut() {
        Some(Modal::RemoveWorktree(f)) => f,
        _ => panic!("the Remove worktree dialog is not open: {:?}", st.log.last().map(|l| &l.text)),
    }
}

fn last_toast(st: &AppState) -> (bool, String) {
    st.toasts.last().map(|t| (t.ok, t.text.clone())).expect("a toast")
}

fn has_branch(st: &AppState, b: &str) -> bool {
    st.snap.as_ref().unwrap().locals.iter().any(|l| l.name == b)
}

fn on_remote(bare: &Path, b: &str) -> bool {
    let (ok, out) = run(bare, &args(&["ls-remote", "--heads", &bare.display().to_string(), b]));
    assert!(ok, "{out}");
    !out.trim().is_empty()
}

/// `with_origin` with `feature` (pushed, tracked, merged into main) checked out in a linked
/// worktree, and the Remove dialog open for it with both deletes ticked.
fn feature_worktree(name: &str) -> (AppState, PathBuf, PathBuf, PathBuf) {
    let (mut st, work, bare) = with_origin(name);
    let linked = root_of(&work).join("feature");
    let _ = std::fs::remove_dir_all(root_of(&work));
    git(&work, &["worktree", "add", "-q", &linked.display().to_string(), "feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, wt(WorktreeMsg::RemoveDialog(linked.clone())));
    (st, work, bare, linked)
}

fn tick_both(st: &mut AppState) {
    let f = form(st);
    f.delete_branch = true;
    f.delete_remote = true;
}

#[test]
fn removes_the_worktree_and_both_branches() {
    let (mut st, work, bare, linked) = feature_worktree("wtrr-both");
    let f = form(&mut st);
    let remote = f.remote.clone().expect("the upstream is offered");
    assert_eq!((remote.tracking.display(), remote.blocked), ("origin/feature".to_owned(), None));
    tick_both(&mut st);
    let f = form(&mut st);
    assert_eq!(f.button(), "Remove and delete local and remote branch");
    assert_eq!(f.remote_note().as_deref(), Some("feature is deleted on origin for everyone who uses it."));
    drive(&mut st, wt(WorktreeMsg::Remove));
    assert!(st.modal.is_none());
    assert!(!linked.exists() && !has_branch(&st, "feature"));
    assert!(!on_remote(&bare, "feature"), "deleted on origin");
    assert_eq!(last_toast(&st), (true, "Removed worktree feature and deleted branch feature and origin/feature".to_owned()));
    let log = &st.log.last().unwrap().text;
    assert!(log.contains("merge-base --is-ancestor refs/heads/feature HEAD") && log.contains("push origin --delete feature"), "{log}");
    assert_eq!(st.undo.undo.len(), 1, "the local delete is undoable");
    cleanup(&[&work, &bare]);
}

#[test]
fn an_unmerged_branch_keeps_its_remote_copy() {
    let (mut st, work, bare, linked) = feature_worktree("wtrr-unmerged");
    st.modal = None;
    git(&linked, &["commit", "--allow-empty", "-q", "-m", "only here"]);
    git(&linked, &["push", "-q", "origin", "feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, wt(WorktreeMsg::RemoveDialog(linked.clone())));
    let why = form(&mut st).remote.as_ref().and_then(|r| r.blocked.clone()).expect("offered but blocked");
    assert!(why.contains("aren't merged into main"), "{why}");
    tick_both(&mut st);
    assert_eq!(form(&mut st).button(), "Remove and delete branch", "a blocked remote delete isn't asked for");

    // A dialog opened before the commit still offers it: update refuses.
    form(&mut st).remote.as_mut().unwrap().blocked = None;
    assert!(update(&mut st, wt(WorktreeMsg::Remove), Instant::now()).is_empty());
    assert_eq!(st.log.last().unwrap().text, why);
    assert!(form(&mut st).remote.as_ref().unwrap().blocked.is_some(), "the dialog shows it blocked now");
    assert!(linked.exists() && on_remote(&bare, "feature"));
    cleanup(&[&work, &bare]);
}

#[test]
fn a_failed_push_is_reported_and_the_local_delete_stays_undoable() {
    let (mut st, work, bare, linked) = feature_worktree("wtrr-push-fails");
    tick_both(&mut st);
    let broken = work.with_file_name("glowfly-no-such-remote.git");
    git(&work, &["remote", "set-url", "origin", &broken.display().to_string()]);
    drive(&mut st, wt(WorktreeMsg::Remove));
    assert!(!linked.exists() && !has_branch(&st, "feature"));
    let (ok, toast) = last_toast(&st);
    assert!(
        !ok && toast.starts_with("Removed worktree feature and deleted branch feature, but origin/feature wasn't deleted: "),
        "{toast}"
    );
    assert!(!st.log.last().unwrap().ok);
    assert!(on_remote(&bare, "feature"));
    assert_eq!(st.undo.undo.len(), 1, "the local delete is undoable");
    cleanup(&[&work, &bare]);
}

fn job(dir: &Path) -> RunningJob {
    RunningJob {
        label: "Fetch".into(),
        workdir: Some(dir.to_path_buf()),
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        then_pr: None,
        then_link: false,
        keeps_undo: true,
        fetches: false,
    }
}

#[test]
fn remove_closes_the_other_tab_first() {
    let dir = scratch_repo("wtrr-tabs");
    let linked = root_of(&dir).join("side");
    let _ = std::fs::remove_dir_all(root_of(&dir));
    git(&dir, &["worktree", "add", "-q", "-b", "side", &linked.display().to_string()]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, wt(WorktreeMsg::RemoveDialog(linked.clone())));
    assert!(!form(&mut st).close_tabs);
    assert_eq!(form(&mut st).close_note(), None);
    let dialog = st.modal.take();

    // Opened in another tab after the dialog: the first Remove only says so.
    drive(&mut st, wt(WorktreeMsg::Open(linked.clone())));
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert_eq!(st.tabs.len(), 2);
    st.modal = dialog;
    assert!(update(&mut st, wt(WorktreeMsg::Remove), Instant::now()).is_empty());
    assert_eq!(st.log.last().unwrap().text, "side is now open in another tab. Remove closes it.");
    assert!(form(&mut st).close_tabs && form(&mut st).close_note().is_some());

    // A running job closes nothing.
    st.job = Some(job(&dir));
    assert!(update(&mut st, wt(WorktreeMsg::Remove), Instant::now()).is_empty());
    assert_eq!(st.log.last().unwrap().text, "Another operation is still running.");
    assert_eq!(st.tabs.len(), 2);
    st.job = None;

    let fx = update(&mut st, wt(WorktreeMsg::Remove), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::Worktree(WorktreeEffect::Remove(_)))), "{fx:?}");
    assert_eq!((st.tabs.len(), st.active_tab), (1, 0));
    assert!(st.closed_tabs.iter().any(|c| super::tests::same_place(&c.path, &linked)), "Reopen closed tab has it");
    super::tests::finish(&mut st, fx);
    assert!(!linked.exists());

    // Open in another tab when the dialog opens: it says so from the start.
    let other = root_of(&dir).join("other");
    git(&dir, &["worktree", "add", "-q", "-b", "other", &other.display().to_string()]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, wt(WorktreeMsg::Open(other.clone())));
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    drive(&mut st, wt(WorktreeMsg::RemoveDialog(other.clone())));
    assert!(form(&mut st).close_tabs);
    drive(&mut st, wt(WorktreeMsg::Remove));
    assert_eq!(st.tabs.len(), 1);
    assert!(!other.exists());
    cleanup(&[&dir]);
}
