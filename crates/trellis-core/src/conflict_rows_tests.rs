use super::*;

fn lines(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| format!("{s}\n")).collect()
}

fn sample() -> Vec<Segment> {
    vec![
        Segment::Clean(lines(&["a", "b"])),
        Segment::Conflict(Chunk { base: lines(&["c"]), ours: lines(&["o1", "o2", "o3"]), theirs: lines(&["t1"]) }),
        Segment::Clean(lines(&["d"])),
    ]
}

fn none() -> BTreeSet<usize> {
    BTreeSet::new()
}

/// 10 clean lines, a chunk (ours 2 lines, theirs 1), 12 clean lines, a chunk, 2 clean lines.
fn long() -> Vec<Segment> {
    let run = |n: usize, p: &str| Segment::Clean((0..n).map(|i| format!("{p}{i}\n")).collect());
    vec![
        run(10, "a"),
        Segment::Conflict(Chunk { base: Vec::new(), ours: lines(&["o1", "o2"]), theirs: lines(&["t1"]) }),
        run(12, "b"),
        Segment::Conflict(Chunk { base: Vec::new(), ours: lines(&["x"]), theirs: lines(&["y"]) }),
        run(2, "c"),
    ]
}

#[test]
fn pane_rows_fold_long_clean_runs() {
    let segs = long();
    let (rows, firsts) = pane_rows(&segs, &none());
    let fold = |seg, start, end| PaneRow::Fold(CleanFold { seg, start, end });
    // Lines 0..7 of the first run fold; 3 stay before the chunk.
    assert_eq!(rows[0], fold(0, 0, 7));
    assert_eq!(rows[1], PaneRow::Clean { seg: 0, line: 7, ours_no: 8, theirs_no: 8 });
    assert_eq!(firsts, [4, 14]);
    // After the chunk: 3 lines, the fold of 6, 3 lines; each side keeps its own count.
    assert_eq!(rows[7], PaneRow::Clean { seg: 2, line: 0, ours_no: 13, theirs_no: 12 });
    assert_eq!(rows[10], fold(2, 3, 9));
    assert_eq!(rows[11], PaneRow::Clean { seg: 2, line: 9, ours_no: 22, theirs_no: 21 });
    assert_eq!(rows.len(), 18, "the short last run doesn't fold");
    // Opened folds show every line.
    let opened = BTreeSet::from([0, 2]);
    let (rows, firsts) = pane_rows(&segs, &opened);
    assert!(rows.iter().all(|r| !matches!(r, PaneRow::Fold(_))));
    assert_eq!((rows.len(), firsts), (10 + 3 + 12 + 2 + 2, vec![10, 25]));
}

#[test]
fn output_rows_fold_unedited_runs_only() {
    let segs = long();
    let picks = [ChunkPick::Sides(Choice::Ours), ChunkPick::default()];
    let (rows, firsts) = output_rows(&segs, &picks, &CleanEdits::new(), &none());
    assert_eq!(rows[0], OutRow::Fold(CleanFold { seg: 0, start: 0, end: 7 }));
    assert_eq!(firsts, [4, 13]);
    assert_eq!(rows[4], OutRow::Taken { chunk: 0, side: Side::Ours, line: 0 });
    assert_eq!(rows[13], OutRow::Unresolved(1));
    // An edited run never folds; its typed lines take its place.
    let mut cleans = CleanEdits::new();
    cleans.insert(2, lines(&["only"]));
    let (rows, firsts) = output_rows(&segs, &picks, &cleans, &none());
    assert_eq!(rows[6], OutRow::CleanEdited { seg: 2, line: 0 });
    assert_eq!(firsts, [4, 7]);
    assert_eq!(rows[firsts[1]], OutRow::Unresolved(1));
    // Opened, the first run shows every line.
    let (rows, _) = output_rows(&segs, &picks, &cleans, &BTreeSet::from([0]));
    assert_eq!(rows[..10].iter().filter(|r| matches!(r, OutRow::Clean { seg: 0, .. })).count(), 10);
}

#[test]
fn pane_rows_pad_the_shorter_side() {
    let (rows, firsts) = pane_rows(&sample(), &none());
    assert_eq!(firsts, [2]);
    assert_eq!(rows[2], PaneRow::Header(0));
    assert_eq!(rows[3], PaneRow::Lines { chunk: 0, row: 0, ours: Some(3), theirs: Some(3) });
    assert_eq!(rows[4], PaneRow::Lines { chunk: 0, row: 1, ours: Some(4), theirs: None });
    assert_eq!(rows[5], PaneRow::Lines { chunk: 0, row: 2, ours: Some(5), theirs: None });
    // After the chunk each side continues from its own count.
    assert_eq!(rows[6], PaneRow::Clean { seg: 2, line: 0, ours_no: 6, theirs_no: 4 });
    assert_eq!(rows.len(), 7);
    // A chunk with nothing on either side still gets a row.
    let empty = [Segment::Conflict(Chunk { base: lines(&["x"]), ours: Vec::new(), theirs: Vec::new() })];
    let (rows, _) = pane_rows(&empty, &none());
    assert_eq!(rows, [PaneRow::Header(0), PaneRow::Lines { chunk: 0, row: 0, ours: None, theirs: None }]);
}

