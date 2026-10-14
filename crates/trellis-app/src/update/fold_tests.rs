//! Headless tests for whole-file folds: opening one in place, ignoring stale clicks, when the
//! opened set resets, stepping between changes with folds closed, and parking with a tab.

use super::tests::{drive, scratch_repo, state};
use super::update;
use super::whole_file_tests::{blocks, three_changes, with_diff};
use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::AppState;
use std::time::Instant;
use trellis_core::repo::{DiffLine, DiffTarget, FileDiff, Hunk};
use trellis_core::settings::{DiffLayout, DiffMode};
use trellis_core::whole_file::{Expanded, Fold, WholeFile, block_rows, folds};

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn whole(st: &AppState) -> &WholeFile {
    st.diff.as_ref().and_then(|(_, d)| d.whole.as_deref()).expect("whole file on screen")
}

fn first_fold(st: &AppState) -> Fold {
    folds(whole(st))[0]
}

fn f_txt() -> DiffTarget {
    DiffTarget::Unstaged("f.txt".into())
}

/// Lines 1..=30 with lines 15 and 28 replaced, as two hunks and the whole file: it starts with
/// a fold, so its first block's row depends on which folds are open.
fn late_changes() -> FileDiff {
    let ctx = |n: u32| DiffLine { origin: ' ', old: Some(n), new: Some(n), text: format!("line {n}") };
    let del = |n: u32| DiffLine { origin: '-', old: Some(n), new: None, text: format!("line {n}") };
    let add = |n: u32| DiffLine { origin: '+', old: None, new: Some(n), text: format!("changed {n}") };
    let lines = |from: u32, to: u32| -> Vec<DiffLine> {
        (from..=to).flat_map(|n| if [15, 28].contains(&n) { vec![del(n), add(n)] } else { vec![ctx(n)] }).collect()
    };
    let hunk = |from: u32, to: u32| Hunk { header: format!("@@ -{from} +{from} @@"), lines: lines(from, to) };
    let mut d =
        FileDiff { binary: false, hunks: vec![hunk(12, 18), hunk(25, 30)], highlight: None, intraline: Default::default(), whole: None };
    d.whole = WholeFile::new(&d.hunks, hunk(1, 30)).map(Box::new);
    d
}

/// Delivers `d` for `target` as the result of the request in flight.
fn arrive(st: &mut AppState, target: DiffTarget, reload: bool, result: Result<FileDiff, String>) {
    let generation = st.diff_gen;
    run(st, Msg::DiffLoaded { target, reload, generation, result });
}

