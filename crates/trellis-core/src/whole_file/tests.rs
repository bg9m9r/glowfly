use super::*;

pub(super) fn l(origin: char, old: Option<u32>, new: Option<u32>, text: &str) -> DiffLine {
    DiffLine { origin, old, new, text: text.into() }
}

pub(super) fn ctx(old: u32, new: u32) -> DiffLine {
    l(' ', Some(old), Some(new), &format!("line {old}"))
}

pub(super) fn hunk(lines: Vec<DiffLine>) -> Hunk {
    Hunk { header: "@@ -1 +1 @@".into(), lines }
}

pub(super) fn diff(hunks: Vec<Hunk>) -> FileDiff {
    FileDiff { binary: false, hunks, highlight: None, intraline: Default::default(), whole: None }
}

pub(super) const EOFNL: &str = "\\ No newline at end of file";

/// Context, a replaced line, ten context lines, an added line and a last context line, plus
/// the two normal hunks (3 lines of context) that cover it.
pub(super) fn two_changes() -> (Vec<Hunk>, Hunk) {
    let mut whole = vec![ctx(1, 1), l('-', Some(2), None, "old two"), l('+', None, Some(2), "new two")];
    whole.extend((3..=12).map(|n| ctx(n, n)));
    whole.push(l('+', None, Some(13), "added"));
    whole.push(ctx(13, 14));
    let first = hunk(vec![ctx(1, 1), whole[1].clone(), whole[2].clone(), ctx(3, 3), ctx(4, 4), ctx(5, 5)]);
    let second = hunk(vec![ctx(10, 10), ctx(11, 11), ctx(12, 12), whole[13].clone(), ctx(13, 14)]);
    (vec![first, second], hunk(whole))
}

pub(super) fn whole_file() -> WholeFile {
    let (hunks, whole) = two_changes();
    let mut d = diff(hunks);
    d.whole = WholeFile::new(&d.hunks, whole).map(Box::new);
    *d.whole.expect("maps")
}

#[test]
fn change_blocks_map_each_hunk() {
    let (hunks, whole) = two_changes();
    let blocks = change_blocks(&hunks, &whole.lines).expect("maps");
    assert_eq!(blocks, [ChangeBlock { hunk: 0, start: 1, end: 3 }, ChangeBlock { hunk: 1, start: 13, end: 14 }]);
}

#[test]
fn two_runs_in_one_hunk_make_one_block() {
    let whole = vec![ctx(1, 1), l('-', Some(2), None, "a"), ctx(3, 2), ctx(4, 3), l('+', None, Some(4), "b"), ctx(5, 5)];
    let normal = hunk(whole.clone());
    let blocks = change_blocks(&[normal], &whole).expect("maps");
    assert_eq!(blocks, [ChangeBlock { hunk: 0, start: 1, end: 5 }], "the context between the runs is inside the block");
}

#[test]
fn block_includes_no_newline_markers() {
    // Neither side ends in a newline: the markers after the last change belong to the block.
    let whole =
        vec![ctx(1, 1), l('-', Some(2), None, "a"), l('>', Some(2), None, EOFNL), l('+', None, Some(2), "b"), l('<', None, Some(2), EOFNL)];
    let blocks = change_blocks(&[hunk(whole.clone())], &whole).expect("maps");
    assert_eq!(blocks, [ChangeBlock { hunk: 0, start: 1, end: 5 }]);

    // A context line without a newline is not part of any change.
    let whole = vec![l('+', None, Some(1), "new"), ctx(1, 2), l('=', None, None, EOFNL)];
    let blocks = change_blocks(&[hunk(whole.clone())], &whole).expect("maps");
    assert_eq!(blocks, [ChangeBlock { hunk: 0, start: 0, end: 1 }]);
}

#[test]
fn unmappable_hunk_returns_none() {
    let (mut hunks, whole) = two_changes();
    hunks[1].lines[3].new = Some(99);
    assert_eq!(change_blocks(&hunks, &whole.lines), None, "an added line that isn't in the whole file");

    let (mut hunks, whole) = two_changes();
    hunks.swap(0, 1);
    assert_eq!(change_blocks(&hunks, &whole.lines), None, "blocks out of order");
    assert!(WholeFile::new(&hunks, whole).is_none());
}

#[test]
fn whole_inline_rows() {
    let w = whole_file();
    let all = Expanded::all(&w);
    let rows = inline_rows(&w, &all);
    assert_eq!(rows.len(), w.diff.hunks[0].lines.len() + w.blocks.len());
    assert!(!rows.iter().any(|r| matches!(r, DiffRow::Header(_))));
    for b in &w.blocks {
        let at = rows.iter().position(|r| *r == DiffRow::Block(b.hunk)).expect("an action row per block");
        assert_eq!(rows[at + 1], DiffRow::Line { hunk: 0, line: b.start });
    }
    assert_eq!(rows[0], DiffRow::Line { hunk: 0, line: 0 });
    assert_eq!(block_rows(&w, DiffLayout::Inline, &all), [1, 14]);
}

/// The whole file of one normal hunk that is exactly `lines`.
pub(super) fn single(lines: Vec<DiffLine>) -> WholeFile {
    WholeFile::new(&[hunk(lines.clone())], hunk(lines)).expect("maps")
}

