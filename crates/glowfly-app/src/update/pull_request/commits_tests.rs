//! Headless tests for the pull request panel's Commits tab: a row shows its commit in the
//! commit panel without closing the panel, the commit's diffs open in the tab, and a refresh
//! keeps a selected commit that only the panel lists.

use super::super::tests::drive;
use super::super::update;
use super::source_tests::head_on_remote;
use super::tests::{cleanup, open, set_pr, setup};
use crate::effect::{Effect, LineCountsEffect};
use crate::msg::{Msg, RepoMsg};
use crate::pull_request::PullRequestMsg;
use crate::state::pull_request::PrTab;
use crate::state::{AppState, Selection};
use git2::Oid;
use glowfly_core::repo::DiffTarget;
use std::time::Instant;

fn listed(st: &AppState) -> Vec<Oid> {
    st.session.pull_request.view.as_ref().unwrap().shown.as_ref().unwrap().commits.commits.iter().map(|c| c.oid).collect()
}

#[test]
fn a_row_selects_its_commit_and_keeps_the_panel() {
    let (mut st, work, bare, base) = setup("pr-commit-select", &["g.txt"]);
    open(&mut st);
    let commits = listed(&st);
    assert_eq!(commits.len(), 2);
    st.settings.show_details_panel = false;
    let fx = update(&mut st, Msg::PullRequest(PullRequestMsg::SelectCommit(commits[0])), Instant::now());
    assert_eq!(st.session.selection, Selection::Commit(commits[0]));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadDetail { oid, .. } if *oid == commits[0])));
    assert!(fx.iter().any(|e| matches!(e, Effect::LineCounts(LineCountsEffect::Commit { oid, .. }) if *oid == commits[0])));
    assert!(st.session.pull_request.view.is_some(), "the panel stays open");
    assert!(st.settings.show_details_panel, "the commit panel is shown");
    // Again: only the panel is revealed.
    let fx = update(&mut st, Msg::PullRequest(PullRequestMsg::SelectCommit(commits[0])), Instant::now());
    assert!(!fx.iter().any(|e| matches!(e, Effect::LoadDetail { .. })));
    // A commit the panel doesn't list does nothing.
    assert!(update(&mut st, Msg::PullRequest(PullRequestMsg::SelectCommit(base)), Instant::now()).is_empty());
    assert_eq!(st.session.selection, Selection::Commit(commits[0]));

    // Its files open in the Commits tab.
    drive(&mut st, Msg::PullRequest(PullRequestMsg::SelectCommit(commits[1])));
    let target = DiffTarget::Commit(commits[1], "g.txt".into());
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(target.clone())));
    let v = st.session.pull_request.view.as_ref().expect("still open");
    assert_eq!(v.tab, PrTab::Commits);
    assert_eq!(st.session.diff.view.as_ref().map(|(t, _)| t), Some(&target));
    // Selecting another of its commits closes that commit's diff.
    drive(&mut st, Msg::PullRequest(PullRequestMsg::SelectCommit(commits[0])));
    assert!(st.session.diff.view.is_none() && st.session.pull_request.view.is_some());
    // A diff of a commit outside the list closes the panel.
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Commit(base, "a.txt".into()))));
    assert!(st.session.pull_request.view.is_none());
    cleanup(&[&work, &bare]);
}

#[test]
fn a_refresh_keeps_a_fork_commit_that_is_not_on_the_graph() {
    let (mut st, work, bare, _) = setup("pr-commit-refresh", &[]);
    let head = head_on_remote(&bare);
    set_pr(&mut st, |p| {
        p.source = "from-a-fork".into();
        p.head = Some(head.to_string());
    });
    open(&mut st);
    assert!(listed(&st).contains(&head));
    drive(&mut st, Msg::PullRequest(PullRequestMsg::SelectCommit(head)));
    assert!(!st.snap.as_ref().unwrap().row_of.contains_key(&head), "on no branch");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(st.session.selection, Selection::Commit(head));
    assert!(st.session.detail.as_ref().is_some_and(|d| d.oid == head), "its details loaded");
    cleanup(&[&work, &bare]);
}
