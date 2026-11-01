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
    run(st, Msg::Repo(RepoMsg::DiffLoaded { target: target.clone(), reload, generation, result: Ok(d), lfs: None, raw: None }));
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
    assert_eq!(st.view.diff_lines.lines.iter().copied().collect::<Vec<_>>(), [1], "the line selection stays");
}

#[test]
fn lines_picked_in_the_gutter_keep_the_text() {
    let mut st = with_text(&commit_file("app.txt"));
    let kept = st.view.diff_text.clone();
    run(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line: 2, how: LineClick::Toggle }));
    run(&mut st, Msg::Diff(DiffMsg::ClearLineSelection));
    assert_eq!(st.view.diff_text, kept);
}
