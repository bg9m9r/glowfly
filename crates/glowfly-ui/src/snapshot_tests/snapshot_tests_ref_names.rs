//! Long ref names: a ref menu names its ref once, in a header, and its items leave the name
//! out; a graph label cut off by the BRANCH / TAG column shows its whole name on hover, and one
//! that fits shows none; chips whose names share a long start are cut in their middle and stay
//! apart. Shares the fixture repository with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_select::{click, row_y};
use crate::snapshot_check::CheckSnapshot;
use eframe::egui::{Event, PointerButton, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use glowfly_model::msg::{BranchMsg, Msg};
use glowfly_model::state::AppState;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// Where the first chip of a row sits in the BRANCH / TAG column.
const CHIP_X: f32 = 290.0;
/// The fixture's search branch's tip (the WIP row is 0).
const SEARCH_ROW: usize = 1;
const LONG: &str = "todo/git-lfs-live-pattern-refresh-a-missing-g";

/// The fixture with its `search` branch renamed to `LONG`, in the left panel and the graph.
fn long_branch(name: &str) -> (AppState, std::path::PathBuf) {
    renamed(ThemeChoice::Dark, name, &[("search", LONG)])
}

/// The fixture in `theme` with local branches renamed (`(old, new)`), in the left panel and the
/// graph.
fn renamed(theme: ThemeChoice, name: &str, names: &[(&str, &str)]) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let snap = st.snap.as_mut().unwrap();
    for &(old, new) in names {
        snap.locals.iter_mut().find(|b| b.name == old).unwrap().name = new.into();
        for l in snap.refs.values_mut().flatten().filter(|l| l.name == old) {
            l.name = new.into();
            l.full = format!("refs/heads/{new}");
        }
    }
    (st, dir)
}

/// Two branches whose names share a long start, as agents' worktrees name them.
const SHARED: [(&str, &str); 2] =
    [("login-page", "worktree-agent-a1b2c3d4-login-page"), ("search", "worktree-agent-a1b2c3d4-search-index")];

/// The graph with `SHARED`'s chips, cut in their middle by the BRANCH / TAG column: each keeps
/// its start and its own end.
fn chips_shared_prefix(theme: ThemeChoice, dark: bool, name: &str) {
    let (st, dir) = renamed(theme, name, &SHARED);
    let mut h = harness(st, dark);
    h.run();
    // The text snapshot lists each chip's cut name as painted.
    h.check_snapshot(name);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_chips_shared_prefix_dark() {
    chips_shared_prefix(ThemeChoice::Dark, true, "graph_chips_shared_prefix_dark");
}

#[test]
fn graph_chips_shared_prefix_light() {
    chips_shared_prefix(ThemeChoice::Light, false, "graph_chips_shared_prefix_light");
}

/// Rests the pointer on the first chip of the search branch's row.
fn hover_chip(h: &mut H) {
    h.input_mut().events.push(Event::PointerMoved(pos2(CHIP_X, row_y(SEARCH_ROW))));
    for _ in 0..4 {
        h.step();
    }
}

fn count(h: &H, label: &str) -> usize {
    h.get_all_by_label(label).count()
}

#[test]
fn ref_menu_long_name() {
    let (st, dir) = long_branch("ref-menu-long-name");
    let mut h = harness(st, true);
    h.run();
    click(&mut h, pos2(CHIP_X, row_y(SEARCH_ROW)), PointerButton::Secondary);
    h.check_snapshot("ref_menu_long_name");
    for item in
        ["Checkout", "Merge into main", "Rebase main onto this branch", "Push", "Rename…", "Hide in graph", "Show only this", "Delete…"]
    {
        assert!(h.query_by_label(item).is_some(), "{item}");
    }
    assert!(h.query_by_label_contains(&format!("Checkout {LONG}")).is_none(), "the items leave the name out");
    h.state_mut().1.clear();
    h.get_by_label("Checkout").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Branch(BranchMsg::Checkout(b)) if b == LONG)), "the branch is checked out");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_label_cut_off_tooltip() {
    let (st, dir) = long_branch("label-cut-off-tooltip");
    let mut h = harness(st, true);
    h.run();
    let before = count(&h, LONG);
    hover_chip(&mut h);
    h.check_snapshot("graph_label_cut_off_tooltip");
    assert_eq!(count(&h, LONG), before + 1, "the whole name in a tooltip");
    drop(h);

    // A name that fits gets no tooltip.
    let (st, _) = with_repo(ThemeChoice::Dark, "label-cut-off-tooltip");
    let mut h = harness(st, true);
    h.run();
    let before = count(&h, "search");
    hover_chip(&mut h);
    assert_eq!(count(&h, "search"), before, "no tooltip");
    drop(h);
    remove_fixture(&dir);
}
