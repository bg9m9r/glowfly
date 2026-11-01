//! The diff line selection is never there by default: opening a diff, another file of the same
//! commit or the same file again leaves nothing selected, while a reload of the same diff keeps
//! what was picked.

use super::tests::state;
use super::update;
use crate::msg::{DiffMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::lines::LineClick;
use git2::Oid;
use glowfly_core::repo::{DiffLine, DiffTarget, FileDiff, Hunk};
use glowfly_core::settings::DiffMode;
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
    DiffTarget::Commit(Oid::from_str("1111111111111111111111111111111111111111").unwrap(), path.into())
}

fn run(st: &mut AppState, msg: Msg) {
    update(st, msg, Instant::now());
}

/// The diff of `target` arriving for the request on its way (or a reload of the one shown).
fn loaded(st: &mut AppState, target: &DiffTarget, reload: bool) {
    let generation = st.session.diff.generation;
    run(st, Msg::Repo(RepoMsg::DiffLoaded { target: target.clone(), reload, generation, result: Ok(diff()), lfs: None, raw: None }));
}

/// Opens `target` from a file list and lets its diff arrive.
fn open(st: &mut AppState, target: &DiffTarget) {
    run(st, Msg::Repo(RepoMsg::OpenDiff(target.clone())));
    loaded(st, target, false);
}

/// The lines selected in the diff on screen, in any hunk.
fn shown_selected(st: &AppState) -> Vec<usize> {
    let (target, d) = st.session.diff.view.as_ref().expect("a diff on screen");
    (0..d.hunks.len()).flat_map(|h| (0..d.hunks[h].lines.len()).filter(move |&i| st.view.diff_lines.contains(target, h, i))).collect()
}

/// A state in the hunk view (where lines can be picked) with `target` open and its `-b` and
/// `+d` lines picked from the gutter.
fn with_selection(target: &DiffTarget) -> AppState {
    let mut st = state();
    st.settings.diff_mode = DiffMode::Hunks;
    open(&mut st, target);
    for line in [1, 3] {
        run(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line, how: LineClick::Toggle }));
    }
    assert_eq!(shown_selected(&st), [1, 3]);
    st
}

#[test]
fn opening_a_commit_file_selects_nothing() {
    let mut st = state();
    st.settings.diff_mode = DiffMode::Hunks;
    open(&mut st, &commit_file("app.txt"));
    assert!(st.view.diff_lines.is_empty());
    assert!(st.view.diff_lines.target.is_none(), "no diff owns a selection");
    assert_eq!(shown_selected(&st), Vec::<usize>::new());
}

#[test]
fn opening_another_file_clears_the_lines_at_once() {
    let mut st = with_selection(&commit_file("app.txt"));
    let other = commit_file("lib.txt");
    run(&mut st, Msg::Repo(RepoMsg::OpenDiff(other.clone())));
    assert!(st.view.diff_lines.is_empty(), "cleared while the new diff loads");
    loaded(&mut st, &other, false);
    assert_eq!(shown_selected(&st), Vec::<usize>::new());
}

#[test]
fn opening_the_same_file_again_clears_the_lines() {
    let target = commit_file("app.txt");
    let mut st = with_selection(&target);
    open(&mut st, &target);
    assert!(st.view.diff_lines.is_empty());
}

#[test]
fn a_working_tree_file_after_a_commit_file_selects_nothing() {
    let mut st = with_selection(&commit_file("app.txt"));
    open(&mut st, &DiffTarget::Unstaged("app.txt".into()));
    assert!(st.view.diff_lines.is_empty());
}

#[test]
fn a_reload_of_the_same_diff_keeps_the_lines() {
    let target = DiffTarget::Unstaged("app.txt".into());
    let mut st = with_selection(&target);
    loaded(&mut st, &target, true);
    assert_eq!(shown_selected(&st), [1, 3], "the picked lines survive a reload that brings back the same diff");
}
