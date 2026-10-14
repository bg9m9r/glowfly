//! Headless tests for line-level staging: selecting diff lines, applying them, and when the
//! selection resets.

use super::tests::{drive, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LocalOp};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::lines::LineClick::{self, Only, Range, Toggle};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::repo::{self, DiffLine, DiffTarget, FileDiff, Hunk, HunkOp, LoadOptions};

/// Two hunks; hunk 0 mixes context with changes: ` a`, `-b`, `+B`, ` c`, `+d`, `-e`.
fn two_hunks() -> FileDiff {
    let line = |origin: char, text: &str| DiffLine { origin, old: Some(1), new: Some(1), text: text.into() };
    let hunks = vec![
        Hunk {
            header: "@@ -1,4 +1,4 @@".into(),
            lines: vec![line(' ', "a"), line('-', "b"), line('+', "B"), line(' ', "c"), line('+', "d"), line('-', "e")],
        },
        Hunk { header: "@@ -20,1 +20,3 @@".into(), lines: vec![line(' ', "x"), line('+', "y"), line('+', "z")] },
    ];
    FileDiff { binary: false, hunks, highlight: None, intraline: Default::default(), whole: None }
}

fn unstaged() -> DiffTarget {
    DiffTarget::Unstaged("f.txt".into())
}

fn with_diff(target: DiffTarget) -> AppState {
    let mut st = state();
    st.diff = Some((target, two_hunks()));
    st
}

/// A loaded repository (so effects have a working directory) showing `two_hunks` as `target`.
fn with_repo(name: &str, target: DiffTarget) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = with_diff(target);
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    (st, dir)
}

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn click(st: &mut AppState, hunk: usize, line: usize, how: LineClick) {
    assert!(run(st, Msg::SelectDiffLine { hunk, line, how }).is_empty());
}

fn selected(st: &AppState) -> Option<(usize, Vec<usize>)> {
    let s = &st.view.diff_lines;
    (!s.is_empty()).then(|| (s.hunk, s.lines.iter().copied().collect()))
}

fn loaded(st: &mut AppState, target: DiffTarget, reload: bool, result: Result<FileDiff, String>) {
    let generation = st.diff_gen;
    run(st, Msg::DiffLoaded { target, reload, generation, result });
}

#[test]
fn click_shift_ctrl_rules() {
    let mut st = with_diff(unstaged());
    click(&mut st, 0, 1, Only);
    assert_eq!(selected(&st), Some((0, vec![1])));
    assert_eq!(st.view.diff_lines.header, "@@ -1,4 +1,4 @@");
    click(&mut st, 0, 1, Only);
    assert_eq!(selected(&st), None, "clicking the only selected line deselects it");

    // Shift selects from the anchor, skipping context; the anchor stays.
    click(&mut st, 0, 1, Only);
    click(&mut st, 0, 5, Range);
    assert_eq!(selected(&st), Some((0, vec![1, 2, 4, 5])));
    click(&mut st, 0, 2, Range);
    assert_eq!(selected(&st), Some((0, vec![1, 2])));

    // Ctrl toggles and moves the anchor.
    click(&mut st, 0, 4, Toggle);
    assert_eq!(selected(&st), Some((0, vec![1, 2, 4])));
    click(&mut st, 0, 1, Toggle);
    assert_eq!(selected(&st), Some((0, vec![2, 4])));
    click(&mut st, 0, 5, Range);
    assert_eq!(selected(&st), Some((0, vec![1, 2, 4, 5])), "a range from the moved anchor");
}

#[test]
fn clicks_across_hunks_and_on_context() {
    let mut st = with_diff(unstaged());
    click(&mut st, 0, 1, Only);
    click(&mut st, 0, 4, Toggle);
    // Another hunk starts over, whatever the modifier.
    click(&mut st, 1, 2, Range);
    assert_eq!(selected(&st), Some((1, vec![2])));
    click(&mut st, 0, 2, Toggle);
    assert_eq!(selected(&st), Some((0, vec![2])));
    // Shift or Ctrl on context does nothing; a plain click there clears.
    click(&mut st, 0, 0, Range);
    click(&mut st, 0, 3, Toggle);
    assert_eq!(selected(&st), Some((0, vec![2])));
    click(&mut st, 0, 3, Only);
    assert_eq!(selected(&st), None);
    // Shift with nothing selected is a plain click.
    click(&mut st, 0, 4, Range);
    assert_eq!(selected(&st), Some((0, vec![4])));
    run(&mut st, Msg::ClearLineSelection);
    assert_eq!(selected(&st), None);
}

#[test]
fn commit_diff_lines_not_selectable() {
    let mut st = with_diff(DiffTarget::Commit(git2::Oid::zero(), "f.txt".into()));
    click(&mut st, 0, 1, Only);
    click(&mut st, 0, 1, Range);
    click(&mut st, 0, 1, Toggle);
    assert_eq!(selected(&st), None);
    // Nor without a diff.
    let mut st = state();
    click(&mut st, 0, 1, Only);
    assert_eq!(selected(&st), None);
}

