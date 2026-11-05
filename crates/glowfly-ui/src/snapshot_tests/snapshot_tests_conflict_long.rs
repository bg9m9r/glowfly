//! The conflict-check popover and report dialog with the long, unbreakable branch names a
//! release bot makes (issue #523): the text lays out unstretched, the names are cut with the
//! full name on hover, the short action labels keep the popover's width, and the popover stays
//! inside the window.

use super::snapshot_tests::{git, harness, harness_sized, reload, remove_fixture, with_repo};
use crate::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use glowfly_core::branch_ops::merge_cmd;
use glowfly_core::branch_ref::BranchRef;
use glowfly_core::conflict_check::{Prediction, TargetKind};
use glowfly_core::gitcli::args;
use glowfly_core::repo::LoadOptions;
use glowfly_core::settings::ThemeChoice;
use glowfly_model::msg::{JobMsg, Msg};
use glowfly_model::state::conflict_check::{ConflictReport, current_request};
use glowfly_model::state::{AppState, Modal};
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
    assert_eq!((request.target.name(), request.target.kind), (UPSTREAM, TargetKind::Upstream));
    let generation = st.session.reads.conflict_checks.start().unwrap();
    let result = Ok(Prediction::Conflicts(paths.iter().map(|p| p.to_string()).collect()));
    st.session.reads.conflict_checks.finish(generation, ConflictReport { request, result });
    (st, dir)
}

/// Opens the popover with the long names and checks it.
fn popover_long(theme: ThemeChoice, dark: bool, name: &str) {
    let (st, dir) = on_long_branch(theme, name, &["CHANGELOG.md"]);
    let merge_into = st.snap.as_ref().unwrap().merge_into.clone();
    let mut h = harness(st, dark);
    open(&mut h);
    h.check_snapshot(name);
    // The names show cut, each named by its whole text (which shows on hover).
    // The names show cut; screen readers hear each whole, with its role.
    assert!(h.query_by_label(&format!("Branch: {BRANCH}")).is_some());
    assert!(h.query_by_label(&format!("Upstream: {UPSTREAM}")).is_some());
    assert!(h.query_by_label("Compared with the last fetched state.").is_some());
    assert!(h.query_by_label("CHANGELOG.md").is_some());
    h.get_by_label("Merge upstream").click();
    h.run();
    let merge = vec![merge_cmd(&BranchRef::Remote(UPSTREAM.into()), merge_into.as_deref())];
    let title = format!("Merge remote-tracking branch '{UPSTREAM}' into {BRANCH}");
    assert_eq!(merge[0], args(&["merge", "-m", &title, "--", &format!("refs/remotes/{UPSTREAM}")]));
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

#[test]
fn chosen_long_names_read_whole_on_the_buttons() {
    // A check the branch menus asked for: the labels cut the names, screen readers hear them whole.
    let (mut st, dir) = on_long_branch(ThemeChoice::Dark, "conflict-report-chosen", &["CHANGELOG.md"]);
    let current = current_request(st.snap.as_ref().unwrap());
    let mut report = st.session.reads.conflict_checks.shown(current.as_ref()).cloned().expect("the check");
    report.request.target.kind = TargetKind::Chosen;
    st.modal = Some(Modal::ConflictReport(report));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(&format!("Merge {UPSTREAM} into {BRANCH}")).is_some(), "Merge's whole name");
    assert!(h.query_by_label(&format!("Rebase {BRANCH} onto {UPSTREAM}")).is_some(), "Rebase's whole name");
    assert!(h.query_by_label(&format!("Compared with: {UPSTREAM}")).is_some(), "the target's role");
    drop(h);
    remove_fixture(&dir);
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
    let typed = format!("git merge -m \"Merge remote-tracking branch '{UPSTREAM}' into {BRANCH}\" -- refs/remotes/{UPSTREAM}");
    assert!(h.query_by_label_contains(&typed).is_some(), "the command on hover, quoted as typed");
    assert!(h.query_by_label_contains("Merges it into the checked-out branch. Runs:").is_some(), "what it does");
    h.check_snapshot("conflict_popover_merge_tip_dark");
    drop(h);
    remove_fixture(&dir);
}
