//! What the header and the palette count of the open diff's unchanged runs: the hunk view's
//! (above the first hunk, between hunks, below the last) with whitespace shown and hidden, and
//! the whole file's folds as before.

use super::counts;
use crate::state::{AppState, diff_header};
use crate::testing::state;
use glowfly_core::repo::{DiffLine, DiffTarget, FileDiff, Hunk};
use glowfly_core::settings::DiffMode;
use std::sync::Arc;

/// A hunk of one changed line at `at`, with a line of context each side.
fn hunk(at: u32) -> Hunk {
    let line = |origin, old, new: Option<u32>| DiffLine { origin, old, new, text: "x".into() };
    let lines = vec![
        line(' ', Some(at), Some(at)),
        line('-', Some(at + 1), None),
        line('+', None, Some(at + 1)),
        line(' ', Some(at + 2), Some(at + 2)),
    ];
    Hunk { header: format!("@@ -{at},3 +{at},3 @@"), lines }
}

fn diff(hunks: Vec<Hunk>, old_lines: Option<u32>) -> FileDiff {
    FileDiff { binary: false, hunks, highlight: None, intraline: Default::default(), whole: None, old_lines }
}

fn open(d: FileDiff) -> AppState {
    let mut st = state();
    st.settings.diff_mode = DiffMode::Hunks;
    st.session.diff.view = Some((DiffTarget::Unstaged("a.txt".into()), Arc::new(d)));
    st
}

#[test]
fn the_hunk_views_runs_are_counted() {
    // Above (lines 1-5), between (9-19) and below (23-30).
    let mut st = open(diff(vec![hunk(6), hunk(20)], Some(30)));
    let c = counts(&st).unwrap();
    assert_eq!((c.open, c.closed, c.hunks, c.ws_hidden), (0, 3, true, false));
    st.session.tab_view.diff_expanded.insert(1);
    st.session.tab_view.diff_expanded.insert(23);
    let c = counts(&st).unwrap();
    assert_eq!((c.open, c.closed), (2, 1), "a run waiting for its text counts as open");
    // The header's View menu has the same counts.
    let m = diff_header::model(&st);
    assert_eq!(m.and_then(|m| m.view).and_then(|v| v.folds), Some(c));

    st.settings.diff_ignore_whitespace = true;
    let c = counts(&st).unwrap();
    assert_eq!((c.open, c.closed, c.hunks, c.ws_hidden), (0, 3, true, true));
}

#[test]
fn a_diff_without_runs_has_no_counts() {
    // One hunk from line 1 to the last line; an added file's single hunk.
    let st = open(diff(vec![hunk(1)], Some(3)));
    assert_eq!(counts(&st), None);
    let st = open(diff(vec![hunk(1)], None));
    assert_eq!(counts(&st), None);
    // The old file's line count unknown: only the run above exists.
    let st = open(diff(vec![hunk(6)], None));
    assert_eq!(counts(&st).map(|c| c.closed), Some(1));
    let binary = FileDiff { binary: true, ..diff(vec![hunk(6)], Some(30)) };
    assert_eq!(counts(&open(binary)), None);
    let none = state();
    assert_eq!(counts(&none), None);
}

#[test]
fn the_whole_files_folds_are_counted_as_before() {
    // Line 12 changed in a 30-line file: a fold above (lines 1-8) and one below (16-30).
    let line = |origin, old: Option<u32>, new: Option<u32>| DiffLine { origin, old, new, text: "x".into() };
    let mut lines: Vec<DiffLine> = (1..=11).map(|n| line(' ', Some(n), Some(n))).collect();
    lines.extend([line('-', Some(12), None), line('+', None, Some(12))]);
    lines.extend((13..=30).map(|n| line(' ', Some(n), Some(n))));
    let hunks = vec![hunk(11)];
    let whole = glowfly_core::whole_file::WholeFile::new(&hunks, Hunk { header: "@@ -1,30 +1,30 @@".into(), lines }).unwrap();
    let mut st = open(FileDiff { whole: Some(Box::new(whole)), ..diff(hunks, Some(30)) });
    st.settings.diff_mode = DiffMode::WholeFile;
    let c = counts(&st).unwrap();
    assert_eq!((c.open, c.closed, c.hunks, c.ws_hidden), (0, 2, false, false));
    st.session.tab_view.diff_expanded.insert(1);
    assert_eq!(counts(&st).map(|c| (c.open, c.closed)), Some((1, 1)));
    // Whitespace hidden changes nothing there.
    st.settings.diff_ignore_whitespace = true;
    assert_eq!(counts(&st).map(|c| (c.open, c.closed, c.ws_hidden)), Some((1, 1, false)));
    // Back in the hunk view the same diff counts its runs.
    st.settings.diff_ignore_whitespace = false;
    st.settings.diff_mode = DiffMode::Hunks;
    assert!(counts(&st).unwrap().hunks);
}
