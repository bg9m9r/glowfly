//! Which line Open in external editor opens at, on a handmade diff of two hunks: the first
//! selected line, the row menu's line, the first change of the hunk on screen, lines the working
//! file lacks, the whole-file view, opened runs and the old-side case.

use super::*;
use crate::state::diff_text::{TextPos, TextSel, TextSide, Unit};
use crate::state::lines::LineSelection;
use git2::Oid;
use glowfly_core::repo::Hunk;
use glowfly_core::whole_file::WholeFile;

fn dl(origin: char, old: Option<u32>, new: Option<u32>) -> DiffLine {
    DiffLine { origin, old, new, text: format!("{origin}{old:?}{new:?}") }
}

fn plain(hunks: Vec<Hunk>) -> FileDiff {
    FileDiff { binary: false, hunks, highlight: None, intraline: Default::default(), whole: None, old_lines: None }
}

/// h0 `@@ -1,3 +1,3 @@`: context 1, removed 2, added 2, context 3; h1 `@@ -10,4 +10,2 @@`: context 10 (10),
/// removed 11, removed 12, context 13 (new 11). The run between them is 6 lines from line 4 on
/// both sides.
fn hunks() -> Vec<Hunk> {
    let ctx = |n, m| dl(' ', Some(n), Some(m));
    vec![
        Hunk { header: "@@ -1,3 +1,3 @@".into(), lines: vec![ctx(1, 1), dl('-', Some(2), None), dl('+', None, Some(2)), ctx(3, 3)] },
        Hunk {
            header: "@@ -10,4 +10,2 @@".into(),
            lines: vec![ctx(10, 10), dl('-', Some(11), None), dl('-', Some(12), None), ctx(13, 11)],
        },
    ]
}

fn diff() -> FileDiff {
    plain(hunks())
}

/// The same diff with the whole file loaded: indexes 0..=3 are h0, 4..=9 the run, 10..=13 h1.
fn whole() -> FileDiff {
    let mut lines = hunks()[0].lines.clone();
    lines.extend((4..=9).map(|n| dl(' ', Some(n), Some(n))));
    lines.extend(hunks()[1].lines.clone());
    let whole = Hunk { header: "@@ -1,13 +1,11 @@".into(), lines };
    let mut d = diff();
    d.whole = Some(Box::new(WholeFile::new(&d.hunks, whole).expect("the hunks are in the whole file")));
    d
}

fn hunk(hunk: usize, line: usize) -> LineRef {
    LineRef::Hunk { hunk, line }
}

