//! When the text selected in a diff's code goes: another diff opening, a reload whose lines
//! changed, the diff closing, the layout or view mode switching, or Clear; and what keeps it:
//! a reload bringing back the same lines (keyed to it then), and wrapping and fold changes,
//! which only move the rows it is on.

use super::tests::state;
use super::update;
use crate::msg::{DiffMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::diff_text::{LineRef, TextPos, TextSel, TextSide, Unit};
use crate::state::lines::LineClick;
use git2::Oid;
use glowfly_core::repo::{DiffLine, DiffTarget, FileDiff, Hunk};
use glowfly_core::settings::{DiffLayout, DiffMode};
use std::time::Instant;

/// One hunk: ` a`, `-b`, `+c`, `+d`.
fn diff() -> FileDiff {
    let line = |origin: char, text: &str| DiffLine { origin, old: Some(1), new: Some(1), text: text.into() };
    let lines = vec![line(' ', "a"), line('-', "b"), line('+', "c"), line('+', "d")];
    FileDiff {
        binary: false,
        hunks: vec![Hunk { header: "@@ -1,2 +1,3 @@".into(), lines }],
        highlight: None,
        intraline: Default::default(),
        whole: None,
        old_lines: None,
    }
}

fn commit_file(path: &str) -> DiffTarget {
    DiffTarget::Commit(Oid::from_str("2222222222222222222222222222222222222222").unwrap(), path.into())
}

fn run(st: &mut AppState, msg: Msg) {
    update(st, msg, Instant::now());
}

/// The diff of `target` arriving for the request on its way.
fn loaded(st: &mut AppState, target: &DiffTarget, reload: bool) {
    loaded_as(st, target, reload, diff());
}

/// `d`, the diff of `target`, arriving for the request on its way.
fn loaded_as(st: &mut AppState, target: &DiffTarget, reload: bool, d: FileDiff) {
    let generation = st.session.diff.generation;
    run(
        st,
        Msg::Repo(RepoMsg::DiffLoaded { target: target.clone(), reload, generation, result: Ok(d), lfs: None, raw: None, gitlink: None }),
    );
}

/// A state in the hunk view with `target` open and `a` to `c` selected as text, as a drag
/// would leave it.
fn with_text(target: &DiffTarget) -> AppState {
    let mut st = state();
    st.settings.diff_mode = DiffMode::Hunks;
    run(&mut st, Msg::Repo(RepoMsg::OpenDiff(target.clone())));
    loaded(&mut st, target, false);
    let at = |line| TextPos { line: LineRef::Hunk { hunk: 0, line }, col: 0 };
    let key = (target.clone(), st.session.diff.shown_gen);
    st.view.diff_text = Some(TextSel { head: at(2), ..TextSel::at(at(0), Unit::Char, TextSide::Inline, key) });
    st
}

#[test]
fn another_diff_clears_the_text_at_once() {
    let mut st = with_text(&commit_file("app.txt"));
    let other = commit_file("lib.txt");
    run(&mut st, Msg::Repo(RepoMsg::OpenDiff(other.clone())));
    assert!(st.view.diff_text.is_none(), "cleared while the new diff loads");
}

#[test]
fn a_reload_with_the_same_lines_keeps_the_text_for_the_new_load() {
    let target = DiffTarget::Unstaged("app.txt".into());
    let mut st = with_text(&target);
    let before = st.view.diff_text.clone().unwrap();
    st.session.diff.next();
    loaded(&mut st, &target, true);
    let after = st.view.diff_text.clone().expect("an auto-refresh keeps what is being copied");
    assert_eq!((after.anchor, after.head), (before.anchor, before.head));
    assert_eq!(after.key, (target, st.session.diff.shown_gen), "keyed to the load shown");
    assert_ne!(after.key.1, before.key.1);
}

#[test]
fn a_reload_with_changed_lines_clears_the_text() {
    let target = DiffTarget::Unstaged("app.txt".into());
    let mut st = with_text(&target);
    st.session.diff.next();
    let mut d = diff();
    d.hunks[0].lines[3].text = "e".into();
    loaded_as(&mut st, &target, true, d);
    assert!(st.view.diff_text.is_none(), "its lines may have moved");
}

#[test]
fn closing_the_diff_clears_the_text() {
    let mut st = with_text(&commit_file("app.txt"));
    run(&mut st, Msg::Repo(RepoMsg::CloseDiff));
    assert!(st.view.diff_text.is_none());
}

#[test]
fn switching_layout_or_mode_clears_the_text() {
    let mut st = with_text(&commit_file("app.txt"));
    run(&mut st, Msg::Diff(DiffMsg::SetDiffLayout(DiffLayout::Split)));
    assert!(st.view.diff_text.is_none(), "split's sides are other columns");
    let mut st = with_text(&commit_file("app.txt"));
    run(&mut st, Msg::Diff(DiffMsg::SetDiffMode(DiffMode::WholeFile)));
    assert!(st.view.diff_text.is_none(), "the whole file names its lines differently");
}

#[test]
fn wrapping_and_folds_keep_the_text() {
    let mut st = with_text(&commit_file("app.txt"));
    let kept = st.view.diff_text.clone();
    run(&mut st, Msg::Diff(DiffMsg::SetWordWrap(true)));
    run(&mut st, Msg::Diff(DiffMsg::ExpandAllUnchanged));
    run(&mut st, Msg::Diff(DiffMsg::CollapseAllUnchanged));
    run(&mut st, Msg::Diff(DiffMsg::ExpandUnchanged(1)));
    run(&mut st, Msg::Diff(DiffMsg::CollapseUnchanged(1)));
    assert!(kept.is_some());
    assert_eq!(st.view.diff_text, kept);
}

#[test]
fn clear_drops_the_text_and_keeps_the_lines() {
    let mut st = with_text(&commit_file("app.txt"));
    run(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line: 1, how: LineClick::Toggle }));
    run(&mut st, Msg::Diff(DiffMsg::ClearTextSelection));
    assert!(st.view.diff_text.is_none());
    assert_eq!(st.cues.diff_lines.lines.iter().copied().collect::<Vec<_>>(), [1], "the line selection stays");
}

