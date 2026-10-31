//! The header cache: its change rows, counts and file order are built once, kept while nothing they
//! come from changes, and built again when the diff, the layout, the folds, the list or the list
//! mode change, always giving the header a fresh build would.

use super::HeaderCache;
use crate::state::AppState;
use crate::state::diff_header::model;
use crate::state::diff_nav::Counter;
use crate::state::op_fixture::{rows, snapshot};
use crate::update::refresh::status_set;
use glowfly_core::repo::{self, DiffLine, DiffTarget, FileDiff, FileEntry, Hunk};
use glowfly_core::settings::{DiffLayout, FileListMode, Settings};
use std::sync::Arc;
use std::time::Instant;

/// Two hunks of one added and one removed line each.
fn diff() -> FileDiff {
    let line = |origin: char, n: u32| DiffLine { origin, old: Some(n), new: Some(n), text: format!("line {n}") };
    let hunk = |n: u32| Hunk { header: format!("@@ -{n} +{n} @@"), lines: vec![line(' ', n), line('-', n + 1), line('+', n + 1)] };
    FileDiff { binary: false, hunks: vec![hunk(1), hunk(40)], highlight: None, intraline: Default::default(), whole: None }
}

/// The WIP lists with b/two.txt open between a.txt and z.txt (List mode), and its diff;
/// `name` keeps each test's repository folder apart.
fn state(name: &str) -> AppState {
    let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
    let mut snap = snapshot(name);
    snap.unstaged = rows(&[("z.txt", 'M'), ("b/two.txt", 'M'), ("a.txt", 'A')]);
    st.snap = Some(snap);
    st.session.diff.view = Some((DiffTarget::Unstaged("b/two.txt".into()), Arc::new(diff())));
    st.view.diff_visible_rows = 4;
    st
}

/// Draws one frame's header and checks it against a fresh build.
fn frame(cache: &mut HeaderCache, st: &AppState) -> (usize, usize) {
    assert_eq!(cache.model(st), model(st), "the cached header is the fresh one");
    cache.builds()
}

#[test]
fn the_header_parts_are_kept_until_what_they_come_from_changes() {
    let mut st = state("header-cache-kept");
    let mut cache = HeaderCache::default();
    for _ in 0..3 {
        assert_eq!(frame(&mut cache, &st), (1, 1), "redrawing the same diff reuses its parts");
    }
    assert_eq!(cache.counts, (2, 2));
    assert_eq!(cache.order.place, Some((1, 3)));

    // Scrolling moves the counter without rebuilding.
    st.session.tab_view.diff_top_row = *cache.rows.last().unwrap();
    assert_eq!(frame(&mut cache, &st), (1, 1));
    assert_eq!(cache.model(&st).unwrap().counter, Some(Counter { index: 2, total: 2, files: false }));

    // A reloaded diff comes with a new generation.
    st.session.diff.generation += 1;
    st.session.diff.shown_gen = st.session.diff.generation;
    st.session.diff.view = Some((DiffTarget::Unstaged("b/two.txt".into()), Arc::new(diff())));
    assert_eq!(frame(&mut cache, &st), (2, 1), "a new diff generation");

    st.settings.diff_layout = DiffLayout::Split;
    assert_eq!(frame(&mut cache, &st), (3, 1), "a new layout");

    st.session.tab_view.diff_expanded.insert(20);
    assert_eq!(frame(&mut cache, &st), (4, 1), "a fold opened");
    st.session.tab_view.diff_expanded.remove(20);
    assert_eq!(frame(&mut cache, &st), (5, 1), "a fold closed");

    // A status read replaces the list: a new list, even with the same files.
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged = rows(&[("z.txt", 'M'), ("b/two.txt", 'D'), ("a.txt", 'A')]);
    status_set(snap, true, None);
    assert_eq!(frame(&mut cache, &st), (5, 2), "a new list");
    assert_eq!(cache.order.status, Some('D'));

    st.settings.file_list_mode = FileListMode::Tree;
    assert_eq!(frame(&mut cache, &st), (5, 3), "a new list mode");
    assert_eq!(cache.order.place, Some((0, 3)), "the folder comes first in the tree");

    // Another file of the list.
    st.session.diff.generation += 1;
    st.session.diff.view = Some((DiffTarget::Unstaged("z.txt".into()), Arc::new(diff())));
    assert_eq!(frame(&mut cache, &st), (6, 4), "another file");
    for _ in 0..3 {
        assert_eq!(frame(&mut cache, &st), (6, 4), "and kept again");
    }

    // Closing the diff drops the parts.
    st.session.diff.view = None;
    assert_eq!(cache.model(&st), None);
    assert!(cache.rows.is_empty() && cache.order.place.is_none());
}

#[test]
fn a_status_read_merged_in_place_is_a_new_list() {
    let mut st = state("header-cache-merged");
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged = rows(&[("a.txt", 'A'), ("b/two.txt", 'M'), ("z.txt", 'M')]);
    status_set(snap, true, None);
    let mut cache = HeaderCache::default();
    assert_eq!(frame(&mut cache, &st), (1, 1));
    assert_eq!(cache.order.place, Some((1, 3)));
    assert_eq!(cache.order.prev, Some(DiffTarget::Unstaged("a.txt".into())));

    // The watcher reports a.txt gone and c.txt new: one row out, one in, in the same buffer.
    let snap = st.snap.as_mut().unwrap();
    let before = (snap.unstaged.as_ptr(), snap.unstaged.len());
    repo::merge_unstaged(&mut snap.unstaged, &["a.txt".into()], vec![FileEntry::new("c.txt", '?')], false);
    assert_eq!((snap.unstaged.as_ptr(), snap.unstaged.len()), before, "the list kept its buffer and length");
    status_set(snap, true, None);
    assert_eq!(frame(&mut cache, &st), (1, 2), "the merged list is a new list");
    assert_eq!(cache.order.place, Some((0, 3)));
    assert_eq!(cache.order.prev, None);
    assert_eq!(cache.order.next, Some(DiffTarget::Unstaged("c.txt".into())));
}
