//! Headless tests for the hunk view's runs above the first hunk and below the last, through the
//! runner: opening them one at a time and all at once (one read of the text), across a reload,
//! with the fold keys, shared with the whole-file view, and with staging while they are open.

use super::diff_preview_tests::deliver_diff;
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use glowfly_core::diff_rows::{DiffRow, gaps, header_rows_filled, inline_rows, inline_rows_filled};
use glowfly_core::repo::{DiffTarget, FileDiff, HunkOp};
use glowfly_core::settings::{DiffLayout, DiffMode};
use glowfly_core::testkit::git_out;
use glowfly_core::whole_file;
use glowfly_model::effect::{DiffEffect, Effect};
use glowfly_model::msg::{DiffMsg, Msg, RepoMsg, StageMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::lines::LineClick;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// Lines 1..=40 of f.txt, `line N`, with the numbers in `changed` rewritten.
fn body(changed: &[usize]) -> String {
    (1..=40).map(|n| if changed.contains(&n) { format!("changed {n}\n") } else { format!("line {n}\n") }).collect()
}

fn f_txt() -> DiffTarget {
    DiffTarget::Unstaged("f.txt".into())
}

/// A repository whose f.txt is changed at lines 12 and 28 (hunks 9-15 and 25-31, so runs of 8
/// lines above, 9 between and 9 below, keys 1, 16 and 32), open on its diff in the hunk view.
pub(in crate::flows) fn edges(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    std::fs::write(dir.join("f.txt"), body(&[])).unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    std::fs::write(dir.join("f.txt"), body(&[12, 28])).unwrap();
    let mut st = state();
    st.settings.diff_mode = DiffMode::Hunks;
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(f_txt())));
    (st, dir)
}

fn shown(st: &AppState) -> Arc<FileDiff> {
    st.session.diff.view.as_ref().map(|(_, d)| d.clone()).expect("a diff is open")
}

fn rows(st: &AppState) -> Vec<DiffRow> {
    inline_rows_filled(&shown(st), &st.session.tab_view.diff_expanded)
}

fn reads(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::Diff(DiffEffect::Gaps { .. }))).count()
}

/// Runs the reads among `fx` and applies their results.
fn land(st: &mut AppState, fx: Vec<Effect>) {
    super::diff_gaps_tests::land(st, fx);
}

/// The text of the line a run's row shows, from the text read.
fn text_at(st: &AppState, row: DiffRow) -> String {
    let DiffRow::GapLine { hunk, line } = row else { panic!("not a run's line: {row:?}") };
    let g = gaps(&shown(st)).into_iter().find(|g| g.hunk == hunk).unwrap();
    st.session.tab_view.diff_expanded.gap_text().unwrap().lines_for(&g).unwrap().lines[line].clone()
}

