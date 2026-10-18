//! Comparing commits: one commit with the working directory (the panel, both rows selected), a
//! line of commits combined, Swap direction, the branch tips of a selection in the sidebar, and
//! Shift+arrows in the graph. Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{remove_fixture, render, with_repo};
use super::snapshot_tests_esc::app_harness;
use crate::msg::{Msg, RepoMsg, SelectionMsg};
use crate::state::selection::MultiSelection;
use crate::state::selection_span::Span;
use crate::state::selection_wip::WorkdirCompare;
use crate::state::{AppState, Selection};
use eframe::egui::{Event, Key, Modifiers};
use git2::Oid;
use std::path::PathBuf;
use trellis_core::repo;
use trellis_core::settings::ThemeChoice;

fn commit(st: &AppState, summary: &str) -> Oid {
    st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == summary).unwrap().oid
}

/// The merge on main compared with a working tree that changes app.txt, deletes login.txt and
/// adds new.txt (README.md is the fixture's own edit).
fn compared(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    std::fs::write(dir.join("app.txt"), "app\nfix\nmore\n").unwrap();
    std::fs::remove_file(dir.join("login.txt")).unwrap();
    std::fs::write(dir.join("new.txt"), "new\n").unwrap();
    let oid = commit(&st, "Merge branch 'login-page'");
    let mut w = WorkdirCompare::new(oid);
    w.changes = Some(repo::compare_workdir(&dir, oid).map_err(|e| e.to_string()));
    st.selection = Selection::VsWorkdir(w);
    (st, dir)
}

#[test]
fn a_commit_compared_with_the_working_directory() {
    let (st, dir) = compared(ThemeChoice::Dark, "compare-workdir");
    let Selection::VsWorkdir(w) = &st.selection else { unreachable!() };
    let c = w.changes.clone().unwrap().unwrap();
    let mut files: Vec<(&str, char)> = c.files.iter().map(|f| (f.path.as_str(), f.status)).collect();
    // libgit2 orders paths ignoring case where the file system does.
    files.sort_by_key(|(p, _)| p.to_lowercase());
    assert_eq!(files, [("app.txt", 'M'), ("login.txt", 'D'), ("new.txt", 'A'), ("README.md", 'M')]);
    assert!(!c.in_workdir("login.txt"));
    render("compare_workdir_panel", st, true);
    remove_fixture(&dir);

    // The graph draws the WIP row and the commit selected.
    let (st, dir) = compared(ThemeChoice::Light, "compare-workdir-light");
    render("compare_workdir_graph_light", st, false);
    remove_fixture(&dir);
}

/// `summaries` selected, with the comparison's file list loaded (`swapped` runs it the other way).
fn selected(theme: ThemeChoice, name: &str, summaries: &[&str], swapped: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let oids: Vec<Oid> = summaries.iter().map(|s| commit(&st, s)).collect();
    let mut sel = MultiSelection::build(oids.iter().copied(), oids[0], st.snap.as_ref().unwrap());
    if let Selection::Commits(m) = &mut sel {
        m.swapped = swapped;
        if let Some((from, to)) = m.compare_ends() {
            m.changes = Some(repo::compare(&dir, from, to).map_err(|e| e.to_string()));
        }
    }
    st.selection = sel;
    (st, dir)
}

#[test]
fn a_line_of_commits_combined() {
    // Each is the first parent of the one above it; main and search have their tips in it.
    let (st, dir) =
        selected(ThemeChoice::Dark, "compare-range", &["Search results page", "Merge branch 'login-page'", "Fix build on Windows"], false);
    let Selection::Commits(m) = &st.selection else { unreachable!() };
    assert_eq!(m.span, Span::Range { base: commit(&st, "Add app skeleton"), newest: commit(&st, "Search results page") });
    render("graph_range_selected", st, true);
    remove_fixture(&dir);
}

#[test]
fn two_commits_swapped() {
    let (st, dir) = selected(ThemeChoice::Dark, "compare-swapped", &["Add login page", "Search results page"], true);
    let Selection::Commits(m) = &st.selection else { unreachable!() };
    assert_eq!(m.compare_ends(), Some((commit(&st, "Search results page"), commit(&st, "Add login page"))));
    render("graph_two_commits_swapped", st, true);
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
    st.selection = Selection::Commit(rows[1]);
    let mut h = app_harness(st);
    h.run();
    let msgs = arrow(&mut h, Modifiers::SHIFT, Key::ArrowDown);
    assert!(matches!(&msgs[..], [Msg::Selection(SelectionMsg::SelectCommitRange(o))] if *o == rows[2]), "{}", msgs.len());

    // From the top commit, Up meets the WIP row: nothing.
    h.state_mut().0.selection = Selection::Commit(rows[0]);
    assert!(arrow(&mut h, Modifiers::SHIFT, Key::ArrowUp).is_empty());
    // Without a commit selected, Shift+Down moves like Down.
    h.state_mut().0.selection = Selection::Wip;
    let msgs = arrow(&mut h, Modifiers::SHIFT, Key::ArrowDown);
    assert!(matches!(&msgs[..], [Msg::Repo(RepoMsg::SelectCommit(o))] if *o == rows[0]), "{}", msgs.len());
    drop(h);
    remove_fixture(&dir);
}
