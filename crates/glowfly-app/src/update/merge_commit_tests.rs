//! A merge in the commit box: the draft prefilled from MERGE_MSG (and cleared again when the
//! merge ends untouched), the commit rules while merging, and the banner's Commit merge.

use super::conflict_tests::conflicted_repo;
use super::tests::{drive, git, state};
use super::update;
use crate::msg::{CommitMsg, Msg, RepoMsg};
use crate::state::commit::commit_blocked;
use crate::state::{AppState, CommitDraft};
use git2::{Repository, RepositoryState};
use glowfly_core::gitcli::{args, run};
use std::path::Path;
use std::time::Instant;

fn opened(dir: &Path) -> AppState {
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())));
    st
}

fn refresh(st: &mut AppState) {
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

/// Writes `text` to a.txt and stages it, which resolves the merge's conflict.
fn resolve(st: &mut AppState, dir: &Path, text: &str) {
    std::fs::write(dir.join("a.txt"), text).unwrap();
    git(dir, &["add", "a.txt"]);
    refresh(st);
}

#[test]
fn merge_msg_prefills_the_draft() {
    let dir = conflicted_repo("merge-prefill");
    let st = opened(&dir);
    assert_eq!((st.draft.summary.as_str(), st.draft.description.as_str()), ("Merge branch 'other'", ""));
    assert!(st.draft.merge_msg.is_some() && st.draft.is_blank());
    git(&dir, &["merge", "--abort"]);
    let _ = std::fs::remove_dir_all(&dir);

    // Text typed before the merge started stays.
    let dir = conflicted_repo("merge-prefill-typed");
    git(&dir, &["merge", "--abort"]);
    let mut st = opened(&dir);
    st.draft.summary = "My own words".into();
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok, "conflicts again");
    refresh(&mut st);
    assert_eq!(st.draft.summary, "My own words");
    assert!(st.draft.merge_msg.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn prefill_cleared_when_merge_aborted() {
    let dir = conflicted_repo("merge-prefill-clear");
    std::fs::write(dir.join(".git/trellis-template.txt"), "feat: \n\n# why\n").unwrap();
    git(&dir, &["config", "commit.template", ".git/trellis-template.txt"]);
    let mut st = opened(&dir);
    assert_eq!(st.draft.summary, "Merge branch 'other'", "the merge's message wins over the template");
    git(&dir, &["merge", "--abort"]);
    refresh(&mut st);
    assert!(st.draft.merge_msg.is_none());
    assert_eq!(st.draft.summary, "feat:", "cleared, then the template applies again");

    st.draft = CommitDraft::default();
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok);
    refresh(&mut st);
    assert!(st.draft.merge_msg.is_some());
    st.draft.description = "Kept this one".into();
    git(&dir, &["merge", "--abort"]);
    refresh(&mut st);
    assert_eq!((st.draft.summary.as_str(), st.draft.description.as_str()), ("Merge branch 'other'", "Kept this one"), "edited: kept");
    assert!(st.draft.merge_msg.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn commit_blocked_rules() {
    let dir = conflicted_repo("merge-blocked");
    let mut st = opened(&dir);
    let t = st.settings.lang();
    assert_eq!(commit_blocked(&st, false), Some(t.tr("commit.blocked.merge_conflicts")));
    // Resolved as ours: the merge's result equals HEAD, so nothing is staged.
    resolve(&mut st, &dir, "one\nours\n");
    assert!(st.snap.as_ref().unwrap().staged.is_empty());
    assert_eq!(commit_blocked(&st, false), None, "an empty Staged list still commits a merge");
    st.draft.amend = true;
    assert_eq!(commit_blocked(&st, false), Some(t.tr("commit.blocked.amend_merge")));
    git(&dir, &["merge", "--abort"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn commit_merge_from_banner() {
    let dir = conflicted_repo("merge-commit-banner");
    let mut st = opened(&dir);
    let fx = update(&mut st, Msg::Commit(CommitMsg::CommitMerge), Instant::now());
    assert!(fx.is_empty() && st.job.is_none(), "conflicts left: nothing runs");
    assert!(st.toasts.iter().any(|t| !t.ok && t.text == "Resolve the 1 conflicted file first."));

    resolve(&mut st, &dir, "one\nmerged\n");
    st.draft = CommitDraft::default();
    drive(&mut st, Msg::Commit(CommitMsg::CommitMerge));
    let repo = Repository::open(&dir).unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(head.parent_count(), 2);
    assert_eq!(head.summary(), Some("Merge branch 'other'"));
    assert_eq!(st.snap.as_ref().unwrap().state, RepositoryState::Clean);
    assert!(st.draft.summary.is_empty() && st.draft.merge_msg.is_none(), "the draft starts over");
    let _ = std::fs::remove_dir_all(&dir);
}
