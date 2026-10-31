//! Predicted merge conflicts: the toolbar chip and its popover, and the report dialog for
//! checks the branch menus ask for. Shares the fixture repository and renderer with
//! `snapshot_tests`.

use super::snapshot_tests::{git, harness, reload, remove_fixture, render, with_repo};
use crate::msg::{JobMsg, Msg};
use crate::state::conflict_check::{ConflictReport, current_request};
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::conflict_check::{MergeTarget, Prediction, TargetKind};
use glowfly_core::gitcli::args;
use glowfly_core::repo::LoadOptions;
use glowfly_core::settings::ThemeChoice;
use glowfly_model::conflict_check::CheckRequest;
use std::path::PathBuf;

// The chip under the real pointer.
#[path = "snapshot_tests_conflict_chip.rs"]
mod chip;

const HEAD_MOVED: &str = "The checked-out branch has changed since this check, so it can't merge or rebase from here.";

/// The fixture checked out on `search`, whose automatic check against main found `result`.
pub(crate) fn on_search(theme: ThemeChoice, name: &str, result: Result<Prediction, String>) -> (AppState, PathBuf, CheckRequest) {
    let (mut st, dir) = with_repo(theme, name);
    git(&dir, &["checkout", "-q", "search"], "2026-01-10T10:00:00+00:00");
    reload(&mut st, &dir, &LoadOptions::all(500));
    let request = current_request(st.snap.as_ref().unwrap()).expect("search compares with main");
    assert_eq!((request.target.name.as_str(), request.target.kind), ("main", TargetKind::DefaultBranch));
    let generation = st.session.reads.conflict_checks.start().unwrap();
    st.session.reads.conflict_checks.finish(generation, ConflictReport { request: request.clone(), result });
    (st, dir, request)
}

pub(crate) fn conflicts(paths: &[&str]) -> Result<Prediction, String> {
    Ok(Prediction::Conflicts(paths.iter().map(|p| p.to_string()).collect()))
}

#[test]
fn conflict_popover() {
    let files = conflicts(&["src/app.rs", "README.md", "docs/guide with space.md"]);
    let (st, dir, _) = on_search(ThemeChoice::Dark, "conflict-popover", files);
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Conflicts with main").click();
    h.run();
    h.check_snapshot("conflict_popover");
    assert!(h.query_by_label("docs/guide with space.md").is_some());
    h.get_by_label("Merge main into search").click();
    h.run();
    let merge = vec![args(&["merge", "main"])];
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Job(JobMsg::Git { cmds, .. }) if *cmds == merge)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn conflict_popover_many() {
    let paths: Vec<String> = (0..12).map(|i| format!("src/module_{i:02}.rs")).collect();
    let (st, dir, _) = on_search(ThemeChoice::Light, "conflict-popover-many", Ok(Prediction::Conflicts(paths)));
    let mut h = harness(st, false);
    h.run();
    h.get_by_label("Conflicts with main").click();
    h.run();
    h.check_snapshot("conflict_popover_many");
    assert!(h.query_by_label("src/module_07.rs").is_some());
    assert!(h.query_by_label("src/module_08.rs").is_none(), "the popover lists the first eight");
    assert!(h.query_by_label("and 4 more").is_some());
    drop(h);
    remove_fixture(&dir);
}

/// A check of merging search into main, made for main's tip as checked out in `st`.
pub(crate) fn report(st: &AppState, kind: TargetKind, result: Result<Prediction, String>) -> ConflictReport {
    let target = MergeTarget { name: "search".into(), oid: git2::Oid::zero(), kind };
    let ours = st.snap.as_ref().and_then(|s| s.head_oid).unwrap();
    ConflictReport { request: CheckRequest { branch: "main".into(), ours, target }, result }
}

#[test]
fn conflict_report_modal() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "conflict-report");
    let paths: Vec<String> = (0..60).map(|i| format!("src/module_{i:02}.rs")).collect();
    st.modal = Some(Modal::ConflictReport(report(&st, TargetKind::Chosen, Ok(Prediction::Conflicts(paths)))));
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("conflict_report_modal");
    assert!(h.query_by_label("Merging search into main would stop with conflicts in 60 files:").is_some());
    assert!(h.query_by_label("and 10 more").is_none(), "the report lists every path");
    assert!(h.query_by_label("src/module_59.rs").is_some());
    assert!(h.query_by_label("Rebase main onto search").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn conflict_report_clean() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "conflict-clean");
    st.modal = Some(Modal::ConflictReport(report(&st, TargetKind::Chosen, Ok(Prediction::Clean))));
    render("conflict_report_clean", st, true);

    let (mut st, _) = with_repo(ThemeChoice::Dark, "conflict-clean");
    st.modal = Some(Modal::ConflictReport(report(&st, TargetKind::Chosen, Ok(Prediction::UpToDate))));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("main already contains everything on search.").is_some());
    assert!(h.query_by_label("Merge search into main").is_none(), "nothing to merge");
    assert!(h.query_by_label("Rebase main onto search").is_none());
    h.get_by_label("Close").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::CloseModal)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn conflict_report_after_a_checkout_offers_no_actions() {
    // The check was for main; login-page was checked out before its reply arrived.
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "conflict-moved");
    let r = report(&st, TargetKind::Chosen, conflicts(&["a.txt"]));
    git(&dir, &["checkout", "-q", "login-page"], "2026-01-10T10:00:00+00:00");
    reload(&mut st, &dir, &LoadOptions::all(500));
    st.modal = Some(Modal::ConflictReport(r));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Merging search into main would stop with conflicts in 1 file:").is_some());
    assert!(h.query_by_label("Merge search into main").is_none(), "would merge into login-page");
    assert!(h.query_by_label("Rebase main onto search").is_none(), "would rebase login-page");
    assert!(h.query_by_label(HEAD_MOVED).is_some());
    drop(h);

    // A commit on main since the check: the prediction is out of date, too.
    let (mut st, _) = with_repo(ThemeChoice::Dark, "conflict-moved");
    let mut r = report(&st, TargetKind::Chosen, conflicts(&["a.txt"]));
    r.request.ours = git2::Oid::zero();
    st.modal = Some(Modal::ConflictReport(r));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Merge search into main").is_none());
    assert!(h.query_by_label(HEAD_MOVED).is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn chip_only_for_current_conflicts() {
    let chip = |st: AppState| {
        let mut h = harness(st, true);
        h.run();
        h.query_by_label("Conflicts with main").is_some()
    };
    let (st, dir, _) = on_search(ThemeChoice::Dark, "conflict-chip", conflicts(&["a.txt"]));
    assert!(chip(st));

    let (mut st, _, _) = on_search(ThemeChoice::Dark, "conflict-chip", conflicts(&["a.txt"]));
    st.settings.conflict_check = false;
    assert!(!chip(st), "preference off");

    let (st, _, _) = on_search(ThemeChoice::Dark, "conflict-chip", Ok(Prediction::Clean));
    assert!(!chip(st), "clean");

    let (mut st, _, mut request) = on_search(ThemeChoice::Dark, "conflict-chip", Ok(Prediction::Clean));
    request.ours = git2::Oid::zero();
    let generation = st.session.reads.conflict_checks.start().unwrap();
    st.session.reads.conflict_checks.finish(generation, ConflictReport { request, result: conflicts(&["a.txt"]) });
    assert!(!chip(st), "for another commit");
    remove_fixture(&dir);
}
