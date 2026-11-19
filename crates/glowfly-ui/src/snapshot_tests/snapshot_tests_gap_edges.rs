//! The hunk view's runs above the first hunk and below the last: the controls while closed (a
//! header control and a row after the last line), every run filled in (inline and split), and the
//! spinner while their text is read. Text snapshots only. Shares the fixture repository and
//! renderer with `snapshot_tests`.

use super::super::snapshot_tests::{git, harness, with_repo};
use crate::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::diff_rows::gaps;
use glowfly_core::repo::{self, DiffTarget, LoadDiffOptions, LoadOptions};
use glowfly_core::settings::{DiffLayout, ThemeChoice};
use glowfly_core::{highlight, intraline};
use glowfly_model::msg::{DiffMsg, Msg};
use glowfly_model::state::AppState;
use std::path::PathBuf;
use std::sync::Arc;

/// `fn fN() {}` for lines 1..=30, with lines 8 and 22 given a body.
fn body(changed: bool) -> String {
    (1..=30).map(|i| if changed && [8, 22].contains(&i) { format!("fn f{i}() {{ {i} }}\n") } else { format!("fn f{i}() {{}}\n") }).collect()
}

/// What state `edges` leaves the runs in.
#[derive(Clone, Copy, PartialEq)]
enum Runs {
    Closed,
    Filled,
    /// Every run opened, its text not yet read.
    Reading,
}

/// edges.rs with two changed lines (runs of 4 lines above, 7 between and 5 below), loaded for
/// the viewer and highlighted as Rust.
fn edges(layout: DiffLayout, runs: Runs) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, &format!("diff-edges-{layout:?}-{}", runs as u8));
    std::fs::write(dir.join("edges.rs"), body(false)).unwrap();
    let d = "2026-01-11T10:00:00+00:00";
    git(&dir, &["add", "edges.rs"], d);
    git(&dir, &["commit", "-q", "-m", "Add edges.rs"], d);
    std::fs::write(dir.join("edges.rs"), body(true)).unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let target = DiffTarget::Unstaged("edges.rs".into());
    let mut diff = repo::load_diff_for_view(&dir, &target, LoadDiffOptions::default()).unwrap().diff;
    diff.highlight = highlight::highlight_diff(target.path(), &diff, None);
    diff.intraline = intraline::intraline(&diff);
    let spans = gaps(&diff);
    assert_eq!(spans.iter().map(|g| (g.hunk, g.len, g.key)).collect::<Vec<_>>(), [(0, 4, 1), (1, 7, 12), (2, 5, 26)]);
    if runs != Runs::Closed {
        for g in &spans {
            st.session.tab_view.diff_expanded.insert(g.key);
        }
    }
    if runs == Runs::Filled {
        let syntax = diff.highlight.as_ref().map(|h| h.syntax.as_str());
        let text = repo::load_gap_text(&dir, &target, &diff, syntax).unwrap();
        st.session.tab_view.diff_expanded.set_gap_text(Arc::new(text));
    }
    st.session.diff.view = Some((target, diff.into()));
    if runs == Runs::Reading {
        st.session.tab_view.diff_gaps_reading = Some(st.session.diff.shown_gen);
    }
    st.settings.diff_layout = layout;
    (st, dir)
}

fn cleanup(dir: PathBuf) {
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn diff_gap_edges_closed_inline() {
    let (st, dir) = edges(DiffLayout::Inline, Runs::Closed);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("diff_gap_edges_closed_inline");
    let above = h.get_by_label("Show the 4 unchanged lines above").bounding_box().unwrap();
    let between = h.get_by_label("Show the 7 unchanged lines above").bounding_box().unwrap();
    let below = h.get_by_label("Show the 5 unchanged lines below").bounding_box().unwrap();
    let hunks: Vec<_> = h.query_all_by_label("Discard hunk").filter_map(|n| n.bounding_box()).collect();
    assert_eq!(hunks.len(), 2);
    // The controls of the first and second header, and the row after the last line.
    assert!(above.y0 < hunks[0].y1 + 1.0 && above.y1 > hunks[0].y0 - 1.0, "on the first header: {above:?} {:?}", hunks[0]);
    assert!(between.y0 < hunks[1].y1 + 1.0 && between.y1 > hunks[1].y0 - 1.0, "on the second header");
    assert!(below.y0 > hunks[1].y1, "the row after the last hunk's last line: {below:?}");
    assert!(below.width() > above.width() * 4.0, "a full-width row: {below:?}");
    h.get_by_label("Show the 5 unchanged lines below").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Diff(DiffMsg::ExpandUnchanged(26)))), "{:?}", h.state().1.len());
    h.state_mut().1.clear();
    h.get_by_label("Show the 4 unchanged lines above").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Diff(DiffMsg::ExpandUnchanged(1)))));
    drop(h);
    cleanup(dir);
}

#[test]
fn diff_gap_edges_open_inline() {
    let (st, dir) = edges(DiffLayout::Inline, Runs::Filled);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("diff_gap_edges_open_inline");
    assert!(h.query_by_label_contains("unchanged lines below").is_none() && h.query_by_label_contains("lines above").is_none());
    for label in ["Fold 4 unchanged lines", "Fold 7 unchanged lines", "Fold 5 unchanged lines"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    h.get_by_label("Fold 5 unchanged lines").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Diff(DiffMsg::CollapseUnchanged(26)))));
    drop(h);
    cleanup(dir);
}

#[test]
fn diff_gap_edges_open_split() {
    let (st, dir) = edges(DiffLayout::Split, Runs::Filled);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("diff_gap_edges_open_split");
    for label in ["Fold 4 unchanged lines", "Fold 7 unchanged lines", "Fold 5 unchanged lines"] {
        assert_eq!(h.query_all_by_label(label).count(), 1, "{label}");
    }
    assert!(h.query_by_label_contains("unchanged lines below").is_none());
    drop(h);
    cleanup(dir);
}

#[test]
fn diff_gap_edges_loading_inline() {
    let (st, dir) = edges(DiffLayout::Inline, Runs::Reading);
    let mut h = harness(st, true);
    // The spinners keep repainting; a fixed number of frames keeps the output reproducible.
    h.run_steps(4);
    h.check_snapshot("diff_gap_edges_loading_inline");
    let waiting: Vec<_> = h.query_all_by_label("Reading the unchanged lines…").collect();
    assert_eq!(waiting.len(), 3, "the controls of the two headers and the row below");
    assert!(waiting.iter().all(|n| n.role() == eframe::egui::accesskit::Role::ProgressIndicator), "none is a button");
    assert!(h.query_by_label_contains("Show the 4").is_none() && h.query_by_label_contains("unchanged lines below").is_none());
    h.state_mut().1.clear();
    for n in h.query_all_by_label("Reading the unchanged lines…") {
        n.click();
    }
    h.run_steps(2);
    assert!(h.state().1.is_empty(), "a waiting control isn't clickable: {}", h.state().1.len());
    drop(h);
    cleanup(dir);
}
