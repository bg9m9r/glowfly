use super::tests::{EOFNL, ctx, diff, hunk, l, single, two_changes, whole_file};
use super::*;

/// A removed first line, `gap` context lines, and an added last line, as two normal hunks.
fn two_blocks(gap: u32) -> WholeFile {
    let del = l('-', Some(1), None, "gone");
    let add = l('+', None, Some(gap + 1), "new");
    let mut lines = vec![del.clone()];
    lines.extend((0..gap).map(|k| ctx(k + 2, k + 1)));
    lines.push(add.clone());
    WholeFile::new(&[hunk(vec![del]), hunk(vec![add])], hunk(lines)).expect("maps")
}

fn line(line: usize) -> DiffRow {
    DiffRow::Line { hunk: 0, line }
}

#[test]
fn folds_keep_context_around_blocks() {
    let w = whole_file();
    assert_eq!(w.blocks.iter().map(|b| (b.start, b.end)).collect::<Vec<_>>(), [(1, 3), (13, 14)]);
    // Lines 3..13 are context: 3 stay above, 3 below, the 4 between fold. The one-line runs at
    // either end of the file don't.
    assert_eq!(folds(&w), [Fold { start: 6, end: 10, key: 6 }]);
    assert_eq!(folds(&w)[0].len(), MIN_FOLD);
}

#[test]
fn fold_threshold_boundaries() {
    assert!(folds(&two_blocks(9)).is_empty(), "9 lines between blocks stay shown");
    let f = folds(&two_blocks(10));
    assert_eq!(f, [Fold { start: 1 + FOLD_CONTEXT, end: 1 + 10 - FOLD_CONTEXT, key: 4 }]);
    assert_eq!(f[0].len(), MIN_FOLD);

    // A leading run keeps context only below it.
    let leading = |run: u32| {
        let mut lines: Vec<DiffLine> = (1..=run).map(|n| ctx(n, n)).collect();
        lines.push(l('+', None, Some(run + 1), "new"));
        single(lines)
    };
    assert!(folds(&leading(6)).is_empty());
    assert_eq!(folds(&leading(7)), [Fold { start: 0, end: 4, key: 1 }]);

    // A trailing run up to the end of the file keeps context only above it.
    let mut lines = vec![l('-', Some(1), None, "gone")];
    lines.extend((2..=8).map(|n| ctx(n, n - 1)));
    let n = lines.len();
    let trailing = single(lines);
    let end = trailing.blocks[0].end;
    assert_eq!(folds(&trailing), [Fold { start: end + FOLD_CONTEXT, end: n, key: 4 }]);
}

#[test]
fn trailing_no_newline_marker_stays_visible() {
    let mut lines = vec![l('+', None, Some(1), "new")];
    lines.extend((1..=10).map(|n| ctx(n, n + 1)));
    lines.push(l('=', None, None, EOFNL));
    let w = single(lines);
    let marker = 11;
    assert_eq!(folds(&w), [Fold { start: 4, end: marker - FOLD_CONTEXT, key: 5 }]);
    let rows = inline_rows(&w, &Expanded::default());
    assert_eq!(&rows[rows.len() - 4..], [line(8), line(9), line(10), line(marker)]);
    assert_eq!(rows[..6], [DiffRow::Block(0), line(0), line(1), line(2), line(3), DiffRow::Fold(folds(&w)[0])]);
}

#[test]
fn context_inside_a_block_never_folds() {
    let mut lines = vec![ctx(1, 1), l('-', Some(2), None, "a")];
    lines.extend((3..=14).map(|n| ctx(n, n - 1)));
    lines.push(l('+', None, Some(14), "b"));
    lines.push(ctx(15, 15));
    let w = single(lines);
    assert_eq!(w.blocks.len(), 1);
    assert!(folds(&w).is_empty());
}

#[test]
fn no_blocks_no_folds_and_keys() {
    let lines: Vec<DiffLine> = (1..=30).map(|n| ctx(n, n)).collect();
    let unchanged = WholeFile { diff: diff(vec![hunk(lines)]), blocks: Vec::new() };
    assert!(folds(&unchanged).is_empty());
    let empty = WholeFile { diff: diff(Vec::new()), blocks: vec![ChangeBlock { hunk: 0, start: 0, end: 1 }] };
    assert!(folds(&empty).is_empty());

    // The key is the new-side number of the first hidden line.
    let mut lines = vec![ctx(1, 1), ctx(2, 2)];
    lines.extend((3..=5).map(|n| l('+', None, Some(n), "added")));
    lines.extend((3..=13).map(|n| ctx(n, n + 3)));
    let mut w = single(lines);
    let f = folds(&w);
    assert_eq!(f, [Fold { start: 8, end: 16, key: 9 }]);
    assert_eq!(w.diff.hunks[0].lines[8], ctx(6, 9));
    // Without a new number, the old one; without either, the index.
    w.diff.hunks[0].lines[8].new = None;
    assert_eq!(folds(&w)[0].key, 6);
    w.diff.hunks[0].lines[8].old = None;
    assert_eq!(folds(&w)[0].key, 8);
}

