//! Comparing commits: one commit with the working directory (the panel, both rows selected), a
//! line of commits combined, Swap direction, the branch tips of a selection in the sidebar, and
//! Shift+arrows in the graph. Shares the fixture repository and renderer with `snapshot_tests`.

use super::common::short;
use super::snapshot_tests::{remove_fixture, render, with_repo};
use super::snapshot_tests_esc::app_harness;
use crate::msg::{Msg, RepoMsg, SelectionMsg, StageMsg};
use crate::state::selection::MultiSelection;
use crate::state::selection_span::Span;
use crate::state::selection_wip::WorkdirCompare;
use crate::state::{AppState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Event, Key, Modifiers};
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::repo;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

fn commit(st: &AppState, summary: &str) -> Oid {
    st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == summary).unwrap().oid
}

/// The merge on main compared with a working tree that changes app.txt, deletes login.txt and
/// adds new.txt (README.md is the fixture's own edit); `swapped` runs it from the working tree.
pub(crate) fn compared(theme: ThemeChoice, name: &str, swapped: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    std::fs::write(dir.join("app.txt"), "app\nfix\nmore\n").unwrap();
    std::fs::remove_file(dir.join("login.txt")).unwrap();
    std::fs::write(dir.join("new.txt"), "new\n").unwrap();
    let oid = commit(&st, "Merge branch 'login-page'");
    let mut w = WorkdirCompare::new(oid);
    w.swapped = swapped;
    let changes = if swapped { repo::compare_from_workdir(&dir, oid) } else { repo::compare_workdir(&dir, oid) };
    w.changes = Some(changes.map_err(|e| e.to_string()));
    st.session.selection = Selection::VsWorkdir(w);
    (st, dir)
}

#[test]
fn a_commit_compared_with_the_working_directory() {
    let (st, dir) = compared(ThemeChoice::Dark, "compare-workdir", false);
    let Selection::VsWorkdir(w) = &st.session.selection else { unreachable!() };
    let c = w.changes.clone().unwrap().unwrap();
    let mut files: Vec<(&str, char)> = c.files.iter().map(|f| (f.path.as_str(), f.status)).collect();
    // libgit2 orders paths ignoring case where the file system does.
    files.sort_by_key(|(p, _)| p.to_lowercase());
    assert_eq!(files, [("app.txt", 'M'), ("login.txt", 'D'), ("new.txt", 'A'), ("README.md", 'M')]);
    assert!(!c.in_workdir("login.txt"));
    // The panel, and the graph with the WIP row and the commit selected.
    render("compare_workdir_panel", st, true);
    remove_fixture(&dir);
}

#[test]
fn the_working_directory_compared_with_a_commit() {
    let (st, dir) = compared(ThemeChoice::Dark, "compare-workdir-swapped", true);
    let Selection::VsWorkdir(w) = &st.session.selection else { unreachable!() };
    let c = w.changes.clone().unwrap().unwrap();
    let status = |p: &str| c.files.iter().find(|f| f.path == p).map(|f| f.status);
    assert_eq!((status("new.txt"), status("login.txt")), (Some('D'), Some('A')));
    render("compare_workdir_swapped", st, true);
    remove_fixture(&dir);
}

/// The comparison of `compared` with app.txt's diff open (the commit to the working tree).
fn workdir_diff(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = compared(ThemeChoice::Dark, name, false);
    let oid = commit(&st, "Merge branch 'login-page'");
    let target = repo::DiffTarget::ToWorkdir(oid, "app.txt".into());
    let mut diff = repo::load_diff(&dir, &target).unwrap();
    diff.intraline = glowfly_core::intraline::intraline(&diff);
    st.session.diff.view = Some((target, diff.into()));
    (st, dir)
}

