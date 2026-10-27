//! Headless tests for the dashboard's row actions on fixture repositories: opening a clone in
//! its tab, showing a pull request at once or once that tab lists it, and the dashboard closing
//! (but keeping its results) when a tab is chosen.

use super::super::tests::{drive, finish, git, scratch_repo, state};
use super::super::update;
use super::tests::{gh, pr};
use crate::effect::{Effect, ForgeReply, ForgeResult, LoadKind};
use crate::msg::{DashboardMsg, ForgeMsg, Msg, PaletteMsg, RepoMsg, TabsMsg};
use crate::state::AppState;
use crate::state::dashboard::{PendingPr, TargetData};
use crate::state::palette::PaletteCommand;
use glowfly_core::forge::{PullRequest, Target};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A fixture repository whose origin is github.com/acme/<repo>.
fn repo(name: &str, repo: &str) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["remote", "add", "origin", &format!("https://github.com/acme/{repo}.git")]);
    dir
}

fn run(st: &mut AppState, m: DashboardMsg) -> Vec<Effect> {
    update(st, Msg::Dashboard(m), Instant::now())
}

fn listed(st: &mut AppState, target: Target, prs: Vec<PullRequest>) {
    let reply = ForgeReply { target, auth: None, results: vec![ForgeResult::Prs(Ok(prs))] };
    update(st, Msg::Forge(ForgeMsg::ForgeReplied(reply)), Instant::now());
}

fn shown_pr(st: &AppState) -> Option<u64> {
    st.session.pull_request.view.as_ref().map(|v| v.pr.id)
}

fn same(a: &Path, b: &Path) -> bool {
    super::super::tests::same_place(a, b)
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn open_repo_selects_its_tab_or_loads_it() {
    let (a, b) = (repo("dash-open-a", "a"), repo("dash-open-b", "b"));
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    assert_eq!(st.active_tab, 1);
    st.dashboard.open = true;
    run(&mut st, DashboardMsg::OpenRepo(a.clone()));
    assert_eq!(st.active_tab, 0, "the parked tab is selected");
    assert!(!st.dashboard.open);
    assert!(st.snap.as_ref().is_some_and(|s| same(&s.workdir, &a)));

    let mut st = state();
    st.dashboard.open = true;
    let fx = run(&mut st, DashboardMsg::OpenRepo(b.clone()));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { kind: LoadKind::Open, .. })));
    assert_eq!(st.tabs.len(), 1, "the empty tab is used");
    assert_eq!(st.tabs[0].path.as_deref(), Some(b.as_path()));
    assert!(!st.dashboard.open);
    cleanup(&[&a, &b]);
}

#[test]
fn show_pull_request_opens_at_once_or_when_the_tab_lists_it() {
    let (a, b) = (repo("dash-pr-a", "a"), repo("dash-pr-b", "b"));
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    assert_eq!(st.session.forge.as_ref().and_then(|f| f.host.clone()), Some(gh("acme", "a")));
    st.session.forge.as_mut().unwrap().prs = vec![pr(4, "Listed", "ada", &[])];
    st.dashboard.open = true;
    run(&mut st, DashboardMsg::OpenPr { path: a.clone(), target: gh("acme", "a"), id: 4 });
    assert_eq!(shown_pr(&st), Some(4), "already listed: shown at once");
    assert!(st.dashboard.pending_pr.is_none() && !st.dashboard.open);

    // Not open yet: it waits for the new tab's list.
    let fx = run(&mut st, DashboardMsg::OpenPr { path: b.clone(), target: gh("acme", "b"), id: 7 });
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { kind: LoadKind::Open, .. })));
    assert!(st.dashboard.pending_pr.is_some());
    finish(&mut st, fx);
    assert!(st.snap.as_ref().is_some_and(|s| same(&s.workdir, &b)));
    assert_eq!(shown_pr(&st), None);
    listed(&mut st, gh("acme", "b"), vec![pr(7, "Waiting", "ada", &[])]);
    assert_eq!(shown_pr(&st), Some(7));
    assert!(st.dashboard.pending_pr.is_none());

    // Merged meanwhile: the list doesn't have it.
    st.dashboard.pending_pr = Some(PendingPr { path: b.clone(), target: gh("acme", "b"), id: 8 });
    listed(&mut st, gh("acme", "b"), Vec::new());
    assert!(st.log.iter().any(|l| !l.ok && l.text == "#8 is no longer open on GitHub."), "{:?}", st.log.last().map(|l| &l.text));
    assert!(st.dashboard.pending_pr.is_none());

    // Another repository's list drops the request.
    st.session.pull_request.view = None;
    st.dashboard.pending_pr = Some(PendingPr { path: a.clone(), target: gh("acme", "a"), id: 9 });
    listed(&mut st, gh("acme", "b"), vec![pr(9, "Same number elsewhere", "ada", &[])]);
    assert_eq!(shown_pr(&st), None);
    assert!(st.dashboard.pending_pr.is_none());
    cleanup(&[&a, &b]);
}

#[test]
fn choosing_a_tab_closes_the_dashboard_but_keeps_its_results() {
    let (a, b) = (repo("dash-tabs-a", "a"), repo("dash-tabs-b", "b"));
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    st.dashboard.open = true;
    st.dashboard.targets.push(TargetData::new(gh("acme", "a")));
    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    assert!(!st.dashboard.open);
    assert_eq!(st.dashboard.targets.len(), 1, "not parked with a tab");
    st.dashboard.open = true;
    update(&mut st, Msg::Tabs(TabsMsg::NewTab), Instant::now());
    assert!(!st.dashboard.open);
    assert_eq!(st.dashboard.targets.len(), 1);
    cleanup(&[&a, &b]);
}

#[test]
fn opening_a_repository_any_other_way_closes_the_dashboard() {
    let (a, b) = (repo("dash-elsewhere-a", "a"), repo("dash-elsewhere-b", "b"));
    let mut st = state();
    st.dashboard.open = true;
    st.dashboard.targets.push(TargetData::new(gh("acme", "a")));
    // The toolbar's Open… or a recent or favorite entry.
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    assert!(!st.dashboard.open);
    assert!(st.snap.as_ref().is_some_and(|s| same(&s.workdir, &a)));
    assert_eq!(st.dashboard.targets.len(), 1, "its results are kept");
    // Already open in a tab: that tab is selected.
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    st.dashboard.open = true;
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    assert!(!st.dashboard.open);
    assert_eq!(st.active_tab, 0);
    cleanup(&[&a, &b]);
}

fn has_entry(st: &AppState) -> bool {
    crate::state::palette_commands::entries(st).iter().any(|e| e.label == "Open dashboard" && e.command == PaletteCommand::Dashboard)
}

#[test]
fn the_palette_opens_the_dashboard_with_or_without_a_repository() {
    let mut st = state();
    assert!(has_entry(&st));
    update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::Dashboard)), Instant::now());
    assert!(st.dashboard.open);
    let a = repo("dash-palette", "a");
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    assert!(st.snap.is_some() && has_entry(&st));
    cleanup(&[&a]);
}
