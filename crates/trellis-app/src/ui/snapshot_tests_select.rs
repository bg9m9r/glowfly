//! Several commits selected: the graph rows, the details panel, the two-commit diff, the
//! multi-commit menu and Esc. Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::SelectionMsg;
use crate::patch::PatchMsg;
use crate::state::selection::MultiSelection;
use crate::state::{AppState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use std::path::PathBuf;
use trellis_core::intraline;
use trellis_core::repo::{self, DiffTarget};
use trellis_core::settings::ThemeChoice;

/// The fixture's commit with this summary.
fn commit(st: &AppState, summary: &str) -> Oid {
    st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == summary).unwrap().oid
}

/// `summaries` selected, as Ctrl-clicks would leave them, with the two-commit file list loaded.
fn with_selection(theme: ThemeChoice, name: &str, summaries: &[&str]) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let oids: Vec<Oid> = summaries.iter().map(|s| commit(&st, s)).collect();
    let mut sel = MultiSelection::build(oids.iter().copied(), oids[0], st.snap.as_ref().unwrap());
    if let Selection::Commits(m) = &mut sel
        && let Some((older, newer)) = m.pair()
    {
        m.changes = Some(repo::compare(&dir, older, newer).map_err(|e| e.to_string()));
    }
    st.selection = sel;
    (st, dir)
}

fn multi(st: &AppState) -> &MultiSelection {
    let Selection::Commits(m) = &st.selection else { panic!("not a multi-selection") };
    m
}

#[test]
fn graph_two_commits_selected() {
    let (st, dir) = with_selection(ThemeChoice::Dark, "multi-two", &["Add login page", "Search results page"]);
    let m = multi(&st);
    let names: Vec<(&str, char)> =
        m.changes.as_ref().unwrap().as_ref().unwrap().files.iter().map(|f| (f.path.as_str(), f.status)).collect();
    assert_eq!(names, [("app.txt", 'M'), ("search.txt", 'A')]);
    let (older, newer) = m.pair().unwrap();
    assert_eq!((older, newer), (commit(&st, "Add login page"), commit(&st, "Search results page")));
    render("graph_two_commits_selected", st, true);

    // The same selection with the diff of app.txt between the two open.
    let (mut st, _) = with_selection(ThemeChoice::Dark, "multi-two", &["Add login page", "Search results page"]);
    let target = DiffTarget::Between(older, newer, "app.txt".into());
    let mut diff = repo::load_diff(&dir, &target).unwrap();
    diff.intraline = intraline::intraline(&diff);
    st.diff = Some((target, diff.into()));
    render("graph_two_commits_diff", st, true);
    remove_fixture(&dir);
}

#[test]
fn graph_many_commits_selected_light() {
    let (st, dir) =
        with_selection(ThemeChoice::Light, "multi-many", &["Merge branch 'login-page'", "Fix build on Windows", "Add login page"]);
    let m = multi(&st);
    assert_eq!(m.len(), 3);
    assert!(m.pick_blocked.is_some(), "a merge commit can't be picked with the others");
    render("graph_many_commits_selected_light", st, false);
    remove_fixture(&dir);
}

pub(super) fn click(h: &mut Harness<'_, (AppState, Vec<Msg>)>, pos: Pos2, button: PointerButton) {
    for pressed in [true, false] {
        let events = &mut h.input_mut().events;
        events.push(Event::PointerMoved(pos));
        events.push(Event::PointerButton { pos, button, pressed, modifiers: Modifiers::NONE });
        h.step();
    }
    h.run();
}

/// The y of graph row `row` (the WIP row is 0) in the window the harness draws.
pub(super) fn row_y(row: usize) -> f32 {
    ROWS_TOP + 30.0 * row as f32 + 15.0
}

/// Where the graph's first row starts (below the tab bar, toolbar and column header).
const ROWS_TOP: f32 = 109.0;

