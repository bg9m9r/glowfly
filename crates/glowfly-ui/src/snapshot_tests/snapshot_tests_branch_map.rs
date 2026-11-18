//! Branch map snapshots: the list with its badges and warning marks, a selected branch's shared
//! files (a conflicting block, a clean one, one past the shown limit), the empty, loading and
//! failed views, the stacked layout of a narrow window and the German view; and what its rows,
//! Close and Esc send. The maps are built by hand on the fixture's commits with times relative
//! to `NOW`, so the text never depends on when the fixture was made.

use super::snapshot_tests::{NOW, harness, harness_sized, remove_fixture, with_repo};
use crate::snapshot_check::CheckSnapshot;
use eframe::egui::{self, Key};
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::branch_map::{BranchMap, Empty, Forecast, Pair, Request, Row, Tip};
use glowfly_core::branch_ref::BranchRef;
use glowfly_core::error::ReadFailed;
use glowfly_core::settings::ThemeChoice;
use glowfly_model::branch_map::BranchMapMsg;
use glowfly_model::msg::Msg;
use glowfly_model::state::AppState;
use glowfly_model::state::branch_map::{BranchMapView, SHOWN_FILES};
use std::path::PathBuf;

fn commit(st: &AppState, summary: &str) -> Oid {
    st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == summary).unwrap_or_else(|| panic!("{summary}")).oid
}

fn local(name: &str) -> BranchRef {
    BranchRef::Local(name.into())
}

#[allow(clippy::too_many_arguments)]
fn row(name: &str, tip: Oid, ago: i64, ahead: usize, behind: usize, base: Option<Oid>, files: &[&str], total: usize) -> Row {
    let files = files.iter().map(|f| (*f).to_owned()).collect();
    Row { branch: local(name), tip, time: NOW - ago, ahead, behind, base, files, files_total: total }
}

fn paths(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("src/module_{i:02}/mod.rs")).collect()
}

/// main, the checked-out `feature/login` (conflicts with main, with `fix-build` and shares many
/// files with `old-ui`), `fix-build`, a merged branch, a copy of main and an unrelated one.
fn sample(st: &AppState) -> BranchMap {
    let (merge, search, fix) =
        (commit(st, "Merge branch 'login-page'"), commit(st, "Search results page"), commit(st, "Fix build on Windows"));
    let (skeleton, login) = (commit(st, "Add app skeleton"), commit(st, "Add login page"));
    let files = ["README.md", "src/app.rs", "src/login.rs", "src/ui/form.rs"];
    let rows = vec![
        row("main", merge, 3_600, 0, 0, Some(merge), &[], 0),
        row("feature/login", search, 2 * 86_400, 5, 3, Some(skeleton), &files, 4),
        row("fix-build", fix, 5 * 86_400, 2, 1, Some(skeleton), &["src/app.rs", "src/lib.rs"], 2),
        row("old-ui", login, 40 * 86_400, 1, 3, Some(skeleton), &["src/app.rs"], 1),
        row("merged-search", skeleton, 90 * 86_400, 0, 4, Some(skeleton), &[], 0),
        row("copy-of-main", merge, 3_600, 0, 0, Some(merge), &[], 0),
        row("island", skeleton, 400 * 86_400, 2, 0, None, &[], 0),
    ];
    let shared = |list: &[&str]| list.iter().map(|f| (*f).to_owned()).collect::<Vec<_>>();
    let many = paths(SHOWN_FILES + 4);
    let pairs = vec![
        Pair {
            a: 0,
            b: 1,
            shared: shared(&["README.md", "src/app.rs", "src/login.rs"]),
            forecast: Forecast::Conflicts(shared(&["src/app.rs", "src/login.rs"])),
        },
        Pair { a: 1, b: 2, shared: shared(&["src/app.rs"]), forecast: Forecast::Conflicts(shared(&["src/app.rs"])) },
        Pair { a: 1, b: 3, shared: many, forecast: Forecast::Clean },
        Pair { a: 1, b: 4, shared: shared(&["README.md"]), forecast: Forecast::Unknown("object not found".into()) },
    ];
    BranchMap { rows, pairs, current: Some(1), total: 57, merged: 4 }
}

