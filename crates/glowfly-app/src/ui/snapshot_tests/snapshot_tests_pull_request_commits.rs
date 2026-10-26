//! The pull request panel's Commits tab (the list with a commit selected, no commits, the
//! notes when the source is missing) and the notes on the head commit: shown from the listed
//! head, being fetched, and a failed fetch with its button disabled in an untrusted repository.

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_pull_request::{compare, with_pr};
use super::snapshot_tests_select::click;
use crate::msg::Msg;
use crate::pull_request::PullRequestMsg;
use crate::state::pull_request::{Files, PrTab, PrView};
use crate::state::pull_request_checks::ChecksPane;
use crate::state::pull_request_source::HeadFetch;
use crate::state::{AppState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::PointerButton;
use egui_kittest::kittest::Queryable;
use git2::{Oid, Repository, Signature, Time};
use glowfly_core::forge::CheckList;
use glowfly_core::repo::{self, LoadOptions};
use glowfly_core::settings::{RepoTrust, ThemeChoice};
use std::path::Path;

/// Commits `file` on top of search (2026-01-11), moves search and origin/search to it, and
/// reopens the panel with it as the listed head.
fn extend_search(st: &mut AppState, dir: &Path, file: &str, summary: &str) -> Oid {
    let repo = Repository::open(dir).unwrap();
    let parent = repo.revparse_single("search").unwrap().peel_to_commit().unwrap();
    let mut tb = repo.treebuilder(Some(&parent.tree().unwrap())).unwrap();
    tb.insert(file, repo.blob(b"results\n").unwrap(), 0o100644).unwrap();
    let tree = repo.find_tree(tb.write().unwrap()).unwrap();
    let sig = Signature::new("Ada Lovelace", "ada@example.com", &Time::new(1_768_125_600, 0)).unwrap();
    let oid = repo.commit(Some("refs/heads/search"), &sig, &sig, summary, &tree, &[&parent]).unwrap();
    repo.reference("refs/remotes/origin/search", oid, true, "test").unwrap();
    st.snap = Some(repo::load(dir, &LoadOptions::all(500)).unwrap());
    reopen(st, |p| p.head = Some(oid.to_string()));
    oid
}

/// The panel opened again from the listed pull request as `f` changes it.
fn reopen(st: &mut AppState, f: impl FnOnce(&mut glowfly_core::forge::PullRequest)) {
    let fs = st.forge.as_mut().unwrap();
    f(&mut fs.prs[0]);
    let mut view = PrView::new(fs.prs[0].clone(), fs, st.snap.as_ref().unwrap());
    view.checks.pane = ChecksPane::Loaded(CheckList::empty(fs.prs[0].head.clone().unwrap_or_default()));
    st.session.pull_request.view = Some(view);
}

fn commit_oids(st: &AppState) -> Vec<Oid> {
    st.session.pull_request.view.as_ref().unwrap().shown.as_ref().unwrap().commits.commits.iter().map(|c| c.oid).collect()
}

#[test]
fn pull_request_commits_dark() {
    let (mut st, dir) = with_pr(ThemeChoice::Dark, "pr-commits", true);
    let second = extend_search(&mut st, &dir, "paging.txt", "Page the results");
    compare(&mut st, &dir);
    let oids = commit_oids(&st);
    assert_eq!(oids.len(), 2);
    assert_eq!(oids[1], second, "oldest first");
    st.session.pull_request.view.as_mut().unwrap().tab = PrTab::Commits;
    st.selection = Selection::Commit(second);
    st.detail = Some(repo::commit_detail(&dir, second).unwrap());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("pull_request_commits_dark");
    assert!(h.query_by_label("Commits  2").is_some());
    assert!(h.query_by_label("2 commits").is_some());
    let first = format!("{} Search results page", &oids[0].to_string()[..7]);
    assert!(h.query_by_label(&format!("{} Page the results", &second.to_string()[..7])).is_some());
    assert!(h.query_by_label_contains("Select a commit to see it in the commit panel").is_some());
    h.get_by_label(&first).click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::PullRequest(PullRequestMsg::SelectCommit(o)) if *o == oids[0])));
    let rect = h.get_by_label(&first).bounding_box().expect("the row");
    let center = eframe::egui::pos2(((rect.x0 + rect.x1) / 2.0) as f32, ((rect.y0 + rect.y1) / 2.0) as f32);
    click(&mut h, center, PointerButton::Secondary);
    assert!(h.query_by_label("Copy commit SHA").is_some());
    assert!(!h.get_by_label("Show on graph").is_disabled(), "search is on the graph");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pull_request_commits_empty_light() {
    let (mut st, dir) = with_pr(ThemeChoice::Light, "pr-commits-empty", true);
    // The source is the target's tip: nothing to bring.
    let main = st.session.pull_request.view.as_ref().unwrap().tips.target.unwrap();
    let repo = Repository::open(&dir).unwrap();
    repo.reference("refs/remotes/origin/search", main, true, "test").unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    reopen(&mut st, |p| p.head = Some(main.to_string()));
    compare(&mut st, &dir);
    st.session.pull_request.view.as_mut().unwrap().tab = PrTab::Commits;
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("pull_request_commits_empty_light");
    assert!(h.query_by_label("No commits").is_some());
    assert!(h.query_by_label_contains("search has no commits that main doesn't.").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pull_request_commits_missing_light() {
    let (mut st, dir) = with_pr(ThemeChoice::Light, "pr-commits-missing", false);
    // The listed head isn't here either.
    reopen(&mut st, |p| p.head = Some("0123456789abcdef0123456789abcdef01234567".into()));
    let v = st.session.pull_request.view.as_mut().unwrap();
    v.tab = PrTab::Commits;
    assert_eq!(v.files, Files::Missing);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("pull_request_commits_missing_light");
    assert!(h.query_by_label_contains("Fetch #12's head from origin to compare.").is_some());
    assert!(!h.get_by_label("Fetch origin").is_disabled());
    assert!(!h.get_by_label("Fetch #12's head").is_disabled());
    h.get_by_label("Fetch #12's head").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::PullRequest(PullRequestMsg::FetchHead))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pull_request_head_used_light() {
    let (mut st, dir) = with_pr(ThemeChoice::Light, "pr-head-used", false);
    compare(&mut st, &dir);
    let v = st.session.pull_request.view.as_mut().unwrap();
    assert_eq!(v.shown.as_ref().unwrap().source.from, repo::SourceFrom::Head);
    v.tab = PrTab::Files;
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("pull_request_head_used_light");
    let note = "origin/search isn't in this repository, so this shows the pull request's head commit";
    assert!(h.query_by_label_contains(note).is_some());
    assert!(h.query_by_label("Fetch origin").is_none(), "only a head-used note");
    assert!(h.query_by_label("1 file changed").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pull_request_head_fetching_dark() {
    let (mut st, dir) = with_pr(ThemeChoice::Dark, "pr-head-fetching", false);
    reopen(&mut st, |p| p.head = Some("0123456789abcdef0123456789abcdef01234567".into()));
    let v = st.session.pull_request.view.as_mut().unwrap();
    v.tab = PrTab::Files;
    v.head_fetch = HeadFetch::Running;
    let mut h = harness(st, true);
    // The banner's spinner keeps repainting: a fixed number of frames.
    h.run_steps(4);
    h.check_snapshot("pull_request_head_fetching_dark");
    assert!(h.query_by_label("Fetching #12's head from origin…").is_some());
    assert!(h.query_by_label("Fetch origin").is_none() && h.query_by_label("Fetch #12's head").is_none());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pull_request_head_failed_light() {
    let (mut st, dir) = with_pr(ThemeChoice::Light, "pr-head-failed", false);
    reopen(&mut st, |p| p.head = Some("0123456789abcdef0123456789abcdef01234567".into()));
    let v = st.session.pull_request.view.as_mut().unwrap();
    v.tab = PrTab::Files;
    v.head_fetch = HeadFetch::Failed("could not read Username for 'https://github.com'".into());
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    crate::state::trust::record(&mut st.settings, &real, RepoTrust::NotTrusted);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("pull_request_head_failed_light");
    assert!(h.query_by_label_contains("Couldn't fetch #12's head from origin: could not read Username").is_some());
    assert!(h.get_by_label("Fetch #12's head").is_disabled(), "untrusted");
    drop(h);
    remove_fixture(&dir);
}
