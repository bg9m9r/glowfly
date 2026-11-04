//! Headless tests for the commit box's states (design pass D11): in each, the button's label,
//! and the caption naming only the first missing step, which is also what a refused commit
//! toasts; and the button menu's Commit and push.

use super::tests::state;
use super::update;
use crate::keymap::BindingId;
use crate::msg::{CommitMsg, Msg};
use crate::state::AppState;
use crate::state::commit::{commit_blocked, commit_label};
use crate::state::op_fixture::{rows, snapshot};
use git2::RepositoryState;
use std::time::Instant;

/// A loaded one-commit repository named `name`, with `staged` and `unstaged` rows.
fn box_state(name: &str, staged: &[(&str, char)], unstaged: &[(&str, char)]) -> AppState {
    let mut st = state();
    let mut snap = snapshot(name);
    snap.staged = rows(staged);
    snap.unstaged = rows(unstaged);
    st.snap = Some(snap);
    st
}

fn label(st: &AppState) -> String {
    commit_label(st)
}

/// Commits, and returns the refusal's toast (None when it started a job).
fn refusal(st: &mut AppState) -> Option<String> {
    let fx = update(st, Msg::Commit(CommitMsg::Commit), Instant::now());
    if fx.is_empty() { st.toasts.last().map(|t| t.text.clone()) } else { None }
}

#[test]
fn clean_tree() {
    let mut st = box_state("box-clean", &[], &[]);
    st.session.draft.summary = "Work".into();
    let want = "Nothing to commit. The working tree is clean.";
    assert_eq!((label(&st).as_str(), commit_blocked(&st, false).as_deref()), ("Commit", Some(want)));
    assert_eq!(commit_blocked(&st, true).as_deref(), Some(want), "nothing to stage either");
    assert_eq!(refusal(&mut st).as_deref(), Some(want));
}

#[test]
fn changes_none_staged() {
    let mut st = box_state("box-unstaged", &[], &[("a.txt", 'M')]);
    let key = st.keymap.hint(BindingId::StageAllAndCommit).expect("a default key").to_owned();
    let want = format!("Stage changes to commit, or press {key} to stage all and commit.");
    // Only the first missing step: staging comes before the empty summary.
    assert_eq!((label(&st).as_str(), commit_blocked(&st, false)), ("Commit", Some(want.clone())));
    assert_eq!(refusal(&mut st), Some(want));
    // Staging everything would mark a conflicted file resolved: the caption doesn't offer it.
    st.snap.as_mut().unwrap().unstaged = rows(&[("a.txt", '!')]);
    assert_eq!(commit_blocked(&st, false).as_deref(), Some("Stage changes to commit."));
}

#[test]
fn staged_no_summary() {
    let mut st = box_state("box-no-summary", &[("a.txt", 'M')], &[]);
    st.settings.push_after_commit = true;
    let want = "Write a summary to commit.";
    assert_eq!((label(&st).as_str(), commit_blocked(&st, false).as_deref()), ("Commit", Some(want)), "no push suffix while disabled");
    assert_eq!(refusal(&mut st).as_deref(), Some(want));
}

#[test]
fn ready() {
    let mut st = box_state("box-ready", &[("a.txt", 'M'), ("b.txt", 'A')], &[]);
    st.session.draft.summary = "Work".into();
    assert_eq!((label(&st).as_str(), commit_blocked(&st, false)), ("Commit 2 files", None));
    assert_eq!(refusal(&mut st), None, "it commits");
    assert!(!st.job.as_ref().unwrap().then_push);
}

#[test]
fn ready_with_push() {
    let mut st = box_state("box-ready-push", &[("a.txt", 'M'), ("b.txt", 'A')], &[]);
    st.session.draft.summary = "Work".into();
    st.settings.push_after_commit = true;
    assert_eq!((label(&st).as_str(), commit_blocked(&st, false)), ("Commit 2 files and push", None));
    assert_eq!(refusal(&mut st), None, "it commits");
    assert!(st.job.as_ref().unwrap().then_push);
}

#[test]
fn amend() {
    let mut st = box_state("box-amend", &[], &[]);
    st.session.draft.amend = true;
    assert_eq!(label(&st), "Amend last commit", "names the action even before a summary");
    assert_eq!(commit_blocked(&st, false).as_deref(), Some("Write a summary to commit."), "an amend needs nothing staged");
    st.session.draft.summary = "Work".into();
    assert_eq!((label(&st).as_str(), commit_blocked(&st, false)), ("Amend last commit", None));
    assert_eq!(refusal(&mut st), None, "it amends");
}

#[test]
fn merge() {
    let mut st = box_state("box-merge", &[], &[]);
    st.snap.as_mut().unwrap().state = RepositoryState::Merge;
    st.session.draft.summary = "Merge branch 'feature'".into();
    assert_eq!((label(&st).as_str(), commit_blocked(&st, false)), ("Commit merge", None), "a merge commits with nothing staged");
}

#[test]
fn commit_and_push_pushes_with_the_setting_off() {
    let mut st = box_state("box-commit-push", &[("a.txt", 'M')], &[]);
    st.session.draft.summary = "Work".into();
    assert!(!st.settings.push_after_commit);
    assert!(!update(&mut st, Msg::Commit(CommitMsg::CommitAndPush), Instant::now()).is_empty());
    assert!(st.job.as_ref().unwrap().then_push, "a one-off push");
    assert!(!st.settings.push_after_commit, "the setting stays off");

    // Nothing to push from a detached HEAD.
    let mut st = box_state("box-commit-push-detached", &[("a.txt", 'M')], &[]);
    st.session.draft.summary = "Work".into();
    st.snap.as_mut().unwrap().head_branch = None;
    assert!(update(&mut st, Msg::Commit(CommitMsg::CommitAndPush), Instant::now()).is_empty());
    assert!(st.job.is_none());
}
