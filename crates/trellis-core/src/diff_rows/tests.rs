use super::*;
use crate::repo::{DiffLine, Hunk};

fn hunk(lines: &[(char, Option<u32>, Option<u32>, &str)]) -> Hunk {
    Hunk {
        header: "@@ -1 +1 @@".into(),
        lines: lines.iter().map(|&(origin, old, new, text)| DiffLine { origin, old, new, text: text.into() }).collect(),
    }
}

fn diff(hunks: Vec<Hunk>) -> FileDiff {
    FileDiff { binary: false, hunks, highlight: None, intraline: Default::default(), whole: None }
}

fn pair(old: Option<usize>, new: Option<usize>) -> SplitRow {
    SplitRow::Pair { hunk: 0, old, new }
}

#[test]
fn split_pairs_replacement_block() {
    let d = diff(vec![hunk(&[
        (' ', Some(1), Some(1), "ctx"),
        ('-', Some(2), None, "a"),
        ('-', Some(3), None, "b"),
        ('+', None, Some(2), "c"),
        (' ', Some(4), Some(3), "ctx"),
    ])]);
    let want = [SplitRow::Header(0), pair(Some(0), Some(0)), pair(Some(1), Some(3)), pair(Some(2), None), pair(Some(4), Some(4))];
    assert_eq!(split_rows(&d), want);
}

#[test]
fn split_more_adds_than_dels() {
    let d = diff(vec![hunk(&[('-', Some(1), None, "a"), ('+', None, Some(1), "b"), ('+', None, Some(2), "c")])]);
    assert_eq!(split_rows(&d), [SplitRow::Header(0), pair(Some(0), Some(1)), pair(None, Some(2))]);
}

#[test]
fn split_pure_add_and_pure_delete() {
    let added = diff(vec![hunk(&[('+', None, Some(1), "a"), ('+', None, Some(2), "b"), ('+', None, Some(3), "c")])]);
    let rows = split_rows(&added);
    assert_eq!(rows.len(), 4);
    assert!(rows[1..].iter().all(|r| matches!(r, SplitRow::Pair { old: None, new: Some(_), .. })));
    let deleted = diff(vec![hunk(&[('-', Some(1), None, "a"), ('-', Some(2), None, "b")])]);
    let rows = split_rows(&deleted);
    assert_eq!(rows.len(), 3);
    assert!(rows[1..].iter().all(|r| matches!(r, SplitRow::Pair { old: Some(_), new: None, .. })));
}

#[test]
fn split_deletion_after_additions_starts_new_block() {
    let d = diff(vec![hunk(&[('+', None, Some(1), "a"), ('-', Some(1), None, "b")])]);
    assert_eq!(split_rows(&d), [SplitRow::Header(0), pair(None, Some(0)), pair(Some(1), None)]);
}

const EOFNL: &str = "\\ No newline at end of file";

#[test]
fn split_no_newline_markers() {
    // Neither side ends in a newline: libgit2 emits `>` after the removed line (old side)
    // and `<` after the added line (new side), so `a` still pairs with `b`.
    let d = diff(vec![hunk(&[
        (' ', Some(1), Some(1), "x"),
        ('-', Some(2), None, "a"),
        ('>', Some(2), None, EOFNL),
        ('+', None, Some(2), "b"),
        ('<', None, Some(2), EOFNL),
    ])]);
    assert_eq!(split_rows(&d), [SplitRow::Header(0), pair(Some(0), Some(0)), pair(Some(1), Some(3)), pair(Some(2), Some(4))]);
    // Only the new file lacks the newline: its marker sits under the new side.
    let new_only = diff(vec![hunk(&[('-', Some(1), None, "a"), ('+', None, Some(1), "b"), ('<', None, Some(1), EOFNL)])]);
    assert_eq!(split_rows(&new_only), [SplitRow::Header(0), pair(Some(0), Some(1)), pair(None, Some(2))]);
    // Only the old file lacks it: its marker sits under the old side.
    let old_only = diff(vec![hunk(&[('-', Some(1), None, "a"), ('>', Some(1), None, EOFNL), ('+', None, Some(1), "b")])]);
    assert_eq!(split_rows(&old_only), [SplitRow::Header(0), pair(Some(0), Some(2)), pair(Some(1), None)]);
    let both = diff(vec![hunk(&[(' ', Some(1), Some(1), "a"), ('=', None, None, EOFNL)])]);
    assert_eq!(split_rows(&both), [SplitRow::Header(0), pair(Some(0), Some(0)), pair(Some(1), Some(1))]);
}

