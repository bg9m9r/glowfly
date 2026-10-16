//! Interactive rebases end to end: the panel's list run against real repositories, finishing,
//! stopping at an edit step, and stopping on a conflict, then the banner's actions.

use super::rebase_tests::{repo, with_panel};
use super::selection_tests::{commit_on, git, head};
use super::tests::{drive, state};
use crate::msg::Msg;
use crate::msg::RebaseMsg;
use crate::msg::RepoMsg;
use crate::state::{AppState, Selection};
use git2::{Oid, RepositoryState};
use std::path::Path;
use trellis_core::rebase::{RebaseAction, RebaseBase};

fn short(o: Oid) -> String {
    o.to_string()[..7].to_owned()
}

fn log(dir: &Path) -> Vec<String> {
    git(dir, &["log", "--format=%s"]).lines().map(str::to_owned).collect()
}

fn repo_state(st: &AppState) -> RepositoryState {
    st.snap.as_ref().unwrap().state
}

fn last_toast(st: &AppState) -> (bool, &str) {
    st.toasts.last().map(|t| (t.ok, t.text.as_str())).unwrap_or((false, ""))
}

/// Runs the banner action labelled `label` for the operation in progress.
fn banner_action(st: &mut AppState, label: &str) {
    let (_, actions) = crate::ui::common::state_actions(repo_state(st)).expect("an operation in progress");
    let (l, a) = actions.iter().find(|(l, _)| *l == label).expect("the action is offered");
    drive(st, crate::state::rebase_banner::action_msg(repo_state(st), l, a));
}

#[test]
fn reorder_reword_and_drop() {
    let (mut st, dir, c) = with_panel("rebase-e2e-finish", RebaseBase::FromCommit(Oid::zero()));
    drive(&mut st, Msg::Rebase(RebaseMsg::RebaseMove { from: 2, to: 0 }));
    drive(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 1, action: RebaseAction::Reword }));
    st.rebase.as_mut().unwrap().draft = "c1, reworded\n\nWith a body.".into();
    drive(&mut st, Msg::Rebase(RebaseMsg::RebaseSaveMessage));
    drive(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 2, action: RebaseAction::Drop }));
    drive(&mut st, Msg::Rebase(RebaseMsg::StartRebase { confirmed: false, autostash: false }));
    assert!(st.modal.is_some(), "dropping asks first");
    drive(&mut st, Msg::ConfirmAccepted);

    assert_eq!(last_toast(&st), (true, "Interactive rebase succeeded"));
    assert!(st.rebase.is_none() && st.job.is_none());
    assert_eq!(log(&dir), ["c1, reworded", "c3", "first"]);
    assert!(!dir.join("c2.txt").exists());
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_ne!(head(&dir), c[2]);
    assert_eq!(st.snap.as_ref().unwrap().commits[0].summary, "c1, reworded", "the repository refreshed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_edit_step_pauses_and_continue_finishes() {
    let (mut st, dir, c) = with_panel("rebase-e2e-edit", RebaseBase::FromCommit(Oid::zero()));
    drive(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 0, action: RebaseAction::Edit }));
    drive(&mut st, Msg::Rebase(RebaseMsg::StartRebase { confirmed: false, autostash: false }));

    assert_eq!(repo_state(&st), RepositoryState::RebaseInteractive);
    assert!(st.rebase.is_none());
    assert_eq!(st.selection, Selection::Wip, "the banner is in view");
    let text = format!("Interactive rebase paused at {} (step 1 of 3) so you can edit it. Use Continue when you're done.", short(c[0]));
    assert_eq!(last_toast(&st), (true, text.as_str()));
    let (name, actions) = crate::ui::common::state_actions(repo_state(&st)).unwrap();
    assert_eq!(name, "Interactive rebase");
    assert_eq!(actions.iter().map(|(l, _)| *l).collect::<Vec<_>>(), ["Continue", "Skip commit", "Abort rebase"]);

    banner_action(&mut st, "Continue");
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!(log(&dir), ["c3", "c2", "c1", "first"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_conflict_stops_and_abort_restores() {
    let (_, dir, _) = repo("rebase-e2e-conflict");
    std::fs::write(dir.join("f.txt"), "base\n").unwrap();
    commit_on(&dir, "base", 10);
    std::fs::write(dir.join("f.txt"), "one\n").unwrap();
    let a = commit_on(&dir, "a", 11);
    std::fs::write(dir.join("f.txt"), "two\n").unwrap();
    let b = commit_on(&dir, "b", 12);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::FromCommit(a))));
    drive(&mut st, Msg::Rebase(RebaseMsg::RebaseMove { from: 1, to: 0 }));
    drive(&mut st, Msg::Rebase(RebaseMsg::StartRebase { confirmed: false, autostash: false }));

    let text = format!("Interactive rebase stopped on a conflict at {} (step 1 of 2). Resolve it, then Continue, or Abort.", short(b));
    assert_eq!(last_toast(&st), (false, text.as_str()));
    assert!(st.rebase.is_none());
    assert_eq!(st.selection, Selection::Wip);
    assert_eq!(repo_state(&st), RepositoryState::RebaseInteractive);
    assert!(st.snap.as_ref().unwrap().unstaged.iter().any(|f| f.status == '!' && f.path == "f.txt"), "the banner lists the conflict");

    banner_action(&mut st, "Abort rebase");
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!(head(&dir), b, "back at the original HEAD");
    let _ = std::fs::remove_dir_all(&dir);
}