#[test]
fn apply_lines_emits_local_op() {
    let (mut st, dir) = with_repo("lines-apply", unstaged());
    assert!(run(&mut st, Msg::ApplyLines(HunkOp::Stage)).is_empty(), "nothing selected");

    click(&mut st, 0, 5, Only);
    click(&mut st, 0, 2, Toggle);
    assert!(run(&mut st, Msg::ApplyLines(HunkOp::Unstage)).is_empty(), "unstage doesn't fit an unstaged diff");
    assert!(!st.view.diff_lines.is_empty());
    let fx = run(&mut st, Msg::ApplyLines(HunkOp::Stage));
    let expected = st.diff.as_ref().unwrap().1.hunks[0].clone();
    assert_eq!(expected.header, "@@ -1,4 +1,4 @@");
    let want = LocalOp::Lines { op: HunkOp::Stage, path: "f.txt".into(), hunk: 0, expected, lines: vec![2, 5] };
    assert!(matches!(&fx[..], [Effect::LocalOp { op, .. }] if *op == want), "{fx:?}");
    assert!(st.view.diff_lines.is_empty(), "applying clears the selection");
    assert_eq!(st.hunk_ops_gen(), None, "hunk and line buttons wait for the reload");

    let mut st = with_diff(DiffTarget::Staged("f.txt".into()));
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    click(&mut st, 1, 1, Only);
    assert!(run(&mut st, Msg::ApplyLines(HunkOp::Stage)).is_empty(), "stage doesn't fit a staged diff");
    assert!(run(&mut st, Msg::ApplyLines(HunkOp::Discard)).is_empty());
    let fx = run(&mut st, Msg::ApplyLines(HunkOp::Unstage));
    assert!(matches!(&fx[..], [Effect::LocalOp { op: LocalOp::Lines { op: HunkOp::Unstage, hunk: 1, .. }, .. }]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discard_through_confirm() {
    let (mut st, dir) = with_repo("lines-confirm", unstaged());
    let confirm = || Msg::confirm("Discard lines", "Discard?".into(), "Discard", Msg::ApplyLines(HunkOp::Discard));
    click(&mut st, 0, 4, Only);
    run(&mut st, confirm());
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert!(matches!(&fx[..], [Effect::LocalOp { op: LocalOp::Lines { op: HunkOp::Discard, lines, .. }, .. }] if *lines == [4]));

    // The diff changes under the open confirmation: accepting does nothing.
    let mut st = with_diff(unstaged());
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    click(&mut st, 0, 4, Only);
    run(&mut st, confirm());
    let mut changed = two_hunks();
    changed.hunks[0].lines[4].text = "D".into();
    loaded(&mut st, unstaged(), true, Ok(changed));
    assert!(run(&mut st, Msg::ConfirmAccepted).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reload_keeps_identical_clears_changed() {
    let mut st = with_diff(unstaged());
    let select = |st: &mut AppState| {
        click(st, 0, 1, Only);
        assert!(!st.view.diff_lines.is_empty());
    };
    select(&mut st);
    loaded(&mut st, unstaged(), true, Ok(two_hunks()));
    assert_eq!(selected(&st), Some((0, vec![1])), "an identical reload keeps it");

    let mut changed = two_hunks();
    changed.hunks[1].lines.pop();
    loaded(&mut st, unstaged(), true, Ok(changed));
    assert_eq!(selected(&st), None, "a changed reload clears it");

    st.diff = Some((unstaged(), two_hunks()));
    select(&mut st);
    loaded(&mut st, unstaged(), false, Ok(two_hunks()));
    assert_eq!(selected(&st), None, "opening a diff clears it");

    select(&mut st);
    run(&mut st, Msg::CloseDiff);
    assert_eq!(selected(&st), None);

    st.diff = Some((unstaged(), two_hunks()));
    select(&mut st);
    loaded(&mut st, unstaged(), true, Err("no changes in f.txt".into()));
    assert!(st.diff.is_none() && selected(&st).is_none(), "a reload that finds nothing closes it");

    let (mut st, dir) = with_repo("lines-commit", unstaged());
    select(&mut st);
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    run(&mut st, Msg::SelectCommit(head));
    assert_eq!(selected(&st), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tab_switch_clears_selection() {
    let mut st = with_diff(unstaged());
    click(&mut st, 0, 1, Only);
    run(&mut st, Msg::NewTab);
    assert_eq!(selected(&st), None);
    run(&mut st, Msg::SelectTab(0));
    assert!(st.diff.as_ref().is_some_and(|(t, _)| *t == unstaged()), "the diff comes back");
    assert_eq!(selected(&st), None, "the selection does not");
}

#[test]
fn end_to_end_stage_lines() {
    let dir = scratch_repo("lines-e2e");
    std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    let mut st = state();
    drive(&mut st, Msg::Open(dir.clone()));
    drive(&mut st, Msg::OpenDiff(DiffTarget::Unstaged("a.txt".into())));
    let (_, d) = st.diff.as_ref().unwrap();
    let two = d.hunks[0].lines.iter().position(|l| l.origin == '+' && l.text == "two").unwrap();
    drive(&mut st, Msg::SelectDiffLine { hunk: 0, line: two, how: Only });
    drive(&mut st, Msg::ApplyLines(HunkOp::Stage));
    assert!(st.view.diff_lines.is_empty());
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.staged.iter().any(|f| f.path == "a.txt"), "a.txt is partly staged");
    assert!(snap.unstaged.iter().any(|f| f.path == "a.txt"), "and still has unstaged changes");
    // The reload after the operation shows what is left: just `+three`.
    let (_, d) = st.diff.as_ref().unwrap();
    let left: BTreeSet<String> = d.hunks.iter().flat_map(|h| &h.lines).filter(|l| l.origin == '+').map(|l| l.text.clone()).collect();
    assert_eq!(left, BTreeSet::from(["three".to_owned()]));

    drive(&mut st, Msg::OpenDiff(DiffTarget::Staged("a.txt".into())));
    let (_, d) = st.diff.as_ref().unwrap();
    let changes: Vec<String> =
        d.hunks.iter().flat_map(|h| &h.lines).filter(|l| l.origin != ' ').map(|l| format!("{}{}", l.origin, l.text)).collect();
    assert_eq!(changes, ["+two"]);
    let _ = std::fs::remove_dir_all(&dir);
}
