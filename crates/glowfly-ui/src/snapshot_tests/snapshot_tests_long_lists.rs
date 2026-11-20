//! The TAGS list and a remote's branch list draw only the rows in view, and still show every
//! row once scrolled to it and exactly the filter's matches: 400 tags and 400 remote branches in
//! a window that shows a few dozen rows.

use super::snapshot_tests::{harness_sized, remove_fixture, with_repo};
use super::snapshot_tests_worktrees::H;
use crate::panel_size;
use eframe::egui::containers::panel::PanelState;
use eframe::egui::{self, Event, Modifiers, MouseWheelUnit, Rect, pos2, vec2};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use glowfly_model::state::AppState;
use std::path::PathBuf;

const COUNT: usize = 400;

fn name(prefix: &str, i: usize) -> String {
    format!("{prefix}-{i:03}")
}

/// The fixture with `COUNT` tags `tag-000`... and the TAGS section open.
fn with_tags(dir_name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, dir_name);
    let snap = st.snap.as_mut().unwrap();
    let head = snap.commits[0].oid;
    snap.tags = (0..COUNT).map(|i| (name("tag", i), head)).collect();
    st.view.sections.tags = true;
    (st, dir)
}

/// The fixture with `COUNT` branches `b-000`... on the remote `origin`, whose section is open.
fn with_remote_branches(dir_name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, dir_name);
    let snap = st.snap.as_mut().unwrap();
    let head = snap.commits[0].oid;
    snap.remotes = vec![("origin".to_owned(), (0..COUNT).map(|i| (name("b", i), head)).collect())];
    st.view.sections.remote = true;
    (st, dir)
}

fn sidebar(h: &H) -> Rect {
    PanelState::load(&h.ctx, egui::Id::new(panel_size::SIDEBAR_ID)).expect("the sidebar").rect
}

/// The numbers of the rows `prefix-NNN` the sidebar shows: those whose middle is inside it (the
/// graph's chips carry names too).
fn shown(h: &H, prefix: &str) -> Vec<usize> {
    let panel = sidebar(h);
    (0..COUNT)
        .filter(|&i| {
            h.query_all_by_label(name(prefix, i).as_str())
                .filter_map(|n| n.bounding_box())
                .any(|r| panel.contains(pos2(((r.x0 + r.x1) / 2.0) as f32, ((r.y0 + r.y1) / 2.0) as f32)))
        })
        .collect()
}

fn scroll_to_end(h: &mut H) {
    let at = sidebar(h).center();
    h.input_mut().events.push(Event::PointerMoved(at));
    h.input_mut().events.push(Event::MouseWheel { unit: MouseWheelUnit::Point, delta: vec2(0.0, -20000.0), modifiers: Modifiers::NONE });
    h.run_steps(6);
}

/// Shows the rows `prefix-NNN` at the start, scrolls to the end, then filters to `-25`.
fn check(mut h: H, prefix: &str) {
    h.run();
    let first = shown(&h, prefix);
    assert!(!first.is_empty(), "the rows at the start show");
    assert!(!first.contains(&(COUNT - 1)), "the last row is below the view");
    scroll_to_end(&mut h);
    let seen = shown(&h, prefix);
    assert!(seen.contains(&(COUNT - 1)), "the last row shows once scrolled to");
    assert!(!seen.contains(&0), "the first row is above the view");
    assert!(seen.windows(2).all(|w| w[1] == w[0] + 1), "the rows shown are one run: {seen:?}");
    h.state_mut().0.view.tab.ref_filter = "-25".into();
    h.run();
    assert_eq!(shown(&h, prefix), (250..260).collect::<Vec<_>>(), "exactly the matches show");
}

#[test]
fn a_long_tag_list_shows_the_rows_in_view_after_scrolling() {
    let (st, dir) = with_tags("long-tags");
    check(harness_sized(st, true, vec2(1200.0, 800.0)), "tag");
    remove_fixture(&dir);
}

#[test]
fn a_long_remote_branch_list_shows_the_rows_in_view_after_scrolling() {
    let (st, dir) = with_remote_branches("long-remote");
    check(harness_sized(st, true, vec2(1200.0, 800.0)), "b");
    remove_fixture(&dir);
}
