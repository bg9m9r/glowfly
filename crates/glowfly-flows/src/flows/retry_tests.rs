//! Headless tests for the panes' Try again and clear actions: Clear search, Clear filter, and
//! asking again for a blame, a file history, a File view's text, a pull request's files and a
//! comparison's file list after they failed.

use super::history_tests::{oid, with_view};
use super::selection_tests::fixture;
use super::tests::{drive, scratch_repo, state};
use super::update;
use crate::effect::SelectionEffect;
use crate::effect::{BlameRequest, Effect, FileTextRequest};
use crate::msg::{BlameMsg, DashboardMsg, HistoryMsg, Msg, RepoMsg, SearchMsg, SelectionMsg};
use crate::pull_request::{FilesJob, PrEffect, PullRequestMsg};
use crate::state::history::{HistoryPane, TextError, TextState};
use crate::state::pull_request::Files;
use crate::state::{AppState, Modal, Selection};
use glowfly_core::repo::HistoryStart;
use std::path::PathBuf;
use std::time::Instant;

fn up(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

#[test]
fn clear_search_keeps_the_bar_and_the_filter() {
    let (mut st, dir) = opened("retry-clear-search");
    up(&mut st, Msg::Search(SearchMsg::Open));
    up(&mut st, Msg::Search(SearchMsg::ToggleFilter));
    st.session.tab_view.search = "first".into();
    st.view.author_filter = "gr".into();
    up(&mut st, Msg::Search(SearchMsg::QueryChanged));
    assert!(!st.session.search.matches.is_empty());
    st.view.focus_search = false;
    up(&mut st, Msg::Search(SearchMsg::Clear));
    assert!(st.session.tab_view.search.is_empty() && st.view.author_filter.is_empty());
    assert!(st.session.search.matches.is_empty() && !st.session.search.active());
    assert!(st.view.show_search && st.session.search.filter, "the bar and the filter stay");
    assert!(st.view.focus_search, "the field takes the focus");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clear_filter_empties_the_dashboard_filter() {
    let mut st = state();
    st.dashboard.filter = "fix".into();
    up(&mut st, Msg::Dashboard(DashboardMsg::ClearFilter));
    assert!(st.dashboard.filter.is_empty());
}

fn blame_loads(fx: &[Effect]) -> Vec<u64> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::LoadBlame(BlameRequest { generation, .. }) => Some(*generation),
            _ => None,
        })
        .collect()
}

#[test]
fn blame_try_again_asks_again() {
    let (mut st, dir) = opened("retry-blame");
    up(&mut st, Msg::Blame(BlameMsg::Open { path: "a.txt".into(), at: None }));
    assert!(blame_loads(&up(&mut st, Msg::Blame(BlameMsg::Retry))).is_empty(), "nothing while it loads");
    let generation = st.session.blame.generation;
    up(&mut st, Msg::Blame(BlameMsg::Loaded { path: "a.txt".into(), generation, result: Err("not found: a.txt".into()) }));
    assert!(st.session.blame.view.as_ref().unwrap().error.is_some());
    let fx = up(&mut st, Msg::Blame(BlameMsg::Retry));
    assert_eq!(blame_loads(&fx), [generation + 1]);
    let v = st.session.blame.view.as_ref().unwrap();
    assert!(v.loading && v.error.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blame_try_again_in_a_held_repository_asks_for_trust() {
    let (mut st, dir) = super::trust_held_tests::held_repo("retry-blame-held");
    up(&mut st, Msg::Blame(BlameMsg::Open { path: "a.txt".into(), at: None }));
    drive(&mut st, Msg::CloseModal);
    assert!(st.modal.is_none());
    let fx = up(&mut st, Msg::Blame(BlameMsg::Retry));
    assert!(blame_loads(&fx).is_empty(), "{fx:?}");
    assert!(matches!(st.modal, Some(Modal::Trust(_))), "the trust prompt opens again");
    assert_eq!(st.session.blame.view.as_ref().unwrap().error.as_deref(), Some(crate::state::blame::HELD));
    let _ = std::fs::remove_dir_all(&dir);
}

fn history_loads(fx: &[Effect]) -> Vec<(HistoryStart, usize)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::LoadFileHistory { start, limit, .. } => Some((start.clone(), *limit)),
            _ => None,
        })
        .collect()
}

