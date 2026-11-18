//! The View menu's Expand all and Collapse all in Changes only: shown with the hunk view's
//! runs, blocked with a reason when nothing is closed or open or whitespace is hidden.

use super::tests::{COLLAPSE_ALL, EXPAND_ALL, harness, in_view_menu, unstaged, view};
use super::*;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::DiffMode;
use glowfly_model::msg::DiffMsg;
use glowfly_model::state::diff_header::{FoldCounts, ViewGroup};

fn with_runs(open: usize, closed: usize, ws_hidden: bool) -> HeaderModel {
    let folds = Some(FoldCounts { open, closed, hunks: true, ws_hidden });
    HeaderModel { view: Some(ViewGroup { mode: DiffMode::Hunks, folds, ignore_ws: ws_hidden, ..view() }), ..unstaged() }
}

/// Whether the menu's `label` item is disabled, with the menu open.
fn disabled(m: HeaderModel, label: &str) -> bool {
    let mut h = harness(m.clone(), 1400.0);
    h.get_by_label(&m.view.unwrap().label(Lang::En)).click();
    h.run();
    h.get_by_label(label).is_disabled()
}

#[test]
fn expand_all_and_collapse_all_work_on_the_hunk_views_runs() {
    let closed = with_runs(0, 2, false);
    assert!(matches!(&in_view_menu(closed.clone(), EXPAND_ALL)[..], [Msg::Diff(DiffMsg::ExpandAllUnchanged)]));
    assert!(disabled(closed.clone(), COLLAPSE_ALL), "nothing is open");
    assert!(!disabled(closed, EXPAND_ALL));

    let open = with_runs(2, 0, false);
    assert!(matches!(&in_view_menu(open.clone(), COLLAPSE_ALL)[..], [Msg::Diff(DiffMsg::CollapseAllUnchanged)]));
    assert!(disabled(open, EXPAND_ALL), "every run is already open");

    let some = with_runs(1, 1, false);
    assert!(!disabled(some.clone(), EXPAND_ALL) && !disabled(some, COLLAPSE_ALL));
}

#[test]
fn both_are_blocked_while_whitespace_is_hidden() {
    let hidden = with_runs(0, 3, true);
    assert!(disabled(hidden.clone(), EXPAND_ALL) && disabled(hidden.clone(), COLLAPSE_ALL));
    assert!(in_view_menu(hidden, EXPAND_ALL).is_empty(), "a blocked item sends nothing");
}

#[test]
fn a_diff_without_runs_shows_neither() {
    let mut h = harness(unstaged(), 1400.0);
    h.get_by_label("Inline \u{b7} Changes only").click();
    h.run();
    assert!(h.query_by_label(EXPAND_ALL).is_none() && h.query_by_label(COLLAPSE_ALL).is_none());
}
