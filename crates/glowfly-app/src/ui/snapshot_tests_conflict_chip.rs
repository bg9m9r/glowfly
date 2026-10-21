//! The conflict-check chip under the real pointer: hovering shows its tooltip and a click
//! toggles its popover, also beside a long branch name, with captions off, with the terminal
//! panel open and in the light theme. These move the mouse (PointerMoved / PointerButton), which
//! goes through egui's hit-testing, unlike an accessibility click.

use crate::conflict_check::CheckRequest;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::conflict_check::{ConflictReport, current_request};
use crate::state::terminal::SessionStatus;
use crate::terminal::TerminalMsg;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{git, harness, reload, remove_fixture, with_repo};
use crate::ui::snapshot_tests_terminal::with_session;
use eframe::egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::conflict_check::{Prediction, TargetKind};
use glowfly_core::repo::LoadOptions;
use glowfly_core::settings::ThemeChoice;
use std::path::Path;

const FILES: [&str; 3] = ["src/app.rs", "README.md", "docs/guide with space.md"];

/// Checks out `branch` (created from search unless it is search) and records a finished
/// automatic check against main that found conflicts in FILES.
fn conflicts_on(st: &mut AppState, dir: &Path, branch: &str) {
    let when = "2026-01-10T10:00:00+00:00";
    if branch == "search" {
        git(dir, &["checkout", "-q", "search"], when);
    } else {
        git(dir, &["checkout", "-q", "-b", branch, "search"], when);
    }
    reload(st, dir, &LoadOptions::all(500));
    let request: CheckRequest = current_request(st.snap.as_ref().unwrap()).expect("compares with main");
    assert_eq!((request.target.name.as_str(), request.target.kind), ("main", TargetKind::DefaultBranch));
    let generation = st.conflict_checks.start().unwrap();
    let result = Ok(Prediction::Conflicts(FILES.iter().map(|p| p.to_string()).collect()));
    st.conflict_checks.finish(generation, ConflictReport { request, result });
}

/// The chip (the popover's heading has the same text).
fn chip<'h>(h: &'h Harness<'_, (AppState, Vec<Msg>)>) -> egui_kittest::kittest::Node<'h> {
    h.get_by_role_and_label(Role::Button, "Conflicts with main")
}

/// Hovers the chip until its tooltip shows (the app's 400 ms delay), checking its text.
fn hover_shows_tooltip(h: &mut Harness<'_, (AppState, Vec<Msg>)>, case: &str) {
    chip(h).hover();
    for _ in 0..4 {
        h.run();
    }
    assert!(h.query_by_label_contains("would stop with conflicts in 3 files").is_some(), "{case}: the tooltip");
    assert!(h.query_by_label_contains("main is this repository's default branch").is_some(), "{case}: the target note");
}

/// Clicks the chip with the pointer: the popover opens, and a second click closes it. Neither
/// click sends anything.
fn click_toggles_popover(h: &mut Harness<'_, (AppState, Vec<Msg>)>, case: &str) {
    h.state_mut().1.clear();
    chip(h).simulate_click();
    h.run();
    assert!(h.query_by_label("Check again").is_some(), "{case}: the popover opened");
    assert!(h.query_by_label("docs/guide with space.md").is_some(), "{case}: its file list");
    chip(h).simulate_click();
    h.run();
    assert!(h.query_by_label("Check again").is_none(), "{case}: a second click closes it");
    // The terminal panel reports its grid size; nothing else is sent.
    let sent: Vec<_> = h.state().1.iter().filter(|m| !matches!(m, Msg::Terminal(TerminalMsg::Resize { .. }))).collect();
    assert!(sent.is_empty(), "{case}: the clicks send nothing: {} sent", sent.len());
}

#[test]
fn chip_hover_and_click_with_the_pointer() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "conflict-chip-pointer");
    conflicts_on(&mut st, &dir, "search");
    let mut h = harness(st, true);
    h.run();
    hover_shows_tooltip(&mut h, "plain");
    h.check_snapshot("conflict_chip_tooltip");
    click_toggles_popover(&mut h, "plain");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn chip_pointer_variants() {
    type Setup = fn(&mut AppState, &Path);
    let cases: [(&str, ThemeChoice, Setup); 4] = [
        ("terminal", ThemeChoice::Dark, |_, _| {}),
        ("no captions", ThemeChoice::Dark, |st, _| st.settings.toolbar_labels = false),
        ("long branch", ThemeChoice::Dark, |st, dir| conflicts_on(st, dir, "feature/a-very-long-branch-name-that-gets-cut")),
        ("light", ThemeChoice::Light, |_, _| {}),
    ];
    for (case, theme, setup) in cases {
        let name = format!("conflict-chip-{}", case.replace(' ', "-"));
        let dark = theme == ThemeChoice::Dark;
        let (mut st, dir) = if case == "terminal" { with_session(theme, &name, SessionStatus::Running) } else { with_repo(theme, &name) };
        conflicts_on(&mut st, &dir, "search");
        setup(&mut st, &dir);
        let mut h = harness(st, dark);
        h.run();
        hover_shows_tooltip(&mut h, case);
        click_toggles_popover(&mut h, case);
        drop(h);
        remove_fixture(&dir);
    }
}