#[test]
fn output_rows_show_placeholders() {
    let segs = sample();
    let (rows, firsts) = output_rows(&segs, &[ChunkPick::default()], &CleanEdits::new(), &none());
    assert_eq!(rows.len(), 4);
    assert_eq!((rows[2], firsts[0]), (OutRow::Unresolved(0), 2));
    let (rows, _) = output_rows(&segs, &[Choice::OursThenTheirs.into()], &CleanEdits::new(), &none());
    let taken: Vec<_> = rows
        .iter()
        .filter_map(|r| match r {
            OutRow::Taken { side, line, .. } => Some((*side, *line)),
            _ => None,
        })
        .collect();
    assert_eq!(taken, [(Side::Ours, 0), (Side::Ours, 1), (Side::Ours, 2), (Side::Theirs, 0)]);
    assert_eq!(rows.last(), Some(&OutRow::Clean { seg: 2, line: 0 }));
    // A picked side with no lines leaves no rows; the chunk starts at the next row.
    let empty = [Segment::Conflict(Chunk { base: Vec::new(), ours: Vec::new(), theirs: lines(&["t"]) }), Segment::Clean(lines(&["z"]))];
    let (rows, firsts) = output_rows(&empty, &[Choice::Ours.into()], &CleanEdits::new(), &none());
    assert_eq!((rows, firsts), (vec![OutRow::Clean { seg: 1, line: 0 }], vec![0]));
    // Single lines in tick order; typed text line by line.
    let lines_pick = ChunkPick::Lines(vec![crate::conflict_pick::LineRef { side: Side::Theirs, line: 0 }]);
    let (rows, _) = output_rows(&segs, &[lines_pick], &CleanEdits::new(), &none());
    assert_eq!(rows[2], OutRow::Taken { chunk: 0, side: Side::Theirs, line: 0 });
    let typed = ChunkPick::Edited { lines: lines(&["x", "y"]), before: Box::default() };
    let (rows, _) = output_rows(&segs, &[typed], &CleanEdits::new(), &none());
    assert_eq!(rows[2..4], [OutRow::Edited { chunk: 0, line: 0 }, OutRow::Edited { chunk: 0, line: 1 }]);
}

#[test]
fn scroll_target_clamps_at_zero() {
    assert_eq!(scroll_target(&[1, 10], 0), 0);
    assert_eq!(scroll_target(&[1, 10], 1), 7);
    assert_eq!(scroll_target(&[], 3), 0);
}

fn chunk(base: &[&str], ours: &[&str], theirs: &[&str]) -> Chunk {
    Chunk { base: lines(base), ours: lines(ours), theirs: lines(theirs) }
}

#[test]
fn trivial_pick_table() {
    use Choice::{Ours, Theirs};
    use Trivial::{Identical, Unchanged, Whitespace};
    let crlf = |v: &[&str]| v.iter().map(|s| format!("{s}\r\n")).collect::<Vec<_>>();
    type Row = (&'static str, Chunk, Option<(Choice, Trivial)>);
    let table: [Row; 9] = [
        ("ours is the base", chunk(&["a"], &["a"], &["b"]), Some((Theirs, Unchanged))),
        ("theirs is the base", chunk(&["a"], &["b"], &["a"]), Some((Ours, Unchanged))),
        ("the same change", chunk(&["a"], &["b", "c"], &["b", "c"]), Some((Ours, Identical))),
        ("indentation", chunk(&["a"], &["    b(x)"], &["\tb(x)"]), Some((Ours, Whitespace))),
        ("trailing spaces", chunk(&["a"], &["b  c"], &["b c  "]), Some((Ours, Whitespace))),
        ("a blank line added", chunk(&["a"], &["b", "", "c"], &["b", "c"]), Some((Ours, Whitespace))),
        ("ours only reindents", chunk(&["  a"], &["a"], &["z"]), Some((Theirs, Whitespace))),
        ("theirs only reindents", chunk(&["  a"], &["z"], &["a "]), Some((Ours, Whitespace))),
        ("a real conflict", chunk(&["a"], &["b"], &["c"]), None),
    ];
    for (what, c, want) in table {
        assert_eq!(trivial_pick(&c, true), want, "{what}");
    }
    // Line endings alone: CR LF against LF.
    let c = Chunk { base: lines(&["a"]), ours: crlf(&["b"]), theirs: lines(&["b"]) };
    assert_eq!(trivial_pick(&c, true), Some((Ours, Whitespace)));
    // Line endings are ignored against the base too.
    let c = Chunk { base: crlf(&["a"]), ours: lines(&["a"]), theirs: lines(&["b"]) };
    assert_eq!(trivial_pick(&c, true), Some((Theirs, Unchanged)));
    // Without a base, an empty side isn't "unchanged": the empty base is made up.
    let c = chunk(&[], &[], &["t"]);
    assert_eq!(trivial_pick(&c, false), None);
    assert_eq!(trivial_pick(&c, true), Some((Theirs, Unchanged)));
    // Equal sides are still equal without a base.
    assert_eq!(trivial_pick(&chunk(&[], &["x"], &["x"]), false), Some((Ours, Identical)));
    assert_eq!(Whitespace.label(), "whitespace only");
}
