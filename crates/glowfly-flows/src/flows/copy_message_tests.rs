//! Copy > Message through the runner: a commit's whole message is read from the repository, so a
//! multi-line body is copied whole and without a trailing newline, a commit without a body copies
//! its summary, and a commit that is gone toasts and copies nothing.

use super::helpers::finish_keeping;
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use git2::Oid;
use glowfly_model::effect::{CommitEffect, Effect, UiEffect};
use glowfly_model::msg::{Msg, RepoMsg};
use glowfly_model::state::AppState;
use std::time::Instant;

/// Copy > Message on commit `oid`, run to the end; the text copied, if any.
fn copy_message(st: &mut AppState, oid: Oid) -> Option<String> {
    let fx = update(st, Msg::Repo(RepoMsg::CopyMessage(oid)), Instant::now());
    assert!(matches!(fx.as_slice(), [Effect::Commit(CommitEffect::CopyMessage { .. })]), "read from the repository: {fx:?}");
    finish_keeping(st, fx).into_iter().find_map(|e| if let Effect::Ui(UiEffect::CopyText(t)) = e { Some(t) } else { None })
}

#[test]
fn a_multi_line_body_is_copied_whole_without_a_trailing_newline() {
    let dir = scratch_repo("copy-message-body");
    std::fs::write(dir.join(".git").join("COPY_MSG"), "Add the second line\n\nWhy:\n- one\n- two\n\nSecond paragraph.\n").unwrap();
    git(&dir, &["commit", "-q", "-a", "-F", ".git/COPY_MSG"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let c = &st.snap.as_ref().unwrap().commits[0];
    let (oid, flattened) = (c.oid, c.body.to_string());
    let text = copy_message(&mut st, oid).expect("copied");
    assert_eq!(text, "Add the second line\n\nWhy:\n- one\n- two\n\nSecond paragraph.");
    assert!(!text.ends_with('\n'));
    assert_ne!(text, flattened);
    assert!(st.toasts.is_empty(), "{:?}", st.toasts.iter().map(|t| &t.text).collect::<Vec<_>>());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_commit_without_a_body_copies_its_summary() {
    let dir = scratch_repo("copy-message-plain");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let c = st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary.as_str() == "first").expect("the first commit");
    let (oid, summary) = (c.oid, c.summary.to_string());
    assert_eq!(copy_message(&mut st, oid), Some(summary));
    assert_eq!(copy_message(&mut st, oid).as_deref(), Some("first"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_commit_that_is_gone_is_toasted_and_nothing_is_copied() {
    let dir = scratch_repo("copy-message-gone");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let gone = Oid::from_str("1234567890123456789012345678901234567890").unwrap();
    assert_eq!(copy_message(&mut st, gone), None);
    let toast = st.toasts.last().expect("a toast");
    assert!(!toast.ok);
    let prefix = "Couldn't copy the commit message: ";
    assert!(toast.text.starts_with(prefix) && toast.text.len() > prefix.len(), "{}", toast.text);
    let _ = std::fs::remove_dir_all(&dir);
}
