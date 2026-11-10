//! Several files picked in Conflicted Files or Resolved Files during a merge: picked rows draw
//! selected with a gap between them, a Ctrl/Cmd+click sends the pick message without opening
//! the file, and a picked row's menu is the selection menu (Mark N files resolved, or Mark N
//! files conflicted... asking first, off while a pick can't go back). Made-up conflicted and
//! resolved files are injected into the shared fixture's snapshot.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_wip_picks::{click_with, labels, right_click, row};
use crate::snapshot_check::CheckSnapshot;
use eframe::egui::{Modifiers, PointerButton};
use egui_kittest::kittest::Queryable;
use git2::RepositoryState;
use glowfly_core::repo::{FileEntry, ResolvedFile};
use glowfly_core::settings::ThemeChoice;
use glowfly_model::msg::{ConflictMsg, Msg};
use glowfly_model::state::conflict_picks::ConflictPicks;
use glowfly_model::state::wip_lists::PickHow;
use glowfly_model::state::{AppState, Selection};
use std::path::PathBuf;

const CONFLICTED: [&str; 3] = ["alpha.txt", "beta.txt", "gamma.txt"];
const RESOLVED: [&str; 3] = ["delta.txt", "epsilon.txt", "zeta.txt"];

/// The fixture mid-merge with `conflicted` files and `resolved` ones (both sides recorded, staged), `picks` picked in one of the two lists.
fn mid_merge(theme: ThemeChoice, name: &str, conflicted: &[&str], resolved: &[&str], picks: ConflictPicks) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let snap = st.snap.as_mut().unwrap();
    snap.state = RepositoryState::Merge;
    snap.unstaged = conflicted.iter().map(|p| FileEntry::new(*p, '!')).collect();
    snap.staged = resolved.iter().map(|p| FileEntry::new(*p, 'M')).collect();
    st.session.conflict_files.resolved =
        resolved.iter().map(|p| ResolvedFile { entry: FileEntry::new(*p, ' '), both_sides: true }).collect();
    st.session.selection = Selection::Wip;
    st.cues.conflict_picks = Some(picks);
    (st, dir)
}

fn picks(resolved: bool, paths: &[&str]) -> ConflictPicks {
    ConflictPicks { resolved, anchor: paths[0].to_owned(), paths: paths.iter().map(|p| (*p).to_owned()).collect() }
}

/// Alpha and gamma picked in Conflicted Files, beta between them not.
fn conflicted_gaps(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    mid_merge(theme, name, &CONFLICTED, &[], picks(false, &["alpha.txt", "gamma.txt"]))
}

/// Delta and zeta picked in Resolved Files, epsilon between them not; alpha still conflicted,
/// so the step that lists them is open.
fn resolved_gaps(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    mid_merge(theme, name, &CONFLICTED[..1], &RESOLVED, picks(true, &["delta.txt", "zeta.txt"]))
}

#[test]
fn conflict_picks_gaps() {
    let (st, dir) = conflicted_gaps(ThemeChoice::Dark, "conflict-picks-gaps");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("conflict_picks_gaps_dark");
    let at = row(&h, "beta.txt");
    let msgs = click_with(&mut h, at, PointerButton::Primary, Modifiers::COMMAND);
    let picked =
        |m: &Msg| matches!(m, Msg::Conflict(ConflictMsg::Pick { resolved: false, path, how: PickHow::Toggle }) if path == "beta.txt");
    assert!(msgs.iter().any(picked));
    assert!(!msgs.iter().any(|m| matches!(m, Msg::Conflict(ConflictMsg::OpenConflict(_)))), "a pick doesn't open the file");
    let msgs = click_with(&mut h, at, PointerButton::Primary, Modifiers::SHIFT);
    assert!(msgs.iter().any(|m| matches!(m, Msg::Conflict(ConflictMsg::Pick { how: PickHow::Range, .. }))));
    let msgs = click_with(&mut h, at, PointerButton::Primary, Modifiers::NONE);
    assert!(msgs.iter().any(|m| matches!(m, Msg::Conflict(ConflictMsg::OpenConflict(p)) if p == "beta.txt")));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn conflict_picks_menu() {
    let (st, dir) = conflicted_gaps(ThemeChoice::Dark, "conflict-picks-menu");
    let mut h = harness(st, true);
    h.run();
    let at = row(&h, "gamma.txt");
    right_click(&mut h, at);
    assert!(h.query_by_label("Mark 2 files resolved").is_some(), "{:?}", labels(&h));
    assert!(h.query_by_label("Copy path").is_none(), "not the single-file menu");
    h.check_snapshot("conflict_picks_menu_dark");
    h.get_by_label("Mark 2 files resolved").click();
    h.run_ok();
    let want = ["alpha.txt", "gamma.txt"].map(String::from).to_vec();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Conflict(ConflictMsg::MarkResolved(p)) if *p == want)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn resolved_picks_gaps() {
    let (st, dir) = resolved_gaps(ThemeChoice::Light, "resolved-picks-gaps");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("resolved_picks_gaps_light");
    let at = row(&h, "epsilon.txt");
    let msgs = click_with(&mut h, at, PointerButton::Primary, Modifiers::COMMAND);
    assert!(
        msgs.iter()
            .any(|m| matches!(m, Msg::Conflict(ConflictMsg::Pick { resolved: true, path, how: PickHow::Toggle }) if path == "epsilon.txt"))
    );
    assert!(!msgs.iter().any(|m| matches!(m, Msg::Repo(_))), "a pick doesn't open the diff");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn resolved_picks_menu() {
    let (st, dir) = resolved_gaps(ThemeChoice::Dark, "resolved-picks-menu");
    let mut h = harness(st, true);
    h.run();
    let at = row(&h, "zeta.txt");
    right_click(&mut h, at);
    assert!(h.query_by_label("Mark 2 files conflicted…").is_some(), "{:?}", labels(&h));
    h.check_snapshot("resolved_picks_menu_dark");
    h.get_by_label("Mark 2 files conflicted…").click();
    h.run_ok();
    let want = ["delta.txt", "zeta.txt"].map(String::from).to_vec();
    let asks = |m: &Msg| match m {
        Msg::Confirm { then, .. } => matches!(&**then, Msg::Conflict(ConflictMsg::MarkConflicted(p)) if *p == want),
        _ => false,
    };
    assert!(h.state().1.iter().any(asks), "one confirmation for both");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn resolved_picks_menu_blocked() {
    let (mut st, dir) = resolved_gaps(ThemeChoice::Dark, "resolved-picks-blocked");
    st.session.conflict_files.resolved[2].both_sides = false;
    let mut h = harness(st, true);
    h.run();
    let at = row(&h, "delta.txt");
    right_click(&mut h, at);
    assert!(h.get_by_label("Mark 2 files conflicted…").is_disabled(), "zeta.txt has no record of both sides");
    h.check_snapshot("resolved_picks_menu_blocked");
    drop(h);
    remove_fixture(&dir);
}
