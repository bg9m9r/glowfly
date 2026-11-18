//! The hunk view's unchanged runs at the edges (above the first hunk, below the last) and all at
//! once: opening and closing them, keeping the row at the top in place, Expand all and Collapse
//! all across a reload, the places they stay off, and the flag that says their text is being read.
//! The diff is built by hand and the text lands by synthetic `GapsLoaded`.

use super::tests::{scratch_repo, state};
use super::update;
use crate::effect::{DiffEffect, Effect};
use crate::msg::{DiffMsg, Msg};
use crate::state::diff_runs;
use crate::state::{AppState, Tab};
use glowfly_core::diff_rows::{DiffRow, GapLines, GapText, SplitRow, gaps, inline_rows_filled, split_rows_filled};
use glowfly_core::error::ReadFailed;
use glowfly_core::repo::{self, DiffLine, DiffTarget, FileDiff, Hunk, LoadOptions};
use glowfly_core::settings::{DiffLayout, DiffMode};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

/// A hunk of three lines at old and new line `at` (` `, `-`/`+` pair, ` `): lines `at..at + 3`.
fn hunk(at: u32) -> Hunk {
    let line = |origin, old, new: Option<u32>, text: &str| DiffLine { origin, old, new, text: text.into() };
    let lines = vec![
        line(' ', Some(at), Some(at), "before"),
        line('-', Some(at + 1), None, "old"),
        line('+', None, Some(at + 1), "new"),
        line(' ', Some(at + 2), Some(at + 2), "after"),
    ];
    Hunk { header: format!("@@ -{at},3 +{at},3 @@"), lines }
}

/// A 30-line file with hunks at lines 6-8 and 20-22: runs of 5 lines above (key 1), 11 between
/// (key 9) and 8 below (key 23).
fn diff() -> FileDiff {
    FileDiff {
        binary: false,
        hunks: vec![hunk(6), hunk(20)],
        highlight: None,
        intraline: Default::default(),
        whole: None,
        old_lines: Some(30),
    }
}

/// The same file after an edit above: its first hunk is gone, so the run above is lines 1-19.
fn later() -> FileDiff {
    FileDiff { hunks: vec![hunk(20)], ..diff() }
}

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// A state in the hunk view with `d` open on a repository (so reads can be asked for).
fn shown(layout: DiffLayout, d: FileDiff) -> (AppState, DiffTarget) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = scratch_repo(&format!("diff-gaps-model-{}", NEXT.fetch_add(1, Ordering::Relaxed)));
    let mut st = state();
    st.tabs[0] = Tab::unloaded(dir.clone());
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    st.settings.diff_mode = DiffMode::Hunks;
    st.settings.diff_layout = layout;
    let target = DiffTarget::Unstaged("a.txt".into());
    st.session.diff.view = Some((target.clone(), Arc::new(d)));
    let _ = std::fs::remove_dir_all(dir);
    (st, target)
}

/// How many `DiffEffect::Gaps` reads `fx` holds.
fn reads(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::Diff(DiffEffect::Gaps { .. }))).count()
}

/// The text of every run of the diff on screen, as a read would bring it.
fn text_of(st: &AppState) -> Arc<GapText> {
    let (_, d) = st.session.diff.view.as_ref().unwrap();
    let lines = gaps(d)
        .into_iter()
        .map(|g| GapLines {
            span: g,
            lines: (0..g.len).map(|i| format!("run {} line {i}", g.key)).collect(),
            spans: vec![Vec::new(); g.len],
        })
        .collect();
    Arc::new(GapText::new(lines))
}

fn land(st: &mut AppState, target: &DiffTarget) {
    let (generation, text) = (st.session.diff.shown_gen, text_of(st));
    run(st, Msg::Diff(DiffMsg::GapsLoaded { target: target.clone(), generation, result: Ok(text) }));
}

fn inline(st: &AppState) -> Vec<DiffRow> {
    inline_rows_filled(&st.session.diff.view.as_ref().unwrap().1, &st.session.tab_view.diff_expanded)
}

fn split(st: &AppState) -> Vec<SplitRow> {
    split_rows_filled(&st.session.diff.view.as_ref().unwrap().1, &st.session.tab_view.diff_expanded)
}

/// What a diff being shown (opened or reloaded) asks for.
fn reload(st: &mut AppState) -> Vec<Effect> {
    let mut fx = Vec::new();
    crate::update::diff_gaps::diff_shown(st, &mut fx);
    fx
}

fn closed_rows(d: &FileDiff) -> usize {
    inline_rows_filled(d, &Default::default()).len()
}