#[test]
fn split_multiple_hunks_and_header_rows() {
    let d = diff(vec![
        hunk(&[(' ', Some(1), Some(1), "x"), ('-', Some(2), None, "a"), ('-', Some(3), None, "b"), ('+', None, Some(2), "c")]),
        hunk(&[('+', None, Some(9), "d"), (' ', Some(9), Some(10), "y")]),
    ]);
    assert_eq!(header_rows(&d, DiffLayout::Inline), [0, 5]);
    let rows = split_rows(&d);
    let headers: Vec<usize> = rows.iter().enumerate().filter(|(_, r)| matches!(r, SplitRow::Header(_))).map(|(i, _)| i).collect();
    assert_eq!(header_rows(&d, DiffLayout::Split), headers);
    assert_eq!(headers, [0, 4]);
    assert_eq!(rows[5], SplitRow::Pair { hunk: 1, old: None, new: Some(0) });

    let empty = diff(Vec::new());
    assert!(split_rows(&empty).is_empty() && inline_rows(&empty).is_empty());
    assert!(header_rows(&empty, DiffLayout::Inline).is_empty() && header_rows(&empty, DiffLayout::Split).is_empty());
}

#[test]
fn inline_rows_matches_flattening() {
    let d = diff(vec![hunk(&[(' ', Some(1), Some(1), "x"), ('+', None, Some(2), "a")]), hunk(&[('-', Some(7), None, "b")])]);
    let want = [
        DiffRow::Header(0),
        DiffRow::Line { hunk: 0, line: 0 },
        DiffRow::Line { hunk: 0, line: 1 },
        DiffRow::Header(1),
        DiffRow::Line { hunk: 1, line: 0 },
    ];
    assert_eq!(inline_rows(&d), want);
}

#[test]
fn step_change_and_hunk_at() {
    let h = [0, 5, 12];
    assert_eq!(step_change(&h, 0, 1), Some(5));
    assert_eq!(step_change(&h, 5, 1), Some(12));
    assert_eq!(step_change(&h, 12, 1), None);
    assert_eq!(step_change(&h, 7, 1), Some(12));
    assert_eq!(step_change(&h, 12, -1), Some(5));
    assert_eq!(step_change(&h, 7, -1), Some(5));
    assert_eq!(step_change(&h, 0, -1), None);
    assert_eq!(step_change(&h, 7, 0), None);
    assert_eq!(step_change(&[], 0, 1), None);
    assert_eq!(step_change(&[], 3, -1), None);
    assert_eq!(hunk_at(&h, 7), 1);
    assert_eq!(hunk_at(&h, 0), 0);
    assert_eq!(hunk_at(&h, 99), 2);
    assert_eq!(hunk_at(&[], 3), 0);
}

/// A hunk of plain lines: each string's first character is its origin.
fn text_hunk(lines: &[&str]) -> Hunk {
    let lines: Vec<(char, Option<u32>, Option<u32>, &str)> =
        lines.iter().map(|l| (l.chars().next().unwrap(), None, None, &l[1..])).collect();
    hunk(&lines)
}

const CONFIG: [&str; 7] = [
    " [server]",
    "-host = \"localhost\"",
    "-port = 8080",
    "+# listen on all interfaces",
    "+host = \"0.0.0.0\"",
    "+port = 9090",
    " timeout = 30",
];

fn decorated(hunks: Vec<Hunk>) -> FileDiff {
    let mut d = diff(hunks);
    d.intraline = crate::intraline::intraline(&d);
    d
}

