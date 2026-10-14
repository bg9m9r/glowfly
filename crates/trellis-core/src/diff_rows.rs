//! How a FileDiff is laid out as rows: one unified column, or old and new side by side.
//!
//! The diff viewer draws one row per entry, with a fixed row height, so huge diffs stay fast.
//! Change navigation (next/previous hunk) works on the same row indices, which is why this
//! lives here rather than in the view.

use crate::repo::FileDiff;
use crate::settings::DiffLayout;

/// A row of the inline (unified) layout.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DiffRow {
    /// The header of hunk `.0`.
    Header(usize),
    /// Line `line` of hunk `hunk`.
    Line { hunk: usize, line: usize },
}

/// A row of the side-by-side layout.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SplitRow {
    /// The header of hunk `.0`, spanning both columns.
    Header(usize),
    /// Line indices into `hunks[hunk].lines` for the old (left) and new (right) side. A context
    /// line is the same index on both sides; `None` is an empty cell.
    Pair { hunk: usize, old: Option<usize>, new: Option<usize> },
}

/// One header row per hunk, then one row per line.
pub fn inline_rows(d: &FileDiff) -> Vec<DiffRow> {
    let mut rows = Vec::new();
    for (h, hunk) in d.hunks.iter().enumerate() {
        rows.push(DiffRow::Header(h));
        rows.extend((0..hunk.lines.len()).map(|line| DiffRow::Line { hunk: h, line }));
    }
    rows
}

/// Which side of the split layout a line belongs to.
enum Side {
    Old,
    New,
    Both,
}

/// libgit2 names the "no newline at end of file" markers after the side that gains or loses
/// the newline, not the side that lacks it: `>` (ADD_EOFNL) follows a removed line and belongs
/// to the old file, `<` (DEL_EOFNL) follows an added line and belongs to the new file. `=` is the
/// marker after a context line, which both files lack.
fn side(origin: char) -> Side {
    match origin {
        '-' | '>' => Side::Old,
        '+' | '<' => Side::New,
        _ => Side::Both,
    }
}

/// Old and new side by side: within each hunk, a run of removed lines followed by a run of
/// added lines is zipped row by row, and the longer run's extra rows get an empty cell opposite.
pub fn split_rows(d: &FileDiff) -> Vec<SplitRow> {
    let mut rows = Vec::new();
    for (h, hunk) in d.hunks.iter().enumerate() {
        rows.push(SplitRow::Header(h));
        let (mut dels, mut adds) = (Vec::new(), Vec::new());
        let flush = |rows: &mut Vec<SplitRow>, dels: &mut Vec<usize>, adds: &mut Vec<usize>| {
            for k in 0..dels.len().max(adds.len()) {
                rows.push(SplitRow::Pair { hunk: h, old: dels.get(k).copied(), new: adds.get(k).copied() });
            }
            dels.clear();
            adds.clear();
        };
        for (i, line) in hunk.lines.iter().enumerate() {
            match side(line.origin) {
                Side::Old => {
                    // A deletion after additions starts a new block.
                    if !adds.is_empty() {
                        flush(&mut rows, &mut dels, &mut adds);
                    }
                    dels.push(i);
                }
                Side::New => adds.push(i),
                Side::Both => {
                    flush(&mut rows, &mut dels, &mut adds);
                    rows.push(SplitRow::Pair { hunk: h, old: Some(i), new: Some(i) });
                }
            }
        }
        flush(&mut rows, &mut dels, &mut adds);
    }
    rows
}

/// The row index of each hunk header in `layout`, in hunk order.
pub fn header_rows(d: &FileDiff, layout: DiffLayout) -> Vec<usize> {
    match layout {
        DiffLayout::Inline => {
            let mut row = 0;
            d.hunks
                .iter()
                .map(|h| {
                    let at = row;
                    row += 1 + h.lines.len();
                    at
                })
                .collect()
        }
        DiffLayout::Split => split_rows(d).iter().enumerate().filter(|(_, r)| matches!(r, SplitRow::Header(_))).map(|(i, _)| i).collect(),
    }
}

/// The hunk shown at row `top`: the last header at or above it, or the first hunk.
pub fn hunk_at(headers: &[usize], top: usize) -> usize {
    headers.iter().rposition(|&h| h <= top).unwrap_or(0)
}

/// The header row to scroll to for the next (`step > 0`) or previous (`step < 0`) change from
/// the top row `top`. `None` at either end, or for a zero step; nothing wraps.
pub fn step_change(headers: &[usize], top: usize, step: isize) -> Option<usize> {
    match step.signum() {
        1 => headers.iter().copied().find(|&h| h > top),
        -1 => headers.iter().copied().rev().find(|&h| h < top),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::{DiffLine, Hunk};

    fn hunk(lines: &[(char, Option<u32>, Option<u32>, &str)]) -> Hunk {
        Hunk {
            header: "@@ -1 +1 @@".into(),
            lines: lines.iter().map(|&(origin, old, new, text)| DiffLine { origin, old, new, text: text.into() }).collect(),
        }
    }

    fn diff(hunks: Vec<Hunk>) -> FileDiff {
        FileDiff { binary: false, hunks, highlight: None }
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
}