#[test]
fn collapsed_inline_rows() {
    let w = whole_file();
    let none = Expanded::default();
    let f = Fold { start: 6, end: 10, key: 6 };
    let want = [
        line(0),
        DiffRow::Block(0),
        line(1),
        line(2),
        line(3),
        line(4),
        line(5),
        DiffRow::Fold(f),
        line(10),
        line(11),
        line(12),
        DiffRow::Block(1),
        line(13),
        line(14),
    ];
    assert_eq!(inline_rows(&w, &none), want);
    assert_eq!(block_rows(&w, DiffLayout::Inline, &none), [1, 11]);

    let mut ex = Expanded::default();
    assert!(ex.insert(6));
    let open = inline_rows(&w, &ex);
    assert_eq!(open, inline_rows(&w, &Expanded::all(&w)));
    assert_eq!(open.len(), w.diff.hunks[0].lines.len() + w.blocks.len());
    assert!(!open.iter().any(|r| matches!(r, DiffRow::Fold(_))));
}

#[test]
fn collapsed_split_rows() {
    let w = whole_file();
    let none = Expanded::default();
    let rows = split_rows(&w, &none);
    let all = split_rows(&w, &Expanded::all(&w));
    let pair = |i: usize| SplitRow::Pair { hunk: 0, old: Some(i), new: Some(i) };
    let at = all.iter().position(|r| *r == pair(6)).expect("context pair");
    let folds_at: Vec<usize> = rows.iter().enumerate().filter(|(_, r)| matches!(r, SplitRow::Fold(_))).map(|(i, _)| i).collect();
    assert_eq!(folds_at, [at]);
    assert_eq!(rows[at], SplitRow::Fold(Fold { start: 6, end: 10, key: 6 }));
    assert!((6..10).all(|i| !rows.contains(&pair(i))), "the fold's pairs are gone");
    assert_eq!(rows[at + 1], pair(10));
    assert_eq!(rows.len(), all.len() - 3);
    let blocks: Vec<usize> = rows.iter().enumerate().filter(|(_, r)| matches!(r, SplitRow::Block(_))).map(|(i, _)| i).collect();
    assert_eq!(block_rows(&w, DiffLayout::Split, &none), blocks);

    // Pure add and pure delete blocks have no folds to take.
    let added = single(vec![l('+', None, Some(1), "a"), l('+', None, Some(2), "b")]);
    assert_eq!(split_rows(&added, &none), split_rows(&added, &Expanded::all(&added)));
    assert_eq!(split_rows(&added, &none).len(), 3);
    let deleted = single(vec![ctx(1, 1), l('-', Some(2), None, "a")]);
    assert_eq!(split_rows(&deleted, &none), split_rows(&deleted, &Expanded::all(&deleted)));
}

#[test]
fn change_rows_count_folds() {
    let (hunks, whole) = two_changes();
    let mut d = diff(hunks);
    d.whole = WholeFile::new(&d.hunks, whole).map(Box::new);
    let w = d.whole.as_deref().unwrap();
    let (none, all) = (Expanded::default(), Expanded::all(w));
    for layout in [DiffLayout::Inline, DiffLayout::Split] {
        let folded = change_rows(&d, DiffMode::WholeFile, layout, &none);
        assert_eq!(folded, block_rows(w, layout, &none));
        let open = change_rows(&d, DiffMode::WholeFile, layout, &all);
        assert_eq!(folded[0], open[0], "{layout:?}: the block above the fold keeps its row");
        assert_eq!(folded[1] + 3, open[1], "{layout:?}: the block after the fold moves up");
        assert_eq!(change_rows(&d, DiffMode::Hunks, layout, &none), change_rows(&d, DiffMode::Hunks, layout, &all));
    }
    for mode in [DiffMode::Hunks, DiffMode::WholeFile] {
        assert_eq!(hunk_of_change(&d, mode, 1), Some(1));
        assert_eq!(change_of_hunk(&d, mode, 0), Some(0));
    }
}

#[test]
fn expanded_set() {
    let w = whole_file();
    let mut ex = Expanded::default();
    assert!(ex.is_empty());
    assert!(ex.insert(6));
    assert!(!ex.insert(6), "already open");
    assert!(ex.contains(6) && !ex.contains(7) && !ex.is_empty());
    ex.clear();
    assert!(ex.is_empty());
    assert!(collapsed(&w, &Expanded::all(&w)).is_empty());
    assert_eq!(collapsed(&w, &Expanded::default()), folds(&w));
}

#[test]
fn expanded_remove_reports_whether_open() {
    let mut ex = Expanded::default();
    assert!(ex.insert(6));
    assert!(ex.remove(6), "it was open");
    assert!(!ex.contains(6));
    assert!(!ex.remove(6), "already closed");
}

#[test]
fn opened_lists_open_folds_in_order() {
    // Three runs of 10 context lines between and around two changes: three folds.
    let (del, add) = (l('-', Some(11), None, "gone"), l('+', None, Some(21), "new"));
    let mut lines: Vec<DiffLine> = (1..=10).map(|n| ctx(n, n)).collect();
    lines.push(del.clone());
    lines.extend((12..=21).map(|n| ctx(n, n - 1)));
    lines.push(add.clone());
    lines.extend((22..=31).map(|n| ctx(n, n)));
    let w = WholeFile::new(&[hunk(vec![del]), hunk(vec![add])], hunk(lines)).expect("maps");
    let all = folds(&w);
    assert_eq!(all.len(), 3);
    let mut ex = Expanded::default();
    for key in [all[2].key, all[0].key, 999] {
        ex.insert(key);
    }
    assert_eq!(opened(&w, &ex), [all[0], all[2]], "in file order, without the stale key");
    assert!(opened(&w, &Expanded::default()).is_empty());
    assert_eq!(opened(&w, &Expanded::all(&w)), all);
}