#[test]
fn edge_runs_open_and_close() {
    let (mut st, target) = shown(DiffLayout::Inline, diff());
    let d = st.session.diff.view.as_ref().unwrap().1.clone();
    let rows = closed_rows(&d);
    assert!(matches!(inline(&st).last(), Some(DiffRow::GapBelow(g)) if g.key == 23));
    let fx = run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(1)));
    assert_eq!(reads(&fx), 1);
    assert!(diff_runs::reading(&st));
    assert_eq!(inline(&st).len(), rows, "closed until the text lands");
    land(&mut st, &target);
    assert!(!diff_runs::reading(&st));
    let after = inline(&st);
    assert_eq!(after[..5], [0, 1, 2, 3, 4].map(|line| DiffRow::GapLine { hunk: 0, line }));
    assert_eq!(after[5], DiffRow::Header(0));

    // The text is in: the run below opens at once, with no read, and loses its row.
    let fx = run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(23)));
    assert_eq!(reads(&fx), 0);
    let after = inline(&st);
    assert_eq!(after.len(), rows - 1 + 5 + 8);
    assert!(!after.iter().any(|r| matches!(r, DiffRow::GapBelow(_))));
    assert_eq!(after[after.len() - 8..], [0, 1, 2, 3, 4, 5, 6, 7].map(|line| DiffRow::GapLine { hunk: 2, line }));

    // Closing with the top row inside the run below puts the top on its control row.
    let n = after.len();
    st.view.tab.diff_top_row = n - 3;
    run(&mut st, Msg::Diff(DiffMsg::CollapseUnchanged(23)));
    let back = inline(&st);
    assert!(matches!(back.last(), Some(DiffRow::GapBelow(_))));
    assert_eq!(st.view.tab.diff_top_row, back.len() - 1);
    assert_eq!(st.view.diff_scroll_to_row, Some(back.len() - 1));
}

#[test]
fn expand_all_and_collapse_all_across_a_reload() {
    let (mut st, target) = shown(DiffLayout::Inline, diff());
    let fx = run(&mut st, Msg::Diff(DiffMsg::ExpandAllUnchanged));
    assert_eq!(reads(&fx), 1);
    assert_eq!(st.session.tab_view.diff_expanded.keys().collect::<Vec<_>>(), [1, 9, 23]);
    assert_eq!(inline(&st).len(), closed_rows(&st.session.diff.view.as_ref().unwrap().1), "waiting for the text");
    land(&mut st, &target);
    let counts = diff_runs::counts(&st).expect("runs");
    assert_eq!((counts.open, counts.closed, counts.hunks), (3, 0, true));
    let rows = inline(&st);
    assert!(!rows.iter().any(|r| matches!(r, DiffRow::GapBelow(_))));
    // Again: nothing changes, nothing is read.
    let fx = run(&mut st, Msg::Diff(DiffMsg::ExpandAllUnchanged));
    assert_eq!((reads(&fx), inline(&st)), (0, rows.clone()));

    // A reload with the same hunks keeps keys, text and the filled rows.
    st.session.diff.shown_gen += 1;
    let fx = reload(&mut st);
    assert_eq!(reads(&fx), 1, "the text is read again");
    assert_eq!(inline(&st), rows, "the old text fills meanwhile");
    land(&mut st, &target);
    assert_eq!(inline(&st), rows);

    // A reload whose first hunk is gone keeps key 1 open: closed until the next text, then filled.
    st.session.diff.view = Some((target.clone(), Arc::new(later())));
    st.session.diff.shown_gen += 1;
    let fx = reload(&mut st);
    assert_eq!(reads(&fx), 1);
    assert!(st.session.tab_view.diff_expanded.contains(1));
    let waiting = inline(&st);
    assert!(!waiting.contains(&DiffRow::GapLine { hunk: 0, line: 0 }), "the old text is of another run");
    land(&mut st, &target);
    let now = inline(&st);
    assert_eq!(now[..19], (0..19).map(|line| DiffRow::GapLine { hunk: 0, line }).collect::<Vec<_>>()[..]);
    assert!(st.session.tab_view.diff_expanded.contains(1));

    // Collapse all closes every run, keeps the text, and a reload after it starts no read.
    run(&mut st, Msg::Diff(DiffMsg::CollapseAllUnchanged));
    assert!(st.session.tab_view.diff_expanded.keys().next().is_none());
    assert!(st.session.tab_view.diff_expanded.gap_text().is_some());
    let closed = inline(&st);
    assert!(matches!(closed.last(), Some(DiffRow::GapBelow(_))));
    st.session.diff.shown_gen += 1;
    let fx = reload(&mut st);
    assert_eq!(reads(&fx), 0);
    assert_eq!(inline(&st), closed);
}

