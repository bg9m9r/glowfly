//! Headless tests for the hunk view's gaps between hunks: opening one reads the text once and
//! fills the rows in, stale results are ignored, closing, whitespace hidden, keeping the top row,
//! reloads, other files, the whole-file view and a failed read.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{DiffMsg, Msg, RepoMsg};
use crate::runner;
use crate::state::AppState;
use glowfly_core::diff_rows::{DiffRow, GapSpan, GapText, gaps, header_rows_filled, inline_rows, inline_rows_filled};
use glowfly_core::repo::{self, DiffTarget, FileDiff};
use glowfly_core::settings::{DiffLayout, DiffMode};
use glowfly_core::whole_file;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// Lines 1..=30 of f.txt, with lines 2, 15 and 28 changed when `changed`.
fn body(changed: bool) -> String {
    (1..=30).map(|n| if changed && [2, 15, 28].contains(&n) { format!("changed {n}\n") } else { format!("line {n}\n") }).collect()
}

fn f_txt() -> DiffTarget {
    DiffTarget::Unstaged("f.txt".into())
}

/// A repository whose f.txt has three hunks (lines 1-5, 12-18 and 25-30), open on its diff.
pub(super) fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    std::fs::write(dir.join("f.txt"), body(false)).unwrap();
    std::fs::write(dir.join("g.txt"), "one\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    std::fs::write(dir.join("f.txt"), body(true)).unwrap();
    std::fs::write(dir.join("g.txt"), "two\n").unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(f_txt())));
    (st, dir)
}

fn shown(st: &AppState) -> Arc<FileDiff> {
    st.session.diff.view.as_ref().map(|(_, d)| d.clone()).expect("a diff is open")
}

fn spans(st: &AppState) -> Vec<GapSpan> {
    gaps(&shown(st))
}

fn gap_lines(st: &AppState) -> usize {
    inline_rows_filled(&shown(st), &st.session.tab_view.diff_expanded).iter().filter(|r| matches!(r, DiffRow::GapLine { .. })).count()
}

fn loads(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::LoadDiffGaps { .. })).count()
}

/// Runs the LoadDiffGaps effects among `fx` and applies their results.
pub(super) fn land(st: &mut AppState, fx: Vec<Effect>) {
    for e in fx.into_iter().filter(|e| matches!(e, Effect::LoadDiffGaps { .. })) {
        let msg = runner::execute(e).expect("a result");
        run(st, msg);
    }
}

