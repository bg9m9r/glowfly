//! Headless tests for selecting diff lines by press-and-drag: the range, how it is kept to its
//! hunk, the modifiers held at the press, and when a drag is ignored or ends.

use super::lines_tests::{click, loaded, run, selected, two_hunks, unstaged, with_diff};
use super::tests::{drive, scratch_repo, state};
use crate::msg::{DiffMsg, Msg, RepoMsg, StageMsg};
use crate::state::AppState;
use crate::state::lines::LineClick::{self, Only, Range, Toggle};
use glowfly_core::repo::{DiffTarget, HunkOp};
use glowfly_core::settings::DiffMode;

fn start(st: &mut AppState, hunk: usize, line: usize, how: LineClick) {
    assert!(run(st, Msg::Diff(DiffMsg::StartLineDrag { hunk, line, how })).is_empty());
}

fn over(st: &mut AppState, hunk: usize, line: usize) {
    assert!(run(st, Msg::Diff(DiffMsg::DragDiffLine { hunk, line })).is_empty());
}

fn end(st: &mut AppState) {
    assert!(run(st, Msg::Diff(DiffMsg::EndLineDrag)).is_empty());
}

fn dragging(st: &AppState) -> bool {
    st.view.diff_lines.dragging().is_some()
}

#[test]
fn drag_selects_from_press_to_pointer() {
    let mut st = with_diff(unstaged());
    start(&mut st, 0, 1, Only);
    assert_eq!(selected(&st), Some((0, vec![1])));
    over(&mut st, 0, 4);
    assert_eq!(selected(&st), Some((0, vec![1, 2, 4])), "context is skipped");
    over(&mut st, 0, 2);
    assert_eq!(selected(&st), Some((0, vec![1, 2])), "it shrinks as the pointer comes back");
    over(&mut st, 0, 0);
    assert_eq!(selected(&st), Some((0, vec![1])), "over context above the start");
    end(&mut st);
    assert!(!dragging(&st));
    assert_eq!(selected(&st), Some((0, vec![1])), "the selection stays");

    // Upwards.
    start(&mut st, 0, 5, Only);
    over(&mut st, 0, 1);
    assert_eq!(selected(&st), Some((0, vec![1, 2, 4, 5])));
    assert_eq!(st.view.diff_lines.anchor, 5, "the press line is the anchor");
    end(&mut st);
    click(&mut st, 0, 2, Range);
    assert_eq!(selected(&st), Some((0, vec![2, 4, 5])), "Shift-click extends from the press line");
}

#[test]
fn drag_is_kept_to_its_hunk() {
    let mut st = with_diff(unstaged());
    start(&mut st, 0, 2, Only);
    over(&mut st, 1, 2);
    assert_eq!(selected(&st), Some((0, vec![2, 4, 5])), "a later hunk stops at the last change");
    over(&mut st, 0, 0);
    assert_eq!(selected(&st), Some((0, vec![1, 2])), "the header runs to the first change");
    end(&mut st);

    start(&mut st, 1, 2, Only);
    over(&mut st, 0, 4);
    assert_eq!(selected(&st), Some((1, vec![1, 2])), "an earlier hunk stops at the first change");
}

#[test]
fn drag_modifiers_on_press() {
    // Ctrl/Cmd adds the dragged range to the lines already selected.
    let mut st = with_diff(unstaged());
    click(&mut st, 0, 1, Only);
    start(&mut st, 0, 4, Toggle);
    assert_eq!(selected(&st), Some((0, vec![1, 4])));
    over(&mut st, 0, 5);
    assert_eq!(selected(&st), Some((0, vec![1, 4, 5])));
    end(&mut st);

    // Shift runs from the existing anchor, which stays.
    let mut st = with_diff(unstaged());
    click(&mut st, 0, 1, Only);
    start(&mut st, 0, 4, Range);
    assert_eq!(selected(&st), Some((0, vec![1, 2, 4])));
    over(&mut st, 0, 5);
    assert_eq!(selected(&st), Some((0, vec![1, 2, 4, 5])));
    assert_eq!(st.view.diff_lines.anchor, 1);
    end(&mut st);

    // A plain drag starting on the only selected line keeps it.
    let mut st = with_diff(unstaged());
    click(&mut st, 0, 1, Only);
    start(&mut st, 0, 1, Only);
    assert_eq!(selected(&st), Some((0, vec![1])));
    end(&mut st);

    // Ctrl or Shift in another hunk starts over.
    for how in [Toggle, Range] {
        let mut st = with_diff(unstaged());
        click(&mut st, 0, 1, Only);
        start(&mut st, 1, 1, how);
        over(&mut st, 1, 2);
        assert_eq!(selected(&st), Some((1, vec![1, 2])), "{how:?}");
        assert_eq!(st.view.diff_lines.anchor, 1);
    }
}

#[test]
fn drag_ignored_when() {
    let mut st = with_diff(unstaged());
    over(&mut st, 0, 4);
    assert_eq!(selected(&st), None, "no drag started");
    start(&mut st, 0, 0, Only);
    assert!(!dragging(&st) && selected(&st).is_none(), "a press on context");
    start(&mut st, 0, 1, Only);
    end(&mut st);
    over(&mut st, 0, 5);
    assert_eq!(selected(&st), Some((0, vec![1])), "after the release");

    let mut whole = with_diff(unstaged());
    whole.settings.diff_mode = DiffMode::WholeFile;
    start(&mut whole, 0, 1, Only);
    assert!(!dragging(&whole) && selected(&whole).is_none(), "the whole-file view");

    // Clicks, clearing and a changed reload end a drag.
    let mut st = with_diff(unstaged());
    start(&mut st, 0, 1, Only);
    click(&mut st, 0, 4, Only);
    assert!(!dragging(&st));
    assert_eq!(selected(&st), Some((0, vec![4])));
    start(&mut st, 0, 1, Only);
    run(&mut st, Msg::Diff(DiffMsg::ClearLineSelection));
    assert!(!dragging(&st) && selected(&st).is_none());
    start(&mut st, 0, 1, Only);
    let mut changed = two_hunks();
    changed.hunks[0].lines[4].text = "D".into();
    loaded(&mut st, unstaged(), true, Ok(changed));
    assert!(!dragging(&st) && selected(&st).is_none());
}

#[test]
fn drag_unstage_every_line_untracks_the_file() {
    let dir = scratch_repo("lines-drag-untrack");
    std::fs::write(dir.join("n.txt"), "one\ntwo\n").unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Stage(StageMsg::StageFile("n.txt".into())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Staged("n.txt".into()))));
    let (_, d) = st.diff.as_ref().unwrap();
    let at = |text: &str| d.hunks[0].lines.iter().position(|l| l.origin == '+' && l.text == text).unwrap();
    let (one, two) = (at("one"), at("two"));
    drive(&mut st, Msg::Diff(DiffMsg::StartLineDrag { hunk: 0, line: one, how: Only }));
    drive(&mut st, Msg::Diff(DiffMsg::DragDiffLine { hunk: 0, line: two }));
    drive(&mut st, Msg::Diff(DiffMsg::EndLineDrag));
    assert_eq!(selected(&st), Some((0, vec![one, two])));
    drive(&mut st, Msg::Stage(StageMsg::ApplyLines(HunkOp::Unstage)));
    let snap = st.snap.as_ref().unwrap();
    assert!(!snap.staged.iter().any(|f| f.path == "n.txt"), "{:?}", snap.staged);
    assert!(snap.unstaged.iter().any(|f| f.path == "n.txt" && f.status == '?'), "{:?}", snap.unstaged);
    let _ = std::fs::remove_dir_all(&dir);
}