#[test]
fn the_edges_open_through_the_runner() {
    let (mut st, dir) = edges("edges-open");
    let runs = gaps(&shown(&st));
    assert_eq!(runs.iter().map(|g| (g.hunk, g.key, g.len)).collect::<Vec<_>>(), [(0, 1, 8), (1, 16, 9), (2, 32, 9)]);
    assert!(matches!(rows(&st).last(), Some(DiffRow::GapBelow(g)) if g.key == 32));

    let fx = run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(1)));
    assert_eq!(reads(&fx), 1, "{fx:?}");
    assert!(glowfly_model::state::diff_runs::reading(&st));
    assert_eq!(rows(&st).first(), Some(&DiffRow::Header(0)), "closed until the text lands");
    land(&mut st, fx);
    assert!(!glowfly_model::state::diff_runs::reading(&st));
    let all = rows(&st);
    for (i, row) in all[..8].iter().enumerate() {
        assert_eq!(*row, DiffRow::GapLine { hunk: 0, line: i });
        assert_eq!(text_at(&st, *row), format!("line {}", i + 1));
    }
    assert_eq!(all[8], DiffRow::Header(0));

    // The run below opens from the text already read: no new read.
    let fx = run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(32)));
    assert_eq!(reads(&fx), 0);
    let all = rows(&st);
    for (i, row) in all[all.len() - 9..].iter().enumerate() {
        assert_eq!(*row, DiffRow::GapLine { hunk: 2, line: i });
        assert_eq!(text_at(&st, *row), format!("line {}", i + 32));
    }
    assert!(!all.iter().any(|r| matches!(r, DiffRow::GapBelow(_))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn expand_all_reads_once() {
    let (mut st, dir) = edges("edges-all");
    let fx = run(&mut st, Msg::Diff(DiffMsg::ExpandAllUnchanged));
    assert_eq!(reads(&fx), 1, "{fx:?}");
    assert_eq!(reads(&run(&mut st, Msg::Diff(DiffMsg::ExpandAllUnchanged))), 0, "already on its way");
    land(&mut st, fx);
    let all = rows(&st);
    assert_eq!(all.iter().filter(|r| matches!(r, DiffRow::GapLine { .. })).count(), 26);
    assert!(!all.iter().any(|r| matches!(r, DiffRow::GapBelow(_))));

    run(&mut st, Msg::Diff(DiffMsg::CollapseAllUnchanged));
    let closed = rows(&st);
    let plain = inline_rows(&shown(&st));
    assert_eq!(closed[..plain.len()], plain[..]);
    assert!(matches!(closed.last(), Some(DiffRow::GapBelow(_))) && closed.len() == plain.len() + 1);

    let fx = run(&mut st, Msg::Diff(DiffMsg::ExpandAllUnchanged));
    assert_eq!(reads(&fx), 0, "the text is there");
    assert_eq!(rows(&st), all);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn expand_all_survives_a_reload() {
    let (mut st, dir) = edges("edges-reload");
    let fx = run(&mut st, Msg::Diff(DiffMsg::ExpandAllUnchanged));
    land(&mut st, fx);
    assert_eq!(rows(&st).iter().filter(|r| matches!(r, DiffRow::GapLine { .. })).count(), 26);

    // Only line 28 differs now: one hunk (25-31), a run of 24 lines above (key 1) and one of 9 below.
    std::fs::write(dir.join("f.txt"), body(&[28])).unwrap();
    let mut fx = Vec::new();
    super::repo::reload_diff(&mut st, &mut fx);
    let fx = deliver_diff(&mut st, fx);
    assert_eq!(reads(&fx), 1, "the text is read again: {fx:?}");
    assert_eq!(shown(&st).hunks.len(), 1);
    land(&mut st, fx);
    let all = rows(&st);
    assert_eq!(all[..24], (0..24).map(|line| DiffRow::GapLine { hunk: 0, line }).collect::<Vec<_>>()[..]);
    assert_eq!(text_at(&st, all[23]), "line 24");
    assert_eq!(all[all.len() - 9..], (0..9).map(|line| DiffRow::GapLine { hunk: 1, line }).collect::<Vec<_>>()[..]);

    // After Collapse all a further reload starts no read.
    run(&mut st, Msg::Diff(DiffMsg::CollapseAllUnchanged));
    let mut fx = Vec::new();
    super::repo::reload_diff(&mut st, &mut fx);
    let fx = deliver_diff(&mut st, fx);
    assert_eq!(reads(&fx), 0, "{fx:?}");
    assert!(st.session.tab_view.diff_expanded.keys().next().is_none());
    assert!(matches!(rows(&st).last(), Some(DiffRow::GapBelow(_))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fold_keys_reach_the_edges() {
    for layout in [DiffLayout::Inline, DiffLayout::Split] {
        let (mut st, dir) = edges(&format!("edges-keys-{layout:?}"));
        st.settings.diff_layout = layout;
        st.view.diff_visible_rows = 80;
        st.view.tab.diff_top_row = 0;
        // From the top: the run above, then the one between, then the one below.
        let fx = run(&mut st, Msg::Diff(DiffMsg::OpenFoldAtTop));
        assert_eq!(reads(&fx), 1, "{layout:?}");
        land(&mut st, fx);
        for _ in 0..2 {
            assert_eq!(reads(&run(&mut st, Msg::Diff(DiffMsg::OpenFoldAtTop))), 0);
        }
        let keys: Vec<u32> = st.session.tab_view.diff_expanded.keys().collect();
        assert_eq!(keys, [1, 16, 32], "{layout:?}");
        // Nothing is left to open.
        run(&mut st, Msg::Diff(DiffMsg::OpenFoldAtTop));
        assert_eq!(st.session.tab_view.diff_expanded.keys().count(), 3);
        // Fold back closes the run that holds the top row: the run above.
        run(&mut st, Msg::Diff(DiffMsg::CloseFoldAtTop));
        assert!(!st.session.tab_view.diff_expanded.contains(1), "{layout:?}");

        // With few visible rows, opening the run below scrolls to its first line.
        run(&mut st, Msg::Diff(DiffMsg::CollapseAllUnchanged));
        st.view.diff_visible_rows = 2;
        let header = header_rows_filled(&shown(&st), layout, &st.session.tab_view.diff_expanded)[1];
        st.view.tab.diff_top_row = header + 1;
        st.view.diff_scroll_to_row = None;
        run(&mut st, Msg::Diff(DiffMsg::OpenFoldAtTop));
        assert!(st.session.tab_view.diff_expanded.contains(32), "{layout:?}");
        let to = st.view.diff_scroll_to_row.expect("scrolls");
        assert_eq!(st.view.tab.diff_top_row, to);
        if layout == DiffLayout::Inline {
            // The text was read earlier, so the run's first line is already there.
            assert_eq!(rows(&st)[to], DiffRow::GapLine { hunk: 2, line: 0 });
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn views_share_the_edge_runs() {
    let (mut st, dir) = edges("edges-views");
    let fx = run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(32)));
    land(&mut st, fx);
    drive(&mut st, Msg::Diff(DiffMsg::SetDiffMode(DiffMode::WholeFile)));
    let d = shown(&st);
    let w = whole_file::shown(&d, DiffMode::WholeFile).expect("the whole file");
    assert!(whole_file::opened(w, &st.session.tab_view.diff_expanded).iter().any(|f| f.key == 32), "the same key");
    drive(&mut st, Msg::Diff(DiffMsg::SetDiffMode(DiffMode::Hunks)));
    let all = rows(&st);
    assert_eq!(all[all.len() - 9..], (0..9).map(|line| DiffRow::GapLine { hunk: 2, line }).collect::<Vec<_>>()[..], "filled again");

    // From the whole-file view to Changes only: the run below is a closed row at once, with no
    // reload, as the old file's line count came with the whole file.
    run(&mut st, Msg::Diff(DiffMsg::CollapseAllUnchanged));
    drive(&mut st, Msg::Diff(DiffMsg::SetDiffMode(DiffMode::WholeFile)));
    assert!(shown(&st).whole.is_some());
    let fx = run(&mut st, Msg::Diff(DiffMsg::SetDiffMode(DiffMode::Hunks)));
    assert!(!fx.iter().any(|e| matches!(e, Effect::Diff(DiffEffect::Load { .. }))), "no reload: {fx:?}");
    assert!(matches!(rows(&st).last(), Some(DiffRow::GapBelow(g)) if g.key == 32 && g.len == 9));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staging_with_the_edges_open() {
    let (mut st, dir) = edges("edges-stage");
    let fx = run(&mut st, Msg::Diff(DiffMsg::ExpandAllUnchanged));
    land(&mut st, fx);
    // Pick the changed line of hunk 0 and drag past the end of the file, over the run below.
    let n = shown(&st).hunks.len();
    run(&mut st, Msg::Diff(DiffMsg::StartLineDrag { hunk: 0, line: 3, how: LineClick::Toggle }));
    run(&mut st, Msg::Diff(DiffMsg::DragDiffLine { hunk: n, line: 0 }));
    run(&mut st, Msg::Diff(DiffMsg::EndLineDrag));
    assert_eq!(st.cues.diff_lines.lines.iter().copied().collect::<Vec<_>>(), [3, 4], "ends at the hunk's last changed line");

    let generation = st.hunk_ops_gen().expect("hunk buttons enabled");
    drive(&mut st, Msg::Stage(StageMsg::Hunk { op: HunkOp::Stage, path: "f.txt".into(), index: 0, generation }));
    let staged = git_out(&dir, &["diff", "--cached", "-U0"]);
    assert!(staged.contains("+changed 12") && !staged.contains("changed 28"), "{staged}");
    // The reloaded diff keeps the run above open (now lines 1-24) and reads it again.
    assert_eq!(shown(&st).hunks.len(), 1);
    assert!(st.session.tab_view.diff_expanded.contains(1));
    let all = rows(&st);
    assert_eq!(all[..24], (0..24).map(|line| DiffRow::GapLine { hunk: 0, line }).collect::<Vec<_>>()[..]);
    let _ = std::fs::remove_dir_all(&dir);
}