fn request(st: &AppState) -> Request {
    Request { main: Tip { branch: local("main"), oid: commit(st, "Merge branch 'login-page'") }, branches: Vec::new(), current: None }
}

fn opened(theme: ThemeChoice, name: &str, selected: Option<&str>) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let map = sample(&st);
    st.session.branch_map.view = Some(BranchMapView {
        generation: 1,
        loading: false,
        plan: Ok(request(&st)),
        result: Some(Ok(Box::new(map))),
        selected: selected.map(local),
    });
    (st, dir)
}

fn shows(st: &mut AppState, plan: Result<Request, Empty>, loading: bool, result: Option<Result<Box<BranchMap>, ReadFailed>>) {
    st.session.branch_map.view = Some(BranchMapView { generation: 1, loading, plan, result, selected: None });
}

fn sent(h: &egui_kittest::Harness<'_, (AppState, Vec<Msg>)>, want: fn(&BranchMapMsg) -> bool) -> bool {
    h.state().1.iter().any(|m| matches!(m, Msg::BranchMap(b) if want(b)))
}

#[test]
fn branch_map_list() {
    let (st, dir) = opened(ThemeChoice::Dark, "branch-map-list", None);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("branch_map_list");
    h.get_by_label_contains("fix-build").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, BranchMapMsg::Select(b) if *b == BranchRef::Local("fix-build".into()))));
    h.get_by_label("Close").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, BranchMapMsg::Close)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn rows_are_named_with_their_numbers() {
    let (st, dir) = opened(ThemeChoice::Dark, "branch-map-names", Some("feature/login"));
    let mut h = harness(st, true);
    h.run();
    let name = "feature/login, checked out: 3 behind, 5 ahead, 4 files changed, last commit 2 days ago, would conflict with main";
    assert!(h.query_by_label(name).is_some(), "the checked-out row's name");
    assert!(h.query_by_label_contains("island: No shared history with main").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn esc_closes_it() {
    let (st, dir) = opened(ThemeChoice::Dark, "branch-map-esc", None);
    let mut h = super::snapshot_tests_esc::app_harness(st);
    h.run();
    h.press_key(Key::Escape);
    h.run();
    assert!(sent(&h, |m| matches!(m, BranchMapMsg::Close)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn branch_map_selected() {
    let (st, dir) = opened(ThemeChoice::Dark, "branch-map-selected", Some("feature/login"));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("branch_map_selected");
    assert!(h.query_by_label("and 4 more files").is_some(), "the block past the limit says how many it left out");
    assert!(h.query_by_label("Conflicts in 2 files").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn branch_map_empty() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "branch-map-empty");
    shows(&mut st, Err(Empty::OneBranch), false, None);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("branch_map_empty");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn branch_map_loading() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "branch-map-loading");
    let plan = Ok(request(&st));
    shows(&mut st, plan, true, None);
    let mut h = harness(st, true);
    // The spinner keeps repainting.
    h.run_steps(4);
    h.check_snapshot("branch_map_loading");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn branch_map_failed() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "branch-map-failed");
    let plan = Ok(request(&st));
    shows(&mut st, plan, false, Some(Err(ReadFailed::Said { said: "could not find object ab12cd3".into() })));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("branch_map_failed");
    h.get_by_label("Try again").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, BranchMapMsg::Retry)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn branch_map_narrow() {
    let (st, dir) = opened(ThemeChoice::Dark, "branch-map-narrow", Some("feature/login"));
    let mut h = harness_sized(st, true, egui::vec2(900.0, 820.0));
    h.run();
    h.check_snapshot("branch_map_narrow");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn branch_map_de() {
    let (mut st, dir) = opened(ThemeChoice::Dark, "branch-map-de", Some("feature/login"));
    st.settings.language = "de".into();
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("branch_map_de");
    for german in ["Branch-Karte", "Konflikte in 2 Dateien", "und 4 weitere Dateien"] {
        assert!(h.query_by_label(german).is_some(), "{german}");
    }
    drop(h);
    remove_fixture(&dir);
}