#[test]
fn expand_unchanged_opens_a_fold_in_place() {
    for layout in [DiffLayout::Inline, DiffLayout::Split] {
        let (mut st, dir) = with_diff(&format!("fold-open-{layout:?}"), three_changes(true));
        st.settings.diff_mode = DiffMode::WholeFile;
        st.settings.diff_layout = layout;
        let f = first_fold(&st);
        assert_eq!((f.start, f.end), (6, 12));
        assert_eq!(folds(whole(&st))[1].start, 20);
        let old = blocks(&three_changes(true), layout);
        st.view.diff_top_row = 4;
        st.view.diff_cursor = Some(old[2]);

        let fx = run(&mut st, Msg::ExpandUnchanged(f.key));
        assert!(fx.is_empty(), "{fx:?}");
        assert!(st.view.diff_expanded.contains(f.key));
        assert_eq!((st.view.diff_top_row, st.view.diff_scroll_to_row), (4, None), "{layout:?}: nothing scrolls");
        let new = block_rows(whole(&st), layout, &st.view.diff_expanded);
        let shift = f.len() - 1;
        assert_eq!(new, [old[0], old[1] + shift, old[2] + shift], "{layout:?}");
        assert_eq!(st.view.diff_cursor, Some(new[2]), "{layout:?}: the stepped-to change keeps its place");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn expand_unchanged_ignores_stale_clicks() {
    let (mut st, dir) = with_diff("fold-stale", three_changes(true));
    st.settings.diff_mode = DiffMode::WholeFile;
    let key = first_fold(&st).key;

    assert!(run(&mut st, Msg::ExpandUnchanged(999)).is_empty());
    assert!(st.view.diff_expanded.is_empty(), "not a fold of this file");
    assert!(run(&mut st, Msg::ExpandUnchanged(7)).is_empty());
    assert!(st.view.diff_expanded.is_empty(), "a line inside a fold, not its key");

    run(&mut st, Msg::ExpandUnchanged(key));
    let once = st.view.diff_expanded.clone();
    assert!(run(&mut st, Msg::ExpandUnchanged(key)).is_empty());
    assert_eq!(st.view.diff_expanded, once, "already open");

    st.view.diff_expanded.clear();
    st.settings.diff_mode = DiffMode::Hunks;
    assert!(run(&mut st, Msg::ExpandUnchanged(key)).is_empty());
    assert!(st.view.diff_expanded.is_empty(), "the hunk view has no folds");

    st.settings.diff_mode = DiffMode::WholeFile;
    st.diff = None;
    assert!(run(&mut st, Msg::ExpandUnchanged(key)).is_empty());
    assert!(st.view.diff_expanded.is_empty(), "no diff open");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn folds_reset_only_for_another_file() {
    let (mut st, dir) = with_diff("fold-reset", three_changes(true));
    st.settings.diff_mode = DiffMode::WholeFile;
    let key = first_fold(&st).key;
    let open = |st: &mut AppState| {
        run(st, Msg::ExpandUnchanged(key));
        assert!(st.view.diff_expanded.contains(key));
    };
    open(&mut st);

    arrive(&mut st, f_txt(), true, Ok(three_changes(true)));
    assert!(st.view.diff_expanded.contains(key), "a watcher or hunk reload of the same file");
    arrive(&mut st, f_txt(), false, Ok(three_changes(true)));
    assert!(st.view.diff_expanded.contains(key), "a reopen of the same file (mode or whitespace switch)");

    let g = DiffTarget::Unstaged("g.txt".into());
    run(&mut st, Msg::OpenDiff(g.clone()));
    assert!(st.view.diff_expanded.contains(key), "f.txt is still on screen");
    arrive(&mut st, g.clone(), false, Ok(three_changes(true)));
    assert!(st.view.diff_expanded.is_empty(), "another file arrived");

    open(&mut st);
    run(&mut st, Msg::CloseDiff);
    assert!(st.view.diff_expanded.is_empty(), "the diff closed");

    st.diff = Some((g.clone(), three_changes(true)));
    open(&mut st);
    arrive(&mut st, g, true, Err("no changes".into()));
    assert!(st.diff.is_none());
    assert!(st.view.diff_expanded.is_empty(), "a failed reload closes the diff");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The rows a run of `dir` steps from the top (1) or the bottom (-1) visits, in order.
fn visit(st: &mut AppState, dir: isize) -> Vec<usize> {
    st.view.diff_cursor = None;
    st.view.diff_top_row = if dir > 0 { 0 } else { 10_000 };
    let mut seen = Vec::new();
    for _ in 0..10 {
        run(st, Msg::JumpToChange(dir));
        match st.view.diff_cursor {
            Some(c) if seen.last() != Some(&c) => seen.push(c),
            _ => break,
        }
    }
    seen
}

#[test]
fn stepping_counts_folds() {
    for layout in [DiffLayout::Inline, DiffLayout::Split] {
        let (mut st, dir) = with_diff(&format!("fold-step-{layout:?}"), three_changes(true));
        st.settings.diff_mode = DiffMode::WholeFile;
        st.settings.diff_layout = layout;
        st.view.diff_visible_rows = 20;
        for expand in [false, true] {
            if expand {
                let key = first_fold(&st).key;
                run(&mut st, Msg::ExpandUnchanged(key));
            }
            let want = block_rows(whole(&st), layout, &st.view.diff_expanded);
            assert_eq!(visit(&mut st, 1), want, "{layout:?} expanded={expand}");
            let mut back = want.clone();
            back.reverse();
            assert_eq!(visit(&mut st, -1), back, "{layout:?} expanded={expand}");
        }

        // A file that starts with a fold opens on its first block, counted with it closed.
        st.diff = Some((f_txt(), late_changes()));
        let key = first_fold(&st).key;
        run(&mut st, Msg::ExpandUnchanged(key));
        assert!(!st.view.diff_expanded.is_empty());
        let g = DiffTarget::Unstaged("g.txt".into());
        run(&mut st, Msg::OpenDiff(g.clone()));
        arrive(&mut st, g, false, Ok(late_changes()));
        let row = blocks(&late_changes(), layout)[0];
        assert_eq!(st.view.diff_scroll_to_row, Some(row), "{layout:?}");
        assert_ne!(row, block_rows(whole(&st), layout, &Expanded::all(whole(&st)))[0], "the leading fold counts");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn folds_are_parked_with_their_tab() {
    let dir = scratch_repo("fold-tabs");
    let mut st = state();
    drive(&mut st, Msg::Open(dir.clone()));
    st.settings.diff_mode = DiffMode::WholeFile;
    st.settings.diff_layout = DiffLayout::Inline;
    st.diff = Some((DiffTarget::Unstaged("a.txt".into()), three_changes(true)));
    let key = first_fold(&st).key;
    run(&mut st, Msg::ExpandUnchanged(key));
    st.view.diff_top_row = block_rows(whole(&st), DiffLayout::Inline, &st.view.diff_expanded)[2];

    run(&mut st, Msg::NewTab);
    assert_eq!(st.active_tab, 1);
    assert!(st.view.diff_expanded.is_empty(), "a new tab has no folds open");
    // The layout changes while the first tab is parked.
    run(&mut st, Msg::SetDiffLayout(DiffLayout::Split));

    run(&mut st, Msg::SelectTab(0));
    assert!(st.view.diff_expanded.contains(key), "restored with the tab");
    let split = block_rows(whole(&st), DiffLayout::Split, &st.view.diff_expanded);
    assert_eq!(st.view.diff_top_row, split[2], "the same block, counted with the fold open");
    assert_ne!(split[2], blocks(&three_changes(true), DiffLayout::Split)[2]);
    let _ = std::fs::remove_dir_all(&dir);
}
