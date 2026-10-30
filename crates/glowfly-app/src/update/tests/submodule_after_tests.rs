//! Headless tests for keeping submodules updated after a cherry-pick, a revert, a stash pop and
//! an undo or redo (a checkout's, Discard all's) that moves the recorded commit, and for leaving
//! them alone while a cherry-pick is stopped for conflicts, until Continue or a commit finishes
//! it (a commit in a sequence with picks left waits for the sequence's end).

use super::gitflow_tests;
use super::submodule_batch_tests::{after_job, checkout_other, checks_all, job_cmds, step, two_branches};
use super::submodule_tests::{PATH, cleanup, sub};
use super::tests::{drive, finish, git};
use super::update;
use crate::effect::{Effect, SubmoduleEffect};
use crate::msg::{JobMsg, Msg, RepoMsg, SelectionMsg};
use crate::state::AppState;
use crate::state::palette::StashAction;
use crate::state::rebase_banner::action_msg;
use crate::submodule::Batch;
use git2::{Oid, RepositoryState};
use glowfly_core::repo::SubmoduleState;
use glowfly_core::submodule_ops::batch_update_cmds;
use glowfly_core::undo::{Action, DISCARD_STASH_MESSAGE};
use std::path::Path;
use std::time::Instant;

fn rev(dir: &Path, what: &str) -> Oid {
    Oid::from_str(gitflow_tests::rev(dir, what).trim()).unwrap()
}

fn refresh(st: &mut AppState) {
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

/// The job `msg` starts succeeds and the update follows it: the submodule ends at the commit
/// the parent's index records.
fn updates_after(st: &mut AppState, msg: Msg) {
    let fx = after_job(st, msg);
    assert!(checks_all(&fx, Batch::Auto), "{fx:?}");
    let fx = step(st, fx);
    assert_eq!(job_cmds(st, &fx, "Update submodules"), Some(batch_update_cmds(None, false)));
    finish(st, fx);
    assert_eq!(sub(st).state, SubmoduleState::InSync);
}

#[test]
fn update_after_cherry_pick_and_revert() {
    let (mut st, parent, lib) = two_branches("smb-after-pick");
    st.settings.submodules.keep_updated = true;
    let (on_other, main_lib) = (rev(&parent, "other"), sub(&st).recorded);
    updates_after(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![on_other])));
    assert_eq!(sub(&st).checked_out, Some(rev(&parent, &format!("other:{PATH}"))), "the picked commit's");

    updates_after(&mut st, Msg::Selection(SelectionMsg::Revert(vec![rev(&parent, "HEAD")])));
    assert_eq!(sub(&st).checked_out, Some(main_lib), "back to main's");
    cleanup(&[&parent, &lib]);
}

/// Stages the submodule moved to other's commit with a change to `c.txt` (a gitlink alone is
/// no change to git stash), runs `stash` (git arguments) and puts the submodule back where main
/// records it. Returns other's commit of the submodule.
fn stash_newer(st: &mut AppState, parent: &Path, stash: &[&str]) -> Oid {
    let newer = rev(parent, &format!("other:{PATH}"));
    git(&parent.join(PATH), &["checkout", "-q", "--detach", &newer.to_string()]);
    std::fs::write(parent.join("c.txt"), "stashed\n").unwrap();
    git(parent, &["add", PATH, "c.txt"]);
    drive(st, Msg::git("Stash", stash));
    git(parent, &["submodule", "update", "-q", "--checkout", "--", PATH]);
    refresh(st);
    assert_eq!(sub(st).state, SubmoduleState::InSync);
    newer
}

#[test]
fn update_after_stash_pop() {
    let (mut st, parent, lib) = two_branches("smb-after-pop");
    st.settings.submodules.keep_updated = true;
    stash_newer(&mut st, &parent, &["stash", "push", "-q"]);
    let oid = st.snap.as_ref().unwrap().stashes[0].oid;
    // A plain pop puts back the files but leaves the recorded commit: the check finds nothing.
    let fx = after_job(&mut st, Msg::Job(JobMsg::Stash { action: StashAction::Pop, index: 0, oid }));
    assert!(checks_all(&fx, Batch::Auto), "{fx:?}");
    let fx = step(&mut st, fx);
    assert!(st.job.is_none(), "{fx:?}");
    assert_eq!(sub(&st).state, SubmoduleState::InSync);
    cleanup(&[&parent, &lib]);
}