fn picked<'a>(d: &'a FileDiff, lines: Option<(usize, &'a BTreeSet<usize>)>, text: Option<[LineRef; 2]>, at: Option<LineRef>) -> Picked<'a> {
    Picked { diff: d, side: LineSide::New, lines, text, at }
}

fn set(lines: &[usize]) -> BTreeSet<usize> {
    lines.iter().copied().collect()
}

fn never() -> Option<usize> {
    panic!("the hunk on screen is only asked for without a selection or a row line")
}

#[test]
fn a_selection_gives_its_first_line() {
    let d = diff();
    assert_eq!(choose(&picked(&d, Some((0, &set(&[2]))), None, None), never), Some(2));
    // Dragged upwards: the anchor after the head; the first line is the topmost.
    assert_eq!(choose(&picked(&d, None, Some([hunk(1, 3), hunk(0, 3)]), None), never), Some(3));
    // The gutter's first line in hunk 1 (line 11) and text in hunk 0 (line 3): the smaller.
    assert_eq!(choose(&picked(&d, Some((1, &set(&[1, 2]))), Some([hunk(0, 3), hunk(0, 3)]), None), never), Some(3));
}

#[test]
fn a_removed_only_selection_maps_to_the_next_line_that_exists() {
    let d = diff();
    assert_eq!(choose(&picked(&d, Some((1, &set(&[1, 2]))), None, None), never), Some(11));
    let trailing = plain(vec![Hunk { header: "@@ -1,2 +1,1 @@".into(), lines: vec![dl(' ', Some(1), Some(1)), dl('-', Some(2), None)] }]);
    assert_eq!(choose(&picked(&trailing, Some((0, &set(&[1]))), None, None), never), Some(1), "none follows: the nearest before");
    let only = plain(vec![Hunk { header: "@@ -1 +0,0 @@".into(), lines: vec![dl('-', Some(1), None)] }]);
    assert_eq!(choose(&picked(&only, Some((0, &set(&[0]))), None, None), || Some(0)), None);
}

#[test]
fn the_row_menu_line_is_used_without_a_selection() {
    let d = diff();
    assert_eq!(choose(&picked(&d, None, None, Some(hunk(0, 3))), never), Some(3));
    assert_eq!(choose(&picked(&d, Some((0, &set(&[2]))), None, Some(hunk(0, 3))), never), Some(2), "a selection wins");
    assert_eq!(choose(&picked(&d, None, Some([hunk(1, 0), hunk(1, 0)]), Some(hunk(0, 3))), never), Some(10), "text wins too");
}

#[test]
fn without_a_selection_the_hunk_on_screen_gives_its_first_change() {
    let d = diff();
    let p = picked(&d, None, None, None);
    assert_eq!(choose(&p, || Some(0)), Some(2));
    assert_eq!(choose(&p, || Some(1)), Some(11));
    assert_eq!(choose(&p, || None), None);
    assert_eq!(choose(&p, || Some(5)), None, "a hunk that isn't there");
}

#[test]
fn the_whole_file_view_maps_its_lines_directly() {
    let d = whole();
    let at = |line| Some(LineRef::Whole { line });
    assert_eq!(choose(&picked(&d, None, None, at(3)), never), Some(3), "context");
    assert_eq!(choose(&picked(&d, None, None, at(2)), never), Some(2), "added");
    assert_eq!(choose(&picked(&d, None, None, at(1)), never), Some(2), "removed: the next line that exists");
    assert_eq!(choose(&picked(&d, None, None, at(12)), never), Some(11));
    assert_eq!(choose(&picked(&d, None, None, at(6)), never), Some(6), "inside the run");
    let end = [LineRef::Whole { line: 12 }, LineRef::Whole { line: 13 }];
    assert_eq!(choose(&picked(&d, None, Some(end), None), never), Some(11));
    assert_eq!(file_line(&d, LineSide::New, LineRef::Whole { line: 40 }), None);
    assert_eq!(file_line(&diff(), LineSide::New, LineRef::Whole { line: 1 }), None, "no whole file loaded");
}

#[test]
fn an_opened_run_gives_its_own_line() {
    let d = diff();
    assert_eq!(file_line(&d, LineSide::New, LineRef::Gap { hunk: 1, line: 2 }), Some(6));
    assert_eq!(file_line(&d, LineSide::New, LineRef::Gap { hunk: 1, line: 5 }), Some(9));
    assert_eq!(file_line(&d, LineSide::New, LineRef::Gap { hunk: 1, line: 6 }), None, "past the run");
    assert_eq!(file_line(&d, LineSide::New, LineRef::Gap { hunk: 0, line: 0 }), None, "no run above the first hunk");
}

#[test]
fn the_working_tree_is_the_old_side_of_a_diff_from_it() {
    let oid = Oid::from_bytes(&[7; 20]).unwrap();
    let p = String::from("a.txt");
    assert_eq!(working_side(&DiffTarget::FromWorkdir(oid, p.clone())), LineSide::Old);
    for t in [
        DiffTarget::Unstaged(p.clone()),
        DiffTarget::Staged(p.clone()),
        DiffTarget::Commit(oid, p.clone()),
        DiffTarget::Between(oid, oid, p.clone()),
        DiffTarget::ToWorkdir(oid, p.clone()),
    ] {
        assert_eq!(working_side(&t), LineSide::New, "{t:?}");
    }
    let d = diff();
    // The added line has no old number: the next line with one, context 3.
    assert_eq!(file_line(&d, LineSide::Old, hunk(0, 2)), Some(3));
    assert_eq!(file_line(&d, LineSide::Old, hunk(0, 1)), Some(2));
    assert_eq!(file_line(&d, LineSide::Old, LineRef::Gap { hunk: 1, line: 2 }), Some(6));
    assert_eq!(file_line(&d, LineSide::New, hunk(0, 1)), Some(2), "a removed line takes the next new line");
}

#[test]
fn picked_ignores_what_belongs_to_another_diff() {
    let d = diff();
    let t = DiffTarget::Unstaged("a.txt".into());
    let other = DiffTarget::Unstaged("b.txt".into());
    let lines = |target: &DiffTarget, header: &str| LineSelection {
        target: Some(target.clone()),
        hunk: 0,
        header: header.into(),
        lines: set(&[2]),
        ..Default::default()
    };
    let (good, wrong_target, wrong_header) = (lines(&t, "@@ -1,3 +1,3 @@"), lines(&other, "@@ -1,3 +1,3 @@"), lines(&t, "@@ -2,3 +2,3 @@"));
    assert!(Picked::of(&t, &d, Some(&good), None, 3, None).lines.is_some());
    assert!(Picked::of(&t, &d, Some(&wrong_target), None, 3, None).lines.is_none());
    assert!(Picked::of(&t, &d, Some(&wrong_header), None, 3, None).lines.is_none(), "the hunk changed since");
    assert!(Picked::of(&t, &d, Some(&LineSelection::default()), None, 3, None).lines.is_none());

    let sel = |target: &DiffTarget, generation, head_col| TextSel {
        anchor: TextPos { line: hunk(0, 1), col: 0 },
        head: TextPos { line: hunk(0, 2), col: head_col },
        unit: Unit::Char,
        side: TextSide::Inline,
        key: (target.clone(), generation),
    };
    assert_eq!(Picked::of(&t, &d, None, Some(&sel(&t, 3, 2)), 3, None).text, Some([hunk(0, 1), hunk(0, 2)]));
    assert!(Picked::of(&t, &d, None, Some(&sel(&other, 3, 2)), 3, None).text.is_none());
    assert!(Picked::of(&t, &d, None, Some(&sel(&t, 2, 2)), 3, None).text.is_none(), "an earlier load");
    let empty = TextSel { head: TextPos { line: hunk(0, 1), col: 0 }, ..sel(&t, 3, 0) };
    assert!(Picked::of(&t, &d, None, Some(&empty), 3, None).text.is_none(), "a click selects nothing");
}