#[test]
fn split_rows_follow_similarity_pairs() {
    let plain = diff(vec![text_hunk(&CONFIG)]);
    let positional = [
        SplitRow::Header(0),
        pair(Some(0), Some(0)),
        pair(Some(1), Some(3)),
        pair(Some(2), Some(4)),
        pair(None, Some(5)),
        pair(Some(6), Some(6)),
    ];
    assert_eq!(split_rows(&plain), positional, "an undecorated diff pairs by position");
    let d = decorated(vec![text_hunk(&CONFIG)]);
    let want = [
        SplitRow::Header(0),
        pair(Some(0), Some(0)),
        pair(None, Some(3)),
        pair(Some(1), Some(4)),
        pair(Some(2), Some(5)),
        pair(Some(6), Some(6)),
    ];
    assert_eq!(split_rows(&d), want);

    // The same number of rows here, so a second hunk's header stays put; with a gap that puts
    // an unmatched line on each side, it moves down one.
    let two = decorated(vec![text_hunk(&CONFIG), text_hunk(&["-x", "+y"])]);
    assert_eq!(header_rows(&two, DiffLayout::Split), [0, 6]);
    let gap = ["-let a = compute(1);", "-zzz", "+qqq", "+let a = compute(2);"];
    let shifted = decorated(vec![text_hunk(&gap), text_hunk(&["-x", "+y"])]);
    assert_eq!(header_rows(&shifted, DiffLayout::Split), [0, 4]);
    assert_eq!(header_rows(&diff(shifted.hunks.clone()), DiffLayout::Split), [0, 3]);
}

#[test]
fn split_gap_fill_between_anchors() {
    let d = decorated(vec![text_hunk(&["-let alpha = 1;", "-zzz", "-let gamma = 3;", "+let alpha = 10;", "+qqq", "+let gamma = 30;"])]);
    assert_eq!(d.intraline.pairs().hunk(0), [(0, 3), (2, 5)]);
    assert_eq!(split_rows(&d), [SplitRow::Header(0), pair(Some(0), Some(3)), pair(Some(1), Some(4)), pair(Some(2), Some(5))]);

    let d = decorated(vec![text_hunk(&["-let alpha = 1;", "-zzz", "+qqq", "+let alpha = 10;"])]);
    assert_eq!(d.intraline.pairs().hunk(0), [(0, 3)]);
    assert_eq!(split_rows(&d), [SplitRow::Header(0), pair(None, Some(2)), pair(Some(0), Some(3)), pair(Some(1), None)]);
}

#[test]
fn split_rows_with_ignores_pairs_that_dont_fit() {
    let other = decorated(vec![text_hunk(&CONFIG)]);
    let d = diff(vec![text_hunk(&["-a", "+b", "+c"]), text_hunk(&[" x"])]);
    assert_eq!(split_rows_with(&d, other.intraline.pairs()), split_rows(&d));
    let short = diff(vec![text_hunk(&["-a", "+b"])]);
    assert_eq!(split_rows_with(&short, other.intraline.pairs()), [SplitRow::Header(0), pair(Some(0), Some(1))]);
    // A diff whose shape changed under its stale intraline: positional, no panic.
    let mut stale = other.clone();
    stale.hunks[0].lines.truncate(3);
    assert_eq!(split_rows(&stale), [SplitRow::Header(0), pair(Some(0), Some(0)), pair(Some(1), None), pair(Some(2), None)]);
}

#[test]
fn change_runs_cut_like_split_rows() {
    let runs = |lines: &[&str]| change_runs(&text_hunk(lines).lines);
    assert_eq!(runs(&["-a", "-b", "+c"]), [(0..2, 2..3)]);
    assert_eq!(runs(&["+a", "-b"]), [(0..0, 0..1), (1..2, 2..2)]);
    assert_eq!(runs(&["-a", ">m", "+b", "<m"]), [(0..2, 2..4)]);
    assert_eq!(runs(&[" x", "-a", " y", "+b", " z"]), [(1..2, 2..2), (3..3, 3..4)]);
    assert!(runs(&[" x", "=m"]).is_empty());
    assert!(runs(&[]).is_empty());
}