#[test]
fn whole_split_rows() {
    let w = whole_file();
    let all = Expanded::all(&w);
    let rows = split_rows(&w, &all);
    assert!(!rows.iter().any(|r| matches!(r, SplitRow::Header(_))), "the leading header is dropped");
    let pair = |old: Option<usize>, new: Option<usize>| SplitRow::Pair { hunk: 0, old, new };
    assert_eq!(&rows[..4], [pair(Some(0), Some(0)), SplitRow::Block(0), pair(Some(1), Some(2)), pair(Some(3), Some(3))]);
    let at = rows.iter().position(|r| *r == SplitRow::Block(1)).unwrap();
    assert_eq!(rows[at + 1], pair(None, Some(13)));
    let blocks: Vec<usize> = rows.iter().enumerate().filter(|(_, r)| matches!(r, SplitRow::Block(_))).map(|(i, _)| i).collect();
    assert_eq!(block_rows(&w, DiffLayout::Split, &all), blocks);
    assert_eq!(blocks, [1, at]);

    // Pure add and pure delete blocks.
    let added = single(vec![l('+', None, Some(1), "a"), l('+', None, Some(2), "b")]);
    let none = Expanded::default();
    assert_eq!(split_rows(&added, &none), [SplitRow::Block(0), pair(None, Some(0)), pair(None, Some(1))]);
    let deleted = single(vec![ctx(1, 1), l('-', Some(2), None, "a")]);
    assert_eq!(split_rows(&deleted, &none), [pair(Some(0), Some(0)), SplitRow::Block(0), pair(Some(1), None)]);
    assert_eq!(block_rows(&deleted, DiffLayout::Inline, &none), [1]);
    assert_eq!(block_rows(&deleted, DiffLayout::Split, &none), [1]);
}

#[test]
fn change_rows_by_mode() {
    let (hunks, whole) = two_changes();
    let mut d = diff(hunks);
    let none = Expanded::default();
    for layout in [DiffLayout::Inline, DiffLayout::Split] {
        assert_eq!(change_rows(&d, DiffMode::Hunks, layout, &none), header_rows(&d, layout));
        assert_eq!(change_rows(&d, DiffMode::WholeFile, layout, &none), header_rows(&d, layout), "no whole file loaded");
    }
    assert_eq!(hunk_of_change(&d, DiffMode::WholeFile, 1), Some(1));
    assert_eq!(hunk_of_change(&d, DiffMode::Hunks, 2), None);

    d.whole = WholeFile::new(&d.hunks, whole).map(Box::new);
    let w = d.whole.as_deref().unwrap();
    let all = Expanded::all(w);
    for layout in [DiffLayout::Inline, DiffLayout::Split] {
        assert_eq!(change_rows(&d, DiffMode::WholeFile, layout, &all), block_rows(w, layout, &all));
        assert_eq!(change_rows(&d, DiffMode::Hunks, layout, &all), header_rows(&d, layout), "the setting decides");
    }
    for mode in [DiffMode::Hunks, DiffMode::WholeFile] {
        for i in 0..2 {
            let h = hunk_of_change(&d, mode, i).unwrap();
            assert_eq!(change_of_hunk(&d, mode, h), Some(i), "{mode:?}");
        }
        assert_eq!(hunk_of_change(&d, mode, 2), None);
        assert_eq!(change_of_hunk(&d, mode, 2), None);
    }
    assert!(shown(&d, DiffMode::WholeFile).is_some() && shown(&d, DiffMode::Hunks).is_none());
}

#[test]
fn block_rows_are_ignored_by_hunk_helpers() {
    // diff_rows only ever sees whole.diff, which has no Block rows; the split rows built from it
    // keep their headers and pairs, and intraline still marks the replaced pair.
    let w = whole_file();
    assert_eq!(header_rows(&w.diff, DiffLayout::Split), [0]);
    let marks = crate::intraline::intraline(&w.diff);
    assert!(!marks.line(0, 1).is_empty() && !marks.line(0, 2).is_empty());
}

/// A comment inserted above two edited settings, with `pad` context lines on each side, and its
/// word marks and line pairs filled in.
pub(super) fn similar_whole(pad: u32) -> WholeFile {
    let mut lines: Vec<DiffLine> = (1..=pad).map(|n| ctx(n, n)).collect();
    let (o, n) = (pad + 1, pad + 1);
    lines.extend([
        l('-', Some(o), None, "host = \"localhost\""),
        l('-', Some(o + 1), None, "port = 8080"),
        l('+', None, Some(n), "# listen on all interfaces"),
        l('+', None, Some(n + 1), "host = \"0.0.0.0\""),
        l('+', None, Some(n + 2), "port = 9090"),
    ]);
    lines.extend((0..pad.max(1)).map(|k| ctx(o + 2 + k, n + 3 + k)));
    let mut w = single(lines);
    w.diff.intraline = crate::intraline::intraline(&w.diff);
    w
}

#[test]
fn whole_split_rows_with_similarity_pairs() {
    let w = similar_whole(1);
    let none = Expanded::default();
    let pair = |old: Option<usize>, new: Option<usize>| SplitRow::Pair { hunk: 0, old, new };
    let want = [
        pair(Some(0), Some(0)),
        SplitRow::Block(0),
        pair(None, Some(3)),
        pair(Some(1), Some(4)),
        pair(Some(2), Some(5)),
        pair(Some(6), Some(6)),
    ];
    assert_eq!(split_rows(&w, &none), want);
    assert_eq!(block_rows(&w, DiffLayout::Split, &none), [1]);
    assert_eq!(block_rows(&w, DiffLayout::Inline, &none), [1]);
}
