//! The conflict-check popover and report dialog with the long, unbreakable branch names a
//! release bot makes (issue #523): the text lays out unstretched, the names are cut with the
//! full name on hover, the short action labels keep the popover's width, and the popover stays
//! inside the window.

use super::snapshot_tests::{git, harness, harness_sized, reload, remove_fixture, with_repo};
use crate::msg::{JobMsg, Msg};
use crate::state::conflict_check::{ConflictReport, current_request};
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use glowfly_core::conflict_check::{Prediction, TargetKind};
use glowfly_core::gitcli::args;
use glowfly_core::repo::LoadOptions;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

const BRANCH: &str = "release-please--branches--main--components--glowfly";
const UPSTREAM: &str = "origin/release-please--branches--main--components--glowfly";

/// The fixture on BRANCH (made from search), tracking UPSTREAM a commit past main, whose automatic
/// check against UPSTREAM found conflicts in `paths`.
fn on_long_branch(theme: ThemeChoice, name: &str, paths: &[&str]) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let when = "2026-01-10T10:00:00+00:00";
    // The upstream: a release commit on main, which the branch lacks.
    git(&dir, &["checkout", "-q", "-b", "release", "main"], when);
    std::fs::write(dir.join("CHANGELOG.md"), "# Changelog\n").unwrap();
    git(&dir, &["add", "CHANGELOG.md"], when);
    git(&dir, &["commit", "-q", "-m", "chore(main): release 0.4.0"], when);
    git(&dir, &["remote", "add", "origin", "https://example.invalid/demo.git"], when);
    git(&dir, &["update-ref", &format!("refs/remotes/{UPSTREAM}"), "release"], when);
    git(&dir, &["checkout", "-q", "-b", BRANCH, "search"], when);
    git(&dir, &["branch", "-q", "-D", "release"], when);
    git(&dir, &["branch", "-q", "--set-upstream-to", UPSTREAM], when);
    reload(&mut st, &dir, &LoadOptions::all(500));
    let request = current_request(st.snap.as_ref().unwrap()).expect("the branch compares with its upstream");
    assert_eq!((request.target.name.as_str(), request.target.kind), (UPSTREAM, TargetKind::Upstream));
    let generation = st.session.reads.conflict_checks.start().unwrap();
    let result = Ok(Prediction::Conflicts(paths.iter().map(|p| p.to_string()).collect()));
    st.session.reads.conflict_checks.finish(generation, ConflictReport { request, result });
    (st, dir)
}

/// Opens the popover with the long names and checks it.
fn popover_long(theme: ThemeChoice, dark: bool, name: &str) {
    let (st, dir) = on_long_branch(theme, name, &["CHANGELOG.md"]);
    let mut h = harness(st, dark);
    open(&mut h);
    h.check_snapshot(name);
    // The names show cut, each named by its whole text (which shows on hover).
    assert!(h.query_all_by_label(BRANCH).next().is_some() && h.query_all_by_label(UPSTREAM).next().is_some());
    assert!(h.query_by_label("Compared with the last fetched state.").is_some());
    assert!(h.query_by_label("CHANGELOG.md").is_some());
    h.get_by_label("Merge upstream").click();
    h.run();
    let merge = vec![args(&["merge", "--", UPSTREAM])];
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Job(JobMsg::Git { cmds, .. }) if *cmds == merge)));
    drop(h);
    remove_fixture(&dir);
}

type H = egui_kittest::Harness<'static, (AppState, Vec<Msg>)>;

/// Clicks the chip, opening the popover.
fn open(h: &mut H) {
    h.run();
    h.get_by_label(&format!("Conflicts with {UPSTREAM}")).click();
    h.run();
}

#[test]
fn conflict_popover_long_dark() {
    popover_long(ThemeChoice::Dark, true, "conflict_popover_long_dark");
}

#[test]
fn conflict_popover_long_light() {
    popover_long(ThemeChoice::Light, false, "conflict_popover_long_light");
}

/// The report dialog for the long names' check.
fn report_long(theme: ThemeChoice, dark: bool, name: &str) {
    let (mut st, dir) = on_long_branch(theme, name, &["CHANGELOG.md", "Cargo.toml"]);
    let current = current_request(st.snap.as_ref().unwrap());
    let report = st.session.reads.conflict_checks.shown(current.as_ref()).cloned().expect("the check");
    st.modal = Some(Modal::ConflictReport(report));
    let mut h = harness(st, dark);
    h.run();
    h.check_snapshot(name);
    assert!(h.query_by_label("Merge upstream").is_some());
    assert!(h.query_by_label("Rebase onto upstream").is_some());
    assert!(h.query_by_label("2 files changed on both sides:").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn conflict_report_long_dark() {
    report_long(ThemeChoice::Dark, true, "conflict_report_long_dark");
}

#[test]
fn conflict_report_long_light() {
    report_long(ThemeChoice::Light, false, "conflict_report_long_light");
}

#[test]
fn conflict_popover_narrow_dark() {
    // The app's least window: the popover keeps its width and its margin from the edge.
    let (st, dir) = on_long_branch(ThemeChoice::Dark, "conflict-popover-narrow", &["CHANGELOG.md"]);
    let mut h = harness_sized(st, true, egui::vec2(960.0, 540.0));
    open(&mut h);
    h.check_snapshot("conflict_popover_narrow_dark");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn conflict_popover_merge_tip_dark() {
    let (st, dir) = on_long_branch(ThemeChoice::Dark, "conflict-popover-tip", &["CHANGELOG.md"]);
    let mut h = harness(st, true);
    open(&mut h);
    h.get_by_label("Merge upstream").hover();
    for _ in 0..4 {
        h.run();
    }
    assert!(h.query_by_label_contains(&format!("git merge -- {UPSTREAM}")).is_some(), "the command on hover");
    assert!(h.query_by_label_contains("Merges it into the checked-out branch. Runs:").is_some(), "what it does");
    h.check_snapshot("conflict_popover_merge_tip_dark");
    drop(h);
    remove_fixture(&dir);
}
