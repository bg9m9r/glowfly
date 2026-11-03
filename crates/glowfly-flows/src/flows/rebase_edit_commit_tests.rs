//! Headless tests for the commit form at a rebase stopped to edit a commit: Amend ticked once per
//! stop and the draft filled from the commit, Amend and continue amending and finishing the
//! rebase in one go, Commit and continue with Amend unticked, an amend that would change nothing
//! refused, and Amend unticked again once the rebase is over.

use super::tests::{drive, git, scratch_repo, state};
use crate::msg::{CommitMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::commit::{commit_blocked, edit_stop};
use git2::RepositoryState;
use glowfly_core::gitcli::{args, run, run_with_input};
use std::path::{Path, PathBuf};

/// A repository with "first" and "second", stopped by an interactive rebase to edit "second".
pub(super) fn stopped_at_edit(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["commit", "-q", "-am", "second"]);
    // git runs the sequence editor through sh with the todo file as its argument.
    let editor = "f() { sed 's/^pick/edit/' \"$1\" > \"$1.tmp\" && mv \"$1.tmp\" \"$1\"; }; f";
    let (ok, out) = run_with_input(&dir, &args(&["rebase", "-i", "HEAD~1"]), "", &[("GIT_SEQUENCE_EDITOR", editor)]);
    assert!(ok, "{out}");
    dir
}

fn open(dir: &Path) -> AppState {
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())));
    st
}

fn log(dir: &Path) -> Vec<String> {
    let (ok, out) = run(dir, &args(&["log", "--format=%s"]));
    assert!(ok, "{out}");
    out.lines().filter(|l| !l.starts_with("$ ") && !l.trim().is_empty()).map(|l| l.trim().to_owned()).collect()
}

fn state_of(st: &AppState) -> RepositoryState {
    st.snap.as_ref().unwrap().state
}

#[test]
fn amend_and_continue_amends_the_commit_and_finishes_the_rebase() {
    let dir = stopped_at_edit("edit-stop-amend");
    let mut st = open(&dir);
    assert!(edit_stop(st.snap.as_ref().unwrap()).is_some(), "stopped to edit");
    let d = &st.session.draft;
    assert!(d.amend, "Amend is ticked at a stop to edit");
    assert_eq!(d.summary, "second", "the draft holds the commit's message");
    assert!(commit_blocked(&st, false).is_some(), "nothing staged and the message unchanged: nothing to amend");

    std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    git(&dir, &["add", "a.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.session.draft.amend, "still ticked: the same stop");
    st.session.draft.summary = "second, amended".into();
    drive(&mut st, Msg::Commit(CommitMsg::CommitAndContinue));

    assert_eq!(state_of(&st), RepositoryState::Clean, "the rebase went on and finished");
    assert_eq!(log(&dir), ["second, amended", "first"], "amended, not a new commit");
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap().replace('\r', ""), "one\ntwo\nthree\n");
    assert!(!st.session.draft.amend && st.session.draft.amend_stop.is_none(), "Amend is off again");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn commit_and_continue_adds_a_commit_when_amend_is_unticked() {
    let dir = stopped_at_edit("edit-stop-commit");
    let mut st = open(&dir);
    st.session.draft.amend = false;
    st.session.draft.summary = "inserted".into();
    st.session.draft.description.clear();
    std::fs::write(dir.join("b.txt"), "new\n").unwrap();
    git(&dir, &["add", "b.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(!st.session.draft.amend, "unticked stays unticked at the same stop");
    drive(&mut st, Msg::Commit(CommitMsg::CommitAndContinue));
    assert_eq!(state_of(&st), RepositoryState::Clean);
    assert_eq!(log(&dir), ["inserted", "second", "first"]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn commit_and_continue_does_nothing_outside_a_stop_to_edit() {
    let dir = scratch_repo("edit-stop-none");
    git(&dir, &["add", "a.txt"]);
    let mut st = open(&dir);
    assert!(!st.session.draft.amend, "no stop, no Amend");
    st.session.draft.summary = "plain".into();
    drive(&mut st, Msg::Commit(CommitMsg::CommitAndContinue));
    assert_eq!(log(&dir), ["first"], "nothing committed");
    let _ = std::fs::remove_dir_all(dir);
}