#[test]
fn a_commit_to_working_tree_diff_reverts_hunks() {
    let (st, dir) = workdir_diff("compare-workdir-revert");
    let generation = st.hunk_ops_gen().expect("buttons enabled");
    let mut h = super::snapshot_tests::harness(st, true);
    h.run();
    h.check_snapshot("diff_to_workdir_revert");
    h.get_by_label("Revert hunk").click();
    h.run();
    let sent = h.state().1.iter().any(|m| matches!(m, Msg::Stage(StageMsg::RevertHunk { index: 0, generation: g }) if *g == generation));
    assert!(sent, "Revert hunk sends the revert");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn a_line_from_the_first_commit_combined() {
    // Each is the first parent of the one above it, down to the fixture's first commit.
    let summaries = ["Fix build on Windows", "Add app skeleton", "Initial commit"];
    let (st, dir) = selected(ThemeChoice::Dark, "compare-range-root", &summaries, false);
    let Selection::Commits(m) = &st.session.selection else { unreachable!() };
    assert_eq!(m.span, Span::Range { base: repo::empty_tree(), newest: commit(&st, "Fix build on Windows") });
    assert!(m.changes.as_ref().is_some_and(|c| c.as_ref().is_ok_and(|c| c.files.iter().all(|f| f.status == 'A'))));
    assert!(m.pick_blocked.is_some(), "the commits are on the current branch");
    let newest = short(commit(&st, "Fix build on Windows"));
    let mut h = super::snapshot_tests::harness(st, true);
    h.run();
    h.check_snapshot("graph_range_from_root");
    assert!(h.query_by_label("3 commits selected · Esc to clear").is_some(), "one heading with the Esc hint");
    // The disabled action says why on a line of its own, not only in its tooltip.
    assert!(h.query_by_label("Some selected commits are already on the current branch.").is_some());
    assert!(h.query_by_label("empty repository").is_some(), "the line starts at the empty tree");
    assert!(h.query_all_by_label(&newest).count() >= 2, "the newest commit ends the direction row (and is listed)");
    assert!(h.query_by_label("3 commits combined").is_some());
    h.get_by_label("Swap direction").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Selection(SelectionMsg::SwapCompare))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn a_line_from_a_shallow_boundary_explains() {
    // As if the clone's downloaded history stopped at its first commit.
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "compare-range-shallow");
    let root = commit(&st, "Initial commit");
    st.snap.as_mut().unwrap().shallow_roots = vec![root];
    let (st, dir) = select(st, dir, &["Fix build on Windows", "Add app skeleton", "Initial commit"], false);
    let Selection::Commits(m) = &st.session.selection else { unreachable!() };
    assert_eq!((m.span, m.compare_ends()), (Span::ShallowEnd, None));
    render("details_multi_shallow_end", st, true);
    remove_fixture(&dir);
}

#[test]
fn the_details_panel_compares_its_commit_with_the_working_directory() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "compare-details-button");
    let oid = commit(&st, "Fix build on Windows");
    st.session.selection = Selection::Commit(oid);
    st.session.detail = Some(repo::commit_detail(&dir, oid).unwrap());
    let mut h = super::snapshot_tests::harness(st, true);
    h.run();
    h.get_by_label("Compare with working directory").click();
    h.run();
    let sent = h.state().1.iter().any(|m| matches!(m, Msg::Selection(SelectionMsg::CompareWithWorkdir(o)) if *o == oid));
    assert!(sent, "the button starts the comparison");
    drop(h);

    // A commit that isn't a graph row (a stash selected in the sidebar) has no button.
    let (mut st, _) = with_repo(ThemeChoice::Dark, "compare-details-button");
    let mut d = repo::commit_detail(&dir, oid).unwrap();
    d.oid = Oid::from_str("1234567890123456789012345678901234567890").unwrap();
    st.session.selection = Selection::Commit(d.oid);
    st.session.detail = Some(d);
    let mut h = super::snapshot_tests::harness(st, true);
    h.run();
    assert!(h.query_by_label("Compare with working directory").is_none());
    drop(h);
    remove_fixture(&dir);
}