#[test]
fn lines_picked_in_the_gutter_keep_the_text() {
    let mut st = with_text(&commit_file("app.txt"));
    let kept = st.view.diff_text.clone();
    run(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line: 2, how: LineClick::Toggle }));
    run(&mut st, Msg::Diff(DiffMsg::ClearLineSelection));
    assert_eq!(st.view.diff_text, kept);
}

#[test]
fn select_all_takes_every_line_of_the_side_asked_for() {
    let target = commit_file("app.txt");
    let mut st = with_text(&target);
    st.settings.diff_layout = DiffLayout::Split;
    run(&mut st, Msg::Diff(DiffMsg::SelectAllText { side: TextSide::New }));
    let sel = st.view.diff_text.clone().expect("everything on the new side");
    let at = |line, col| TextPos { line: LineRef::Hunk { hunk: 0, line }, col };
    // ` a` to the end of `+d`, the new side's last line, keyed to the load shown.
    assert_eq!((sel.anchor, sel.head, sel.side), (at(0, 0), at(3, 1), TextSide::New));
    assert_eq!(sel.key, (target, st.session.diff.shown_gen));
    // The old side ends at `-b`.
    run(&mut st, Msg::Diff(DiffMsg::SelectAllText { side: TextSide::Old }));
    assert_eq!(st.view.diff_text.as_ref().map(|s| s.head), Some(at(1, 1)));
}

#[test]
fn select_all_needs_a_side_the_layout_has() {
    let mut st = with_text(&commit_file("app.txt"));
    let kept = st.view.diff_text.clone();
    // The inline layout has no new side: nothing changes.
    run(&mut st, Msg::Diff(DiffMsg::SelectAllText { side: TextSide::New }));
    assert_eq!(st.view.diff_text, kept);
    run(&mut st, Msg::Diff(DiffMsg::SelectAllText { side: TextSide::Inline }));
    assert_eq!(st.view.diff_text.as_ref().map(|s| s.head.line), Some(LineRef::Hunk { hunk: 0, line: 3 }));
}

/// A 30-line file with hunks at old and new lines 6-8 and 20-22, so runs of 5 lines lie above
/// the first and 8 below the last.
fn with_edges() -> FileDiff {
    let hunk = |at: u32| {
        let line = |origin: char, n: u32, text: &str| DiffLine { origin, old: Some(n), new: Some(n), text: text.into() };
        let lines = vec![line(' ', at, "before"), line('-', at + 1, "old"), line('+', at + 1, "new"), line(' ', at + 2, "after")];
        Hunk { header: format!("@@ -{at},3 +{at},3 @@"), lines }
    };
    FileDiff { hunks: vec![hunk(6), hunk(20)], old_lines: Some(30), ..diff() }
}

#[test]
fn select_all_reaches_the_opened_edges() {
    use glowfly_core::diff_rows::{GapLines, GapText, gaps};
    let target = DiffTarget::Unstaged("app.txt".into());
    let mut st = state();
    st.settings.diff_mode = DiffMode::Hunks;
    st.session.diff.view = Some((target.clone(), std::sync::Arc::new(with_edges())));
    let d = st.session.diff.view.as_ref().unwrap().1.clone();
    let runs = gaps(&d);
    assert_eq!(runs.iter().map(|g| g.hunk).collect::<Vec<_>>(), [0, 1, 2]);
    let text = runs
        .iter()
        .map(|g| GapLines { span: *g, lines: (0..g.len).map(|i| format!("{}:{i}", g.hunk)).collect(), spans: vec![Vec::new(); g.len] })
        .collect();
    for g in &runs {
        st.session.tab_view.diff_expanded.insert(g.key);
    }
    st.session.tab_view.diff_expanded.set_gap_text(std::sync::Arc::new(GapText::new(text)));
    run(&mut st, Msg::Diff(DiffMsg::SelectAllText { side: TextSide::Inline }));
    let sel = st.view.diff_text.clone().expect("everything");
    let at = |line, col| TextPos { line: LineRef::Gap { hunk: 2, line }, col };
    assert_eq!(sel.anchor, TextPos { line: LineRef::Gap { hunk: 0, line: 0 }, col: 0 });
    assert_eq!(sel.head, at(7, "2:7".len()), "the end of the run below's last line");

    // With whitespace hidden the runs aren't drawn: hunk lines only.
    st.settings.diff_ignore_whitespace = true;
    run(&mut st, Msg::Diff(DiffMsg::SelectAllText { side: TextSide::Inline }));
    let sel = st.view.diff_text.clone().unwrap();
    assert_eq!(sel.anchor, TextPos { line: LineRef::Hunk { hunk: 0, line: 0 }, col: 0 });
    assert_eq!(sel.head, TextPos { line: LineRef::Hunk { hunk: 1, line: 3 }, col: "after".len() });

    // Only the run above open: it starts the selection, the last hunk line ends it.
    st.settings.diff_ignore_whitespace = false;
    st.session.tab_view.diff_expanded.remove(runs[2].key);
    run(&mut st, Msg::Diff(DiffMsg::SelectAllText { side: TextSide::Inline }));
    let sel = st.view.diff_text.clone().unwrap();
    assert_eq!(sel.anchor.line, LineRef::Gap { hunk: 0, line: 0 });
    assert_eq!(sel.head.line, LineRef::Hunk { hunk: 1, line: 3 });
}
