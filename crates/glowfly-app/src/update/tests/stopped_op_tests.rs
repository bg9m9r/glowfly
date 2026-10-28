//! A merge, cherry-pick or revert stopped for conflicts, in the commit box: the draft prefilled
//! from MERGE_MSG for a cherry-pick or revert too (a typed draft kept), Amend refused with its
//! reason, and a pick sequence whose stopped commit was committed from the commit box keeping
//! its banner and Continue.

use super::conflict_tests::conflicted_repo;
use super::tests::{drive, git};
use crate::msg::{CommitMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::commit::{amend_blocked, commit_blocked};
use crate::state::op_actions::{banner_title, state_actions};
use crate::state::op_confirm::banner_msg;
use git2::RepositoryState;
use glowfly_core::gitcli::{args, run};
use std::path::{Path, PathBuf};

fn opened(dir: &Path) -> AppState {
    let mut st = super::tests::state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())));
    st
}

fn refresh(st: &mut AppState) {
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

fn repo_state(st: &AppState) -> RepositoryState {
    st.snap.as_ref().unwrap().state
}

fn stops(dir: &Path, a: &[&str]) {
    let (ok, out) = run(dir, &args(a));
    assert!(!ok, "git {a:?} should stop for conflicts: {out}");
}

/// main ("ours") and `other` ("theirs") changing a.txt's second line, nothing in progress.
fn diverged(name: &str) -> PathBuf {
    let dir = conflicted_repo(name);
    git(&dir, &["merge", "--abort"]);
    dir
}

/// `diverged`, with main's "ours" reverted on top of a later edit, which conflicts.
fn stopped_revert(name: &str) -> PathBuf {
    let dir = diverged(name);
    std::fs::write(dir.join("a.txt"), "one\nours\nmore\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "more"]);
    stops(&dir, &["revert", "--no-edit", "HEAD~1"]);
    dir
}

#[test]
fn a_stopped_cherry_pick_or_revert_prefills_its_message() {
    let dir = diverged("stopped-pick-prefill");
    let mut st = opened(&dir);
    stops(&dir, &["cherry-pick", "other"]);
    refresh(&mut st);
    assert_eq!(repo_state(&st), RepositoryState::CherryPick);
    assert_eq!(st.session.draft.summary, "theirs", "the picked commit's message, without git's conflict comments");
    assert!(st.session.draft.is_blank(), "still untouched");
    git(&dir, &["cherry-pick", "--abort"]);
    refresh(&mut st);
    assert!(st.session.draft.summary.is_empty() && st.session.draft.merge_msg.is_none(), "cleared again when it ends untouched");

    // Text typed before the pick stopped stays.
    st.session.draft.summary = "My own words".into();
    stops(&dir, &["cherry-pick", "other"]);
    refresh(&mut st);
    assert_eq!(st.session.draft.summary, "My own words");
    assert!(st.session.draft.merge_msg.is_none());
    git(&dir, &["cherry-pick", "--abort"]);
    let _ = std::fs::remove_dir_all(&dir);

    let dir = stopped_revert("stopped-revert-prefill");
    let st = opened(&dir);
    assert_eq!(repo_state(&st), RepositoryState::Revert);
    assert_eq!(st.session.draft.summary, "Revert \"ours\"");
    assert!(st.session.draft.description.starts_with("This reverts commit "), "{:?}", st.session.draft.description);
    git(&dir, &["revert", "--abort"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn amend_is_refused_while_a_merge_cherry_pick_or_revert_is_stopped() {
    use RepositoryState::*;
    let t = glowfly_core::i18n::Lang::En;
    for (state, why) in [
        (Merge, Some("Finish the merge before amending.")),
        (CherryPick, Some("Finish the cherry-pick before amending.")),
        (CherryPickSequence, Some("Finish the cherry-pick before amending.")),
        (Revert, Some("Finish the revert before amending.")),
        (RevertSequence, Some("Finish the revert before amending.")),
        (RebaseInteractive, None),
        (Clean, None),
    ] {
        assert_eq!(amend_blocked(t, state), why, "{state:?}");
    }

    let dir = diverged("stopped-pick-amend");
    let mut st = opened(&dir);
    stops(&dir, &["cherry-pick", "other"]);
    std::fs::write(dir.join("a.txt"), "one\nours\ntheirs\n").unwrap();
    git(&dir, &["add", "a.txt"]);
    refresh(&mut st);
    assert_eq!(commit_blocked(&st, false), None, "resolved: the pick commits");
    st.session.draft.amend = true;
    assert_eq!(commit_blocked(&st, false), Some("Finish the cherry-pick before amending."));
    let head = st.snap.as_ref().unwrap().head_oid;
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    assert_eq!(st.snap.as_ref().unwrap().head_oid, head, "nothing was amended");
    assert_eq!(repo_state(&st), RepositoryState::CherryPick);
    git(&dir, &["cherry-pick", "--abort"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_pick_sequence_committed_from_the_commit_box_keeps_its_banner_and_continue() {
    let dir = diverged("stopped-pick-sequence");
    git(&dir, &["checkout", "-q", "other"]);
    std::fs::write(dir.join("c.txt"), "c\n").unwrap();
    git(&dir, &["add", "c.txt"]);
    git(&dir, &["commit", "-q", "-m", "add c"]);
    git(&dir, &["checkout", "-q", "main"]);
    let mut st = opened(&dir);
    stops(&dir, &["cherry-pick", "other~1", "other"]);
    std::fs::write(dir.join("a.txt"), "one\nours\ntheirs\n").unwrap();
    git(&dir, &["add", "a.txt"]);
    refresh(&mut st);
    assert_eq!(st.session.draft.summary, "theirs");
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    assert!(st.snap.as_ref().unwrap().staged.is_empty(), "committed");

    // libgit2 now says clean, but the rest of the sequence is still to run.
    assert_eq!(repo_state(&st), RepositoryState::CherryPickSequence);
    let title = banner_title(st.settings.lang(), st.snap.as_ref().unwrap());
    assert_eq!(title.as_deref(), Some("Cherry-pick in progress"));
    let (label, cmd) = *state_actions(repo_state(&st)).unwrap().iter().find(|(l, _)| *l == "Continue").expect("Continue");
    let m = banner_msg(&st, label, cmd);
    drive(&mut st, m);
    assert_eq!(repo_state(&st), RepositoryState::Clean, "the sequence finished");
    assert!(dir.join("c.txt").is_file(), "add c was picked");
    let _ = std::fs::remove_dir_all(&dir);
}