/// `summaries` selected, with the comparison's file list loaded (`swapped` runs it the other way).
pub(crate) fn selected(theme: ThemeChoice, name: &str, summaries: &[&str], swapped: bool) -> (AppState, PathBuf) {
    let (st, dir) = with_repo(theme, name);
    select(st, dir, summaries, swapped)
}

fn select(mut st: AppState, dir: PathBuf, summaries: &[&str], swapped: bool) -> (AppState, PathBuf) {
    let oids: Vec<Oid> = summaries.iter().map(|s| commit(&st, s)).collect();
    let mut sel = MultiSelection::build(oids.iter().copied(), oids[0], st.snap.as_ref().unwrap());
    if let Selection::Commits(m) = &mut sel {
        m.swapped = swapped;
        if let Some((from, to)) = m.compare_ends() {
            m.changes = Some(repo::compare(&dir, from, to).map_err(|e| e.to_string()));
        }
        super::snapshot_tests_select::load_stacked(m, &dir);
    }
    st.session.selection = sel;
    (st, dir)
}

#[test]
fn a_line_of_commits_combined() {
    // Each is the first parent of the one above it; main and search have their tips in it.
    let (st, dir) =
        selected(ThemeChoice::Dark, "compare-range", &["Search results page", "Merge branch 'login-page'", "Fix build on Windows"], false);
    let Selection::Commits(m) = &st.session.selection else { unreachable!() };
    assert_eq!(m.span, Span::Range { base: commit(&st, "Add app skeleton"), newest: commit(&st, "Search results page") });
    render("graph_range_selected", st, true);
    remove_fixture(&dir);
}

#[test]
fn two_commits_swapped() {
    let (st, dir) = selected(ThemeChoice::Dark, "compare-swapped", &["Add login page", "Search results page"], true);
    let Selection::Commits(m) = &st.session.selection else { unreachable!() };
    assert_eq!(m.compare_ends(), Some((commit(&st, "Search results page"), commit(&st, "Add login page"))));
    let mut h = super::snapshot_tests::harness(st, true);
    h.run();
    h.check_snapshot("graph_two_commits_swapped");
    assert!(h.query_by_label_contains("combined").is_none(), "a pair has no count line");
    assert!(h.query_by_label("Swap direction").is_some());
    drop(h);
    remove_fixture(&dir);
}

/// Presses an arrow key with `modifiers` in the whole app and returns the messages.
fn arrow(h: &mut egui_kittest::Harness<'_, (AppState, Vec<Msg>)>, modifiers: Modifiers, key: Key) -> Vec<Msg> {
    h.state_mut().1.clear();
    let input = h.input_mut();
    input.modifiers = modifiers;
    input.events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.step();
    h.input_mut().modifiers = Modifiers::NONE;
    std::mem::take(&mut h.state_mut().1)
}

#[test]
fn shift_arrows_extend_the_selection() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "compare-shift-keys");
    let rows: Vec<Oid> = st.snap.as_ref().unwrap().commits.iter().map(|c| c.oid).collect();
    st.session.selection = Selection::Commit(rows[1]);
    let mut h = app_harness(st);
    h.run_steps(4);
    let msgs = arrow(&mut h, Modifiers::SHIFT, Key::ArrowDown);
    assert!(matches!(&msgs[..], [Msg::Selection(SelectionMsg::SelectCommitRange(o))] if *o == rows[2]), "{}", msgs.len());

    // From the top commit, Up meets the WIP row: nothing.
    h.state_mut().0.session.selection = Selection::Commit(rows[0]);
    assert!(arrow(&mut h, Modifiers::SHIFT, Key::ArrowUp).is_empty());
    // Without a commit selected, Shift+Down moves like Down.
    h.state_mut().0.session.selection = Selection::Wip;
    let msgs = arrow(&mut h, Modifiers::SHIFT, Key::ArrowDown);
    assert!(matches!(&msgs[..], [Msg::Repo(RepoMsg::SelectCommit(o))] if *o == rows[0]), "{}", msgs.len());
    drop(h);
    remove_fixture(&dir);
}