#[test]
fn the_run_takes_its_headers_place_at_the_top() {
    for layout in [DiffLayout::Inline, DiffLayout::Split] {
        let (mut st, target) = shown(layout, diff());
        let header = |st: &AppState, h: usize| match layout {
            DiffLayout::Inline => inline(st).iter().position(|r| *r == DiffRow::Header(h)).unwrap(),
            DiffLayout::Split => split(st).iter().position(|r| *r == SplitRow::Header(h)).unwrap(),
        };
        let first_line = |st: &AppState, h: usize| match layout {
            DiffLayout::Inline => inline(st).iter().position(|r| *r == DiffRow::GapLine { hunk: h, line: 0 }),
            DiffLayout::Split => split(st).iter().position(|r| *r == SplitRow::GapLine { hunk: h, line: 0 }),
        };
        // The top on the first header, whose run opens: the run's first line takes row 0.
        st.view.tab.diff_top_row = 0;
        run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(1)));
        land(&mut st, &target);
        assert_eq!(st.view.tab.diff_top_row, 0, "{layout:?}");
        assert_eq!(st.view.diff_scroll_to_row, None, "{layout:?}: nothing scrolls");
        assert_eq!(first_line(&st, 0), Some(0));
        run(&mut st, Msg::Diff(DiffMsg::CollapseAllUnchanged));

        // The top on the second header with Expand all: it becomes the first line of its run.
        let at = header(&st, 1);
        st.view.tab.diff_top_row = at;
        run(&mut st, Msg::Diff(DiffMsg::ExpandAllUnchanged));
        let top = st.view.tab.diff_top_row;
        assert_eq!(Some(top), first_line(&st, 1), "{layout:?}");
        assert_eq!(st.view.diff_scroll_to_row, Some(top));

        // A run opening above the top row keeps that row where it was.
        run(&mut st, Msg::Diff(DiffMsg::CollapseAllUnchanged));
        st.view.diff_scroll_to_row = None;
        let at = header(&st, 1);
        st.view.tab.diff_top_row = at + 1;
        run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(1)));
        let row = match layout {
            DiffLayout::Inline => inline(&st).iter().position(|r| *r == DiffRow::Line { hunk: 1, line: 0 }),
            DiffLayout::Split => split(&st).iter().position(|r| matches!(r, SplitRow::Pair { hunk: 1, old: Some(0), .. })),
        };
        assert_eq!(Some(st.view.tab.diff_top_row), row, "{layout:?}: still on the hunk's first line");
    }
}

#[test]
fn runs_stay_off() {
    let (mut st, target) = shown(DiffLayout::Inline, diff());
    st.settings.diff_ignore_whitespace = true;
    let before = inline(&st);
    for msg in [DiffMsg::ExpandAllUnchanged, DiffMsg::CollapseAllUnchanged, DiffMsg::ExpandUnchanged(1)] {
        assert_eq!(reads(&run(&mut st, Msg::Diff(msg))), 0);
    }
    assert!(st.session.tab_view.diff_expanded.keys().next().is_none());
    assert_eq!(inline(&st), before);
    // A text that lands while whitespace is hidden is dropped.
    land(&mut st, &target);
    assert!(st.session.tab_view.diff_expanded.gap_text().is_none());
    let counts = diff_runs::counts(&st).unwrap();
    assert!(counts.ws_hidden && counts.hunks);

    // No diff open.
    let mut none = state();
    run(&mut none, Msg::Diff(DiffMsg::ExpandAllUnchanged));
    run(&mut none, Msg::Diff(DiffMsg::CollapseAllUnchanged));
    assert!(none.session.tab_view.diff_expanded.keys().next().is_none());

    // A failed read closes every waiting run and logs once.
    let (mut st, target) = shown(DiffLayout::Inline, diff());
    run(&mut st, Msg::Diff(DiffMsg::ExpandAllUnchanged));
    let logs = st.log.len();
    let generation = st.session.diff.shown_gen;
    run(&mut st, Msg::Diff(DiffMsg::GapsLoaded { target, generation, result: Err(ReadFailed::FileChanged) }));
    assert!(st.session.tab_view.diff_expanded.keys().next().is_none());
    assert_eq!(st.log.len(), logs + 1);
    assert!(!diff_runs::reading(&st));
}

#[test]
fn the_reading_flag() {
    let (mut st, target) = shown(DiffLayout::Inline, diff());
    assert!(!diff_runs::reading(&st));
    run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(1)));
    assert!(diff_runs::reading(&st));
    // A result for another generation or another target is stale: the read is still on its way.
    let generation = st.session.diff.shown_gen;
    let text = text_of(&st);
    run(&mut st, Msg::Diff(DiffMsg::GapsLoaded { target: target.clone(), generation: generation + 1, result: Ok(text.clone()) }));
    run(&mut st, Msg::Diff(DiffMsg::GapsLoaded { target: DiffTarget::Staged("a.txt".into()), generation, result: Ok(text.clone()) }));
    assert!(diff_runs::reading(&st));
    // A second click while it is on its way asks for nothing more.
    assert_eq!(reads(&run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(9)))), 0);
    run(&mut st, Msg::Diff(DiffMsg::GapsLoaded { target: target.clone(), generation, result: Err(ReadFailed::FileChanged) }));
    assert!(!diff_runs::reading(&st), "an error ends it too");
    // After a new generation is shown the old flag no longer holds.
    run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(1)));
    assert!(diff_runs::reading(&st));
    st.session.diff.shown_gen += 1;
    assert!(!diff_runs::reading(&st));
    // The run's read was dropped, so a click asks again.
    assert_eq!(reads(&run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(1)))), 1);
}