#[test]
fn history_try_again_repeats_the_failed_walk() {
    let (mut st, dir) = with_view("retry-history");
    let base = st.settings.commit_limit();
    // Nothing failed: nothing to do.
    st.session.history.view.as_mut().unwrap().loading = false;
    assert!(history_loads(&up(&mut st, Msg::History(HistoryMsg::RetryFileHistory))).is_empty());

    // The first batch failed: from HEAD again, as deep as it had searched.
    {
        let v = st.session.history.view.as_mut().unwrap();
        v.entries.clear();
        v.batches = 2;
        v.error = Some("network error: offline".into());
    }
    let fx = up(&mut st, Msg::History(HistoryMsg::RetryFileHistory));
    assert!(matches!(&history_loads(&fx)[..], [(HistoryStart::Head, n)] if *n == base * 2), "{fx:?}");
    let v = st.session.history.view.as_ref().unwrap();
    assert!(v.loading && v.error.is_none());
    assert!(history_loads(&up(&mut st, Msg::History(HistoryMsg::RetryFileHistory))).is_empty(), "nothing while it loads");
    let _ = std::fs::remove_dir_all(&dir);

    // Search further failed with entries on screen: that batch again.
    let (mut st, dir) = with_view("retry-history-further");
    let resume = vec![(oid(3), "a.txt".to_owned())];
    {
        let v = st.session.history.view.as_mut().unwrap();
        v.loading = false;
        v.resume = resume.clone();
        v.error = Some("boom".into());
    }
    let fx = up(&mut st, Msg::History(HistoryMsg::RetryFileHistory));
    assert!(matches!(&history_loads(&fx)[..], [(HistoryStart::Resume(r), n)] if *r == resume && *n == base), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_text_try_again_reads_it_again() {
    let (mut st, dir) = with_view("retry-file-text");
    up(&mut st, Msg::History(HistoryMsg::SelectHistoryEntry(0)));
    up(&mut st, Msg::History(HistoryMsg::SetHistoryPane(HistoryPane::File)));
    let error = TextError { message: "could not read a.txt".into(), retry: true };
    up(&mut st, Msg::History(HistoryMsg::FileTextLoaded { oid: oid(1), path: "a.txt".into(), before: false, result: Err(error) }));
    let fx = up(&mut st, Msg::History(HistoryMsg::RetryFileText));
    assert!(matches!(&fx[..], [Effect::LoadFileText(FileTextRequest { oid: o, path, .. })] if *o == oid(1) && path == "a.txt"), "{fx:?}");
    let text = st.session.history.view.as_ref().unwrap().text.as_ref().unwrap();
    assert!(matches!(text.state, TextState::Loading));
    assert!(up(&mut st, Msg::History(HistoryMsg::RetryFileText)).is_empty(), "not failed any more");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pull_request_files_try_again_compares_again() {
    let (mut st, work, bare, _) = super::pull_request::tests::setup("retry-pr-files", &[]);
    super::pull_request::tests::open(&mut st);
    let view = st.session.pull_request.view.as_ref().unwrap();
    assert!(matches!(view.files, Files::Loaded(_)));
    let id = view.pr.id;
    let old = st.session.pull_request.generation;
    let fx = up(&mut st, Msg::PullRequest(PullRequestMsg::ReloadFiles));
    let jobs: Vec<u64> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::PullRequest(PrEffect::Files(FilesJob { generation, .. })) => Some(*generation),
            _ => None,
        })
        .collect();
    assert_eq!(jobs, [old + 1]);
    assert!(matches!(st.session.pull_request.view.as_ref().unwrap().files, Files::Loading));
    // The answer to the earlier request changes nothing.
    up(&mut st, Msg::PullRequest(PullRequestMsg::FilesLoaded { id, generation: old, result: Err("late".into()) }));
    assert!(matches!(st.session.pull_request.view.as_ref().unwrap().files, Files::Loading));
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn compare_try_again_reads_the_list_again() {
    let (mut st, f) = fixture("retry-compare");
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(3))));
    assert!(up(&mut st, Msg::Selection(SelectionMsg::RetryCompare)).is_empty(), "one commit: nothing to compare");

    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(5))));
    if let Selection::Commits(m) = &mut st.session.selection {
        m.changes = Some(Err("boom".into()));
    }
    let fx = up(&mut st, Msg::Selection(SelectionMsg::RetryCompare));
    assert!(
        matches!(&fx[..], [Effect::Selection(SelectionEffect::Compare { from, to, .. })] if (*from, *to) == (f.c(3), f.c(5))),
        "{fx:?}"
    );
    assert!(matches!(&st.session.selection, Selection::Commits(m) if m.changes.is_none()));

    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(3))));
    up(&mut st, Msg::Selection(SelectionMsg::ToggleWip));
    let generation = match &mut st.session.selection {
        Selection::VsWorkdir(w) => {
            w.changes = Some(Err("boom".into()));
            w.generation
        }
        other => panic!("expected a working-directory comparison, got {other:?}"),
    };
    let fx = up(&mut st, Msg::Selection(SelectionMsg::RetryCompare));
    assert!(
        matches!(&fx[..], [Effect::Selection(SelectionEffect::WorkdirCompare { generation: g, .. })] if *g == generation + 1),
        "{fx:?}"
    );
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn new_intents_are_dropped_for_an_inactive_tab() {
    assert!(SearchMsg::Clear.on_inactive().is_none());
    assert!(SelectionMsg::RetryCompare.on_inactive().is_none());
    assert!(BlameMsg::Retry.on_inactive().is_none());
    assert!(HistoryMsg::RetryFileHistory.on_inactive().is_none());
    assert!(HistoryMsg::RetryFileText.on_inactive().is_none());
    assert!(PullRequestMsg::ReloadFiles.on_inactive().is_none());
    assert!(DashboardMsg::ClearFilter.on_inactive().is_none());
}
