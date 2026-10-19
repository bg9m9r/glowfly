//! Long ref names: a ref menu names its ref once, in a header, and its items leave the name
//! out; a graph label cut off by the BRANCH / TAG column shows its whole name on hover, and one
//! that fits shows none. Shares the fixture repository with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_select::{click, row_y};
use crate::msg::{BranchMsg, Msg};
use crate::state::AppState;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Event, PointerButton, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use trellis_core::settings::ThemeChoice;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// Where the first chip of a row sits in the BRANCH / TAG column.
const CHIP_X: f32 = 290.0;
/// The fixture's search branch's tip (the WIP row is 0).
const SEARCH_ROW: usize = 1;
const LONG: &str = "todo/git-lfs-live-pattern-refresh-a-missing-g";

/// The fixture with its `search` branch renamed to `LONG`, in the left panel and the graph.
fn long_branch(name: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    let snap = st.snap.as_mut().unwrap();
    snap.locals.iter_mut().find(|b| b.name == "search").unwrap().name = LONG.into();
    for l in snap.refs.values_mut().flatten().filter(|l| l.name == "search") {
        l.name = LONG.into();
        l.full = format!("refs/heads/{LONG}");
    }
    (st, dir)
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
    for item in ["Checkout", "Merge into main", "Rebase main onto this branch", "Push", "Rename…", "Hide in graph", "Solo", "Delete…"] {
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
