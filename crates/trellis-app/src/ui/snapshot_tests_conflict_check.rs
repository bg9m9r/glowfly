//! Predicted merge conflicts: the toolbar chip and its popover, and the report dialog for
//! checks the branch menus ask for. Shares the fixture repository and renderer with
//! `snapshot_tests`.

use super::snapshot_tests::{git, harness, reload, remove_fixture, render, with_repo};
use crate::conflict_check::CheckRequest;
use crate::msg::{JobMsg, Msg};
use crate::state::conflict_check::{ConflictReport, current_request};
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::conflict_check::{MergeTarget, Prediction, TargetKind};
use trellis_core::gitcli::args;
use trellis_core::repo::LoadOptions;
use trellis_core::settings::ThemeChoice;

/// The fixture checked out on `search`, whose automatic check against main found `result`.
fn on_search(theme: ThemeChoice, name: &str, result: Result<Prediction, String>) -> (AppState, PathBuf, CheckRequest) {
    let (mut st, dir) = with_repo(theme, name);
    git(&dir, &["checkout", "-q", "search"], "2026-01-10T10:00:00+00:00");
    reload(&mut st, &dir, &LoadOptions::all(500));
    let request = current_request(st.snap.as_ref().unwrap()).expect("search compares with main");
    assert_eq!((request.target.name.as_str(), request.target.kind), ("main", TargetKind::DefaultBranch));
    let generation = st.conflict_checks.start().unwrap();
    st.conflict_checks.finish(generation, ConflictReport { request: request.clone(), result });
    (st, dir, request)
}

fn conflicts(paths: &[&str]) -> Result<Prediction, String> {
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

fn report(kind: TargetKind, result: Result<Prediction, String>) -> ConflictReport {
    let target = MergeTarget { name: "search".into(), oid: git2::Oid::zero(), kind };
    ConflictReport { request: CheckRequest { branch: "main".into(), ours: git2::Oid::zero(), target }, result }
}

#[test]
fn conflict_report_modal() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "conflict-report");
    let paths: Vec<String> = (0..60).map(|i| format!("src/module_{i:02}.rs")).collect();
    st.modal = Some(Modal::ConflictReport(report(TargetKind::Chosen, Ok(Prediction::Conflicts(paths)))));
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("conflict_report_modal");
    assert!(h.query_by_label("Merging search into main would stop with conflicts in 60 files:").is_some());
    assert!(h.query_by_label("and 10 more").is_some(), "the cap line");
    assert!(h.query_by_label("Rebase main onto search").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn conflict_report_clean() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "conflict-clean");
    st.modal = Some(Modal::ConflictReport(report(TargetKind::Chosen, Ok(Prediction::Clean))));
    render("conflict_report_clean", st, true);

    let (mut st, _) = with_repo(ThemeChoice::Dark, "conflict-clean");
    st.modal = Some(Modal::ConflictReport(report(TargetKind::Chosen, Ok(Prediction::UpToDate))));
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
    let generation = st.conflict_checks.start().unwrap();
    st.conflict_checks.finish(generation, ConflictReport { request, result: conflicts(&["a.txt"]) });
    assert!(!chip(st), "for another commit");
    remove_fixture(&dir);
}
