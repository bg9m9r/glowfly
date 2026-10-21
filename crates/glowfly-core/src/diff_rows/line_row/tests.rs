use super::*;
use crate::diff_rows::DiffRow;
use crate::repo::Hunk;
use crate::whole_file::WholeFile;
use crate::whole_file::tests::{diff, two_changes};

const HUNKS: DiffMode = DiffMode::Hunks;
const WHOLE: DiffMode = DiffMode::WholeFile;

fn l(origin: char, old: Option<u32>, new: Option<u32>, text: &str) -> DiffLine {
    DiffLine { origin, old, new, text: text.into() }
}

fn ctx(old: u32, new: u32) -> DiffLine {
    l(' ', Some(old), Some(new), "same")
}

fn hunk(lines: Vec<DiffLine>) -> Hunk {
    Hunk { header: "@@ -1 +1 @@".into(), lines }
}

/// Two hunks: lines 2-3 replaced, then one line added after line 10.
fn sample() -> FileDiff {
    diff(vec![
        hunk(vec![
            ctx(1, 1),
            l('-', Some(2), None, "a"),
            l('-', Some(3), None, "b"),
            l('+', None, Some(2), "c"),
            l('+', None, Some(3), "d"),
            ctx(4, 4),
        ]),
        hunk(vec![ctx(10, 10), l('+', None, Some(11), "e"), ctx(11, 12)]),
    ])
}

fn row(d: &FileDiff, mode: DiffMode, layout: DiffLayout, ex: &Expanded, side: LineSide, n: u32) -> Option<usize> {
    row_of_file_line(d, mode, layout, ex, side, n)
}

#[test]
fn line_keys_by_origin() {
    use LineSide::{New, Old};
    assert_eq!(line_keys(&l('-', Some(4), None, "x")), [Some((Old, 4)), None]);
    assert_eq!(line_keys(&l('+', None, Some(5), "x")), [Some((New, 5)), None]);
    assert_eq!(line_keys(&ctx(6, 7)), [Some((New, 7)), Some((Old, 6))]);
    for marker in ['<', '>', '='] {
        assert_eq!(line_keys(&l(marker, Some(1), Some(1), "\\ No newline")), [None, None], "{marker}");
    }
}

#[test]
fn line_index_tells_old_from_new() {
    let d = sample();
    assert_eq!(line_index(&d, LineSide::Old, 3), Some((0, 2)), "the deleted line 3");
    assert_eq!(line_index(&d, LineSide::New, 3), Some((0, 4)), "the added line 3");
    assert_eq!(line_index(&d, LineSide::New, 12), Some((1, 2)), "context by its new number");
    assert_eq!(line_index(&d, LineSide::Old, 11), Some((1, 2)), "context by its old number");
    assert_eq!(line_index(&d, LineSide::New, 40), None);
}

#[test]
fn hunk_view_rows_inline_and_split() {
    let (d, ex) = (sample(), Expanded::default());
    let inline = |side, n| row(&d, HUNKS, DiffLayout::Inline, &ex, side, n);
    assert_eq!(inline(LineSide::Old, 3), Some(3));
    assert_eq!(inline(LineSide::New, 3), Some(5));
    assert_eq!(inline(LineSide::New, 11), Some(9), "after the second hunk's header");

    // Split rows: header, (0,0), (1,3), (2,4), (5,5), header, (0,0), (-,1), (2,2).
    let split = |side, n| row(&d, HUNKS, DiffLayout::Split, &ex, side, n);
    assert_eq!(split(LineSide::Old, 2), Some(2), "a deleted line on the left of its pair");
    assert_eq!(split(LineSide::New, 2), Some(2), "the added line beside it");
    assert_eq!(split(LineSide::Old, 3), Some(3));
    assert_eq!(split(LineSide::New, 4), Some(4), "context");
    assert_eq!(split(LineSide::New, 11), Some(7));
}

#[test]
fn lines_outside_the_hunks_have_no_row() {
    let (d, ex) = (sample(), Expanded::default());
    for layout in [DiffLayout::Inline, DiffLayout::Split] {
        assert_eq!(row(&d, HUNKS, layout, &ex, LineSide::New, 7), None, "{layout:?}");
        assert_eq!(row(&d, HUNKS, layout, &ex, LineSide::Old, 30), None, "{layout:?}");
    }
}

#[test]
fn similarity_pairs_out_of_order_find_the_cell() {
    let mut d = diff(vec![hunk(vec![
        l('-', Some(1), None, "let alpha = 1;"),
        l('-', Some(2), None, "zzz"),
        l('+', None, Some(1), "qqq"),
        l('+', None, Some(2), "let alpha = 10;"),
    ])]);
    d.intraline = crate::intraline::intraline(&d);
    // Rows: header, (-, 2), (0, 3), (1, -).
    let split = |side, n| row(&d, HUNKS, DiffLayout::Split, &Expanded::default(), side, n);
    assert_eq!(split(LineSide::New, 1), Some(1));
    assert_eq!(split(LineSide::New, 2), Some(2));
    assert_eq!(split(LineSide::Old, 1), Some(2));
    assert_eq!(split(LineSide::Old, 2), Some(3));
}

fn whole_diff() -> (FileDiff, WholeFile) {
    let (hunks, whole) = two_changes();
    let mut d = diff(hunks);
    let w = WholeFile::new(&d.hunks, whole).expect("maps");
    d.whole = Some(Box::new(w.clone()));
    (d, w)
}

#[test]
fn whole_file_view_lands_on_the_fold_or_the_line() {
    let (d, w) = whole_diff();
    let closed = Expanded::default();
    let open = Expanded::all(&w);
    // Line 8 is unchanged and far from both changes, so it is folded until opened.
    let r = row(&d, WHOLE, DiffLayout::Inline, &closed, LineSide::New, 8).expect("shown");
    assert!(matches!(whole_file::inline_rows(&w, &closed)[r], DiffRow::Fold(_)), "the fold row");
    let r = row(&d, WHOLE, DiffLayout::Inline, &open, LineSide::New, 8).expect("shown");
    assert_eq!(whole_file::inline_rows(&w, &open)[r], DiffRow::Line { hunk: 0, line: 8 });

    let r = row(&d, WHOLE, DiffLayout::Split, &closed, LineSide::New, 8).expect("shown");
    assert!(matches!(whole_file::split_rows(&w, &closed)[r], SplitRow::Fold(_)));
    let r = row(&d, WHOLE, DiffLayout::Split, &open, LineSide::New, 8).expect("shown");
    assert_eq!(whole_file::split_rows(&w, &open)[r], SplitRow::Pair { hunk: 0, old: Some(8), new: Some(8) });

    // The changed lines: the deleted line 2 on the left, the added line 13 on the right.
    let r = row(&d, WHOLE, DiffLayout::Split, &closed, LineSide::Old, 2).expect("shown");
    assert!(matches!(whole_file::split_rows(&w, &closed)[r], SplitRow::Pair { old: Some(1), .. }));
    let r = row(&d, WHOLE, DiffLayout::Inline, &closed, LineSide::New, 13).expect("shown");
    assert_eq!(whole_file::inline_rows(&w, &closed)[r], DiffRow::Line { hunk: 0, line: 13 });
    assert_eq!(row(&d, WHOLE, DiffLayout::Inline, &closed, LineSide::New, 99), None, "past the file");
    // The same diff in the hunk view hides line 8.
    assert_eq!(row(&d, HUNKS, DiffLayout::Inline, &closed, LineSide::New, 8), None);
}