#[test]
fn graph_multi_menu() {
    let (mut st, dir) = with_selection(ThemeChoice::Dark, "multi-menu", &["Add login page", "Search results page"]);
    // As on a branch without these commits, so the cherry-pick is offered.
    if let Selection::Commits(m) = &mut st.selection {
        m.pick_blocked = None;
    }
    let (login, search) = (commit(&st, "Add login page"), commit(&st, "Search results page"));
    let row_of = |o: Oid| st.snap.as_ref().unwrap().row_of[&o] + 1;
    let (search_row, fix_row) = (row_of(search), row_of(commit(&st, "Fix build on Windows")));
    let mut h = harness(st, true);
    h.run();
    click(&mut h, pos2(700.0, row_y(search_row)), PointerButton::Secondary);
    h.check_snapshot("graph_multi_menu");
    // The details panel has the same button; the menu's is the one over the graph.
    let in_menu = h.get_all_by_label("Cherry-pick 2 commits").find(|n| n.bounding_box().is_some_and(|b| b.x0 < 1000.0));
    in_menu.expect("the menu item").click();
    h.run();
    let picked = h.state().1.iter().find_map(|m| if let Msg::Selection(SelectionMsg::CherryPick(o)) = m { Some(o.clone()) } else { None });
    assert_eq!(picked, Some(vec![search, login]));
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::SelectCommit(_)))), "a right-click does not change the selection");

    // The menu offers the selection as one patch.
    click(&mut h, pos2(700.0, row_y(search_row)), PointerButton::Secondary);
    h.state_mut().1.clear();
    let in_menu = h.get_all_by_label("Create patch from 2 commits…").find(|n| n.bounding_box().is_some_and(|b| b.x0 < 1000.0));
    in_menu.expect("the menu item").click();
    h.run();
    let exported = h.state().1.iter().find_map(|m| if let Msg::Patch(PatchMsg::Export(o)) = m { Some(o.clone()) } else { None });
    assert_eq!(exported, Some(vec![search, login]));
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::SelectCommit(_)))), "a right-click does not change the selection");
    // Or copies it.
    click(&mut h, pos2(700.0, row_y(search_row)), PointerButton::Secondary);
    h.state_mut().1.clear();
    h.get_by_label("Copy patch of 2 commits").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Patch(PatchMsg::Copy(o)) if *o == [search, login])));

    // Any other row gets the single-commit menu.
    click(&mut h, pos2(700.0, row_y(fix_row)), PointerButton::Secondary);
    assert!(h.query_by_label("Cherry-pick commit").is_some());
    assert_eq!(h.query_all_by_label("Cherry-pick 2 commits").count(), 1, "only the details panel's button");
    drop(h);
    remove_fixture(&dir);
}

/// A harness that only runs the shell's Esc handling.
fn esc_harness(st: AppState) -> Harness<'static, (AppState, Vec<Msg>)> {
    Harness::builder().build_state(|ctx, (st, msgs): &mut (AppState, Vec<Msg>)| super::escape_key(ctx, st, msgs), (st, Vec::new()))
}

fn press_esc(h: &mut Harness<'_, (AppState, Vec<Msg>)>) {
    let key = |pressed| Event::Key { key: egui::Key::Escape, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE };
    h.input_mut().events.extend([key(true), key(false)]);
    h.run();
}

#[test]
fn esc_collapses_after_closing_the_diff() {
    let (st, dir) = with_selection(ThemeChoice::Dark, "multi-esc", &["Add login page", "Search results page"]);
    let mut h = esc_harness(st);
    press_esc(&mut h);
    assert!(matches!(&h.state().1[..], [Msg::Selection(SelectionMsg::CollapseSelection)]));

    let (mut st, _) = with_selection(ThemeChoice::Dark, "multi-esc", &["Add login page", "Search results page"]);
    let (older, newer) = multi(&st).pair().unwrap();
    let target = DiffTarget::Between(older, newer, "app.txt".into());
    st.diff = Some((target.clone(), repo::load_diff(&dir, &target).unwrap().into()));
    let mut h = esc_harness(st);
    press_esc(&mut h);
    assert!(matches!(&h.state().1[..], [Msg::Repo(RepoMsg::CloseDiff)]));
    remove_fixture(&dir);
}

#[test]
fn graph_commit_menu() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "commit-menu");
    let (fix, merge) = (commit(&st, "Fix build on Windows"), commit(&st, "Merge branch 'login-page'"));
    let row_of = |o: Oid| st.snap.as_ref().unwrap().row_of[&o] + 1;
    let (fix_row, merge_row) = (row_of(fix), row_of(merge));
    let root_row = row_of(commit(&st, "Initial commit"));
    let mut h = harness(st, true);
    h.run();
    click(&mut h, pos2(700.0, row_y(fix_row)), PointerButton::Secondary);
    h.check_snapshot("graph_commit_menu");
    h.state_mut().1.clear();
    h.get_by_label("Create patch…").click();
    h.run();
    let exported = h.state().1.iter().find_map(|m| if let Msg::Patch(PatchMsg::Export(o)) = m { Some(o.clone()) } else { None });
    assert_eq!(exported, Some(vec![fix]));
    click(&mut h, pos2(700.0, row_y(fix_row)), PointerButton::Secondary);
    h.state_mut().1.clear();
    h.get_by_label("Copy patch").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Patch(PatchMsg::Copy(o)) if *o == [fix])));

    click(&mut h, pos2(700.0, row_y(fix_row)), PointerButton::Secondary);
    h.get_by_label("Copy link to this commit").click();
    h.run();
    let want = crate::deep_link::LinkTarget::Commit(fix);
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::DeepLink(crate::deep_link::DeepLinkMsg::Copy(t)) if *t == want)));

    // A merge commit has no patch of its own.
    click(&mut h, pos2(700.0, row_y(merge_row)), PointerButton::Secondary);
    assert!(h.get_by_label("Create patch…").is_disabled());
    assert!(h.get_by_label("Copy patch").is_disabled());
    // History edits need a linear run up to HEAD, which is a merge here.
    assert!(h.get_by_label("Squash into parent").is_disabled());
    click(&mut h, pos2(700.0, row_y(root_row)), PointerButton::Secondary);
    assert!(h.get_by_label("Squash into parent").is_disabled());
    assert!(h.get_by_label("Reword commit…").is_disabled());
    drop(h);
    remove_fixture(&dir);
}