#[test]
fn opening_a_gap_reads_the_text_once_and_fills_the_rows() {
    let (mut st, dir) = opened("gaps-open");
    let g = spans(&st);
    assert_eq!(g.iter().map(|g| (g.hunk, g.key, g.len)).collect::<Vec<_>>(), [(1, 6, 6), (2, 19, 6)]);
    let fx = run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(g[0].key)));
    assert_eq!(loads(&fx), 1, "{fx:?}");
    assert!(st.session.tab_view.diff_expanded.contains(g[0].key));
    assert_eq!(gap_lines(&st), 0, "closed until the text arrives");
    assert_eq!(loads(&run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(g[0].key)))), 0, "already on its way");
    land(&mut st, fx);
    assert_eq!(gap_lines(&st), 6);
    let text = st.session.tab_view.diff_expanded.gap_text().expect("kept");
    assert_eq!(text.lines_for(&g[0]).unwrap().lines, (6..=11).map(|n| format!("line {n}")).collect::<Vec<_>>());

    // The other gap's text came with it.
    assert_eq!(loads(&run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(g[1].key)))), 0);
    assert_eq!(gap_lines(&st), 12);
    run(&mut st, Msg::Diff(DiffMsg::CollapseUnchanged(g[0].key)));
    run(&mut st, Msg::Diff(DiffMsg::CollapseUnchanged(g[1].key)));
    assert_eq!(gap_lines(&st), 0);
    assert_eq!(inline_rows_filled(&shown(&st), &st.session.tab_view.diff_expanded), inline_rows(&shown(&st)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stale_or_other_results_are_ignored() {
    let (mut st, dir) = opened("gaps-stale");
    let key = spans(&st)[0].key;
    run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(key)));
    let text = Arc::new(GapText::default());
    let shown_gen = st.session.diff.shown_gen;
    for (target, generation) in [(f_txt(), shown_gen + 1), (DiffTarget::Unstaged("g.txt".into()), shown_gen)] {
        run(&mut st, Msg::Diff(DiffMsg::GapsLoaded { target, generation, result: Ok(text.clone()) }));
        assert!(st.session.tab_view.diff_expanded.gap_text().is_none());
        run(&mut st, Msg::Diff(DiffMsg::GapsLoaded { target: f_txt(), generation: shown_gen + 1, result: Err("x".into()) }));
        assert!(st.session.tab_view.diff_expanded.contains(key));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn whitespace_hidden_shows_no_gaps() {
    let (mut st, dir) = opened("gaps-ws");
    let key = spans(&st)[0].key;
    st.settings.diff_ignore_whitespace = true;
    assert!(run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(key))).is_empty());
    assert!(st.session.tab_view.diff_expanded.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opening_a_gap_above_the_top_keeps_the_top_row() {
    for layout in [DiffLayout::Inline, DiffLayout::Split] {
        let (mut st, dir) = opened(&format!("gaps-anchor-{layout:?}"));
        st.settings.diff_layout = layout;
        let g = spans(&st);
        // Read the text with the view at the top, then fold the gap back.
        let fx = run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(g[1].key)));
        land(&mut st, fx);
        run(&mut st, Msg::Diff(DiffMsg::CollapseUnchanged(g[1].key)));
        let headers = header_rows_filled(&shown(&st), layout, &st.session.tab_view.diff_expanded);
        st.session.tab_view.diff_top_row = headers[2];
        st.view.diff_cursor = Some(headers[2]);
        st.view.diff_scroll_to_row = None;

        assert!(run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(g[0].key))).is_empty(), "the text is there");
        let now = header_rows_filled(&shown(&st), layout, &st.session.tab_view.diff_expanded);
        assert_eq!(now[2], headers[2] + g[0].len, "{layout:?}");
        assert_eq!((st.session.tab_view.diff_top_row, st.view.diff_scroll_to_row), (now[2], Some(now[2])), "{layout:?}");
        assert_eq!(st.view.diff_cursor, Some(now[2]), "{layout:?}: the stepped-to change keeps its place");
        // Folding it back returns to the same row.
        run(&mut st, Msg::Diff(DiffMsg::CollapseUnchanged(g[0].key)));
        assert_eq!(st.session.tab_view.diff_top_row, headers[2], "{layout:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn reloads_keep_the_gap_and_other_files_reset_it() {
    let (mut st, dir) = opened("gaps-reload");
    let key = spans(&st)[0].key;
    let fx = run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(key)));
    land(&mut st, fx);
    let text = st.session.tab_view.diff_expanded.gap_text().cloned();

    // A reload of the same file keeps the gap open and reads its text again; the old text shows
    // meanwhile.
    let d = repo::load_diff(&dir, &f_txt()).unwrap();
    let generation = st.session.diff.generation;
    let fx =
        run(&mut st, Msg::Repo(RepoMsg::DiffLoaded { target: f_txt(), reload: true, generation, result: Ok(d), lfs: None, raw: None }));
    assert_eq!(loads(&fx), 1, "{fx:?}");
    assert!(st.session.tab_view.diff_expanded.contains(key));
    assert_eq!(st.session.tab_view.diff_expanded.gap_text().cloned(), text);
    assert_eq!(gap_lines(&st), 6);

    // The whole-file view shows the same run's fold opened.
    drive(&mut st, Msg::Diff(DiffMsg::SetDiffMode(DiffMode::WholeFile)));
    let d = shown(&st);
    let w = whole_file::shown(&d, DiffMode::WholeFile).expect("the whole file");
    assert!(whole_file::opened(w, &st.session.tab_view.diff_expanded).iter().any(|f| f.key == key));
    drive(&mut st, Msg::Diff(DiffMsg::SetDiffMode(DiffMode::Hunks)));
    assert_eq!(gap_lines(&st), 6, "open again in the hunk view");

    // Another file starts with every gap closed and no text.
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("g.txt".into()))));
    assert!(st.session.tab_view.diff_expanded.is_empty() && st.session.tab_view.diff_expanded.gap_text().is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_read_closes_the_gap_and_logs() {
    let (mut st, dir) = opened("gaps-error");
    let key = spans(&st)[0].key;
    run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(key)));
    let generation = st.session.diff.shown_gen;
    run(&mut st, Msg::Diff(DiffMsg::GapsLoaded { target: f_txt(), generation, result: Err("boom".into()) }));
    assert!(!st.session.tab_view.diff_expanded.contains(key));
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some("These unchanged lines can't be shown: boom"));
    let _ = std::fs::remove_dir_all(&dir);
}