#[test]
fn update_after_undoing_discard_all() {
    let (mut st, parent, lib) = two_branches("smb-after-discard");
    st.settings.submodules.keep_updated = true;
    let newer = stash_newer(&mut st, &parent, &["stash", "push", "-u", "-m", DISCARD_STASH_MESSAGE]);
    assert!(matches!(st.session.undo.undo.last(), Some(Action::DiscardAll { .. })), "{:?}", st.session.undo.undo);
    // Its undo pops the stash with its index: the newer commit is recorded again.
    updates_after(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(sub(&st).checked_out, Some(newer));
    cleanup(&[&parent, &lib]);
}

#[test]
fn update_after_undo_and_redo_of_a_checkout() {
    let (mut st, parent, lib) = two_branches("smb-after-undo");
    st.settings.submodules.keep_updated = true;
    updates_after(&mut st, checkout_other());
    updates_after(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("main"));
    updates_after(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("other"));

    // A commit's undo is a soft reset: the index, and the commits it records, stay.
    drive(&mut st, Msg::git("Commit", &["commit", "-q", "--allow-empty", "-m", "x"]));
    let fx = after_job(&mut st, Msg::Job(JobMsg::Undo));
    assert!(!checks_all(&fx, Batch::Auto), "{fx:?}");
    assert!(st.submodules.after_job.is_none());
    cleanup(&[&parent, &lib]);
}

/// Commits `file` on the branch checked out in `parent`.
fn commit_file(parent: &Path, file: &str, text: &str) {
    std::fs::write(parent.join(file), text).unwrap();
    git(parent, &["add", file]);
    git(parent, &["commit", "-q", "-m", file]);
}

/// `two_branches` plus a branch `pick` whose commit moves the submodule to other's commit and
/// writes `c.txt`, which main also changes, with a plain commit before it (`before`) and after
/// it (`after`): picking them onto main stops for conflicts at that commit.
fn conflicting_picks(name: &str, before: bool, after: bool) -> (AppState, std::path::PathBuf, std::path::PathBuf) {
    let (mut st, parent, lib) = two_branches(name);
    st.settings.submodules.keep_updated = true;
    git(&parent, &["checkout", "-q", "-b", "pick"]);
    if before {
        commit_file(&parent, "before.txt", "before\n");
    }
    git(&parent.join(PATH), &["checkout", "-q", "--detach", &rev(&parent, &format!("other:{PATH}")).to_string()]);
    git(&parent, &["add", PATH]);
    commit_file(&parent, "c.txt", "pick\n");
    if after {
        commit_file(&parent, "after.txt", "after\n");
    }
    let picks: Vec<Oid> = (0..1 + usize::from(before) + usize::from(after)).rev().map(|n| rev(&parent, &format!("HEAD~{n}"))).collect();
    git(&parent, &["checkout", "-q", "main"]);
    git(&parent, &["submodule", "update", "-q", "--checkout", "--", PATH]);
    commit_file(&parent, "c.txt", "main\n");
    refresh(&mut st);
    assert_eq!(sub(&st).state, SubmoduleState::InSync);
    // Stops for conflicts: no update, and the submodule waits at main's commit.
    let fx = after_job(&mut st, Msg::Selection(SelectionMsg::CherryPick(picks)));
    assert!(!checks_all(&fx, Batch::Auto), "{fx:?}");
    finish(&mut st, fx);
    assert_ne!(st.snap.as_ref().unwrap().state, RepositoryState::Clean);
    assert_eq!(sub(&st).state, SubmoduleState::OutOfSync, "the stopped pick staged the newer commit");
    std::fs::write(parent.join("c.txt"), "both\n").unwrap();
    git(&parent, &["add", "c.txt"]);
    refresh(&mut st);
    (st, parent, lib)
}

fn conflicting_pick(name: &str) -> (AppState, std::path::PathBuf, std::path::PathBuf) {
    let (st, parent, lib) = conflicting_picks(name, false, false);
    assert_eq!(st.snap.as_ref().unwrap().state, RepositoryState::CherryPick);
    (st, parent, lib)
}

fn commit_no_edit() -> Msg {
    Msg::git("Commit", &["commit", "-q", "--no-edit"])
}

#[test]
fn a_commit_mid_sequence_waits_for_its_end() {
    let (mut st, parent, lib) = conflicting_picks("smb-after-seq-mid", false, true);
    let state = st.snap.as_ref().unwrap().state;
    assert_eq!(state, RepositoryState::CherryPickSequence);
    // Picks are left: the check after the commit reports nothing.
    let fx = after_job(&mut st, commit_no_edit());
    assert!(fx.iter().any(|e| matches!(e, Effect::Submodule(SubmoduleEffect::CheckAll { once_finished: true, .. }))), "{fx:?}");
    let fx = step(&mut st, fx);
    assert!(!fx.iter().any(|e| matches!(e, Effect::RunGit { .. })), "{fx:?}");
    finish(&mut st, fx);
    assert_eq!(st.snap.as_ref().unwrap().state, RepositoryState::CherryPickSequence);
    assert_eq!(sub(&st).state, SubmoduleState::OutOfSync);
    // Continue runs the rest and ends it: now the submodule follows.
    updates_after(&mut st, action_msg(state, "Continue", &["cherry-pick", "--continue"]));
    assert_eq!(st.snap.as_ref().unwrap().state, RepositoryState::Clean);
    cleanup(&[&parent, &lib]);
}

#[test]
fn a_commit_of_a_sequence_last_pick_updates() {
    let (mut st, parent, lib) = conflicting_picks("smb-after-seq-last", true, false);
    let fx = after_job(&mut st, commit_no_edit());
    let fx = step(&mut st, fx);
    assert_eq!(job_cmds(&st, &fx, "Update submodules"), Some(batch_update_cmds(None, false)), "the sequence ended");
    finish(&mut st, fx);
    assert_eq!(sub(&st).state, SubmoduleState::InSync);
    cleanup(&[&parent, &lib]);
}

#[test]
fn a_stopped_cherry_pick_updates_once_continued() {
    let (mut st, parent, lib) = conflicting_pick("smb-after-continue");
    let cont = action_msg(RepositoryState::CherryPick, "Continue", &["cherry-pick", "--continue"]);
    updates_after(&mut st, cont);
    assert_eq!(st.snap.as_ref().unwrap().state, RepositoryState::Clean);
    cleanup(&[&parent, &lib]);
}

#[test]
fn a_stopped_cherry_pick_updates_once_committed() {
    let (mut st, parent, lib) = conflicting_pick("smb-after-commit");
    updates_after(&mut st, commit_no_edit());
    assert_eq!(st.snap.as_ref().unwrap().state, RepositoryState::Clean);
    // A plain commit afterwards moves nothing.
    let fx = update(&mut st, Msg::git("Commit", &["commit", "-q", "--allow-empty", "-m", "y"]), Instant::now());
    assert!(st.submodules.after_job.is_none(), "{fx:?}");
    cleanup(&[&parent, &lib]);
}
