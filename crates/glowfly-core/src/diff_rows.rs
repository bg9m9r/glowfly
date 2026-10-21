//! How a FileDiff is laid out as rows: one unified column, or old and new side by side.
//!
//! The diff viewer draws one row per entry, with a fixed row height, so huge diffs stay fast.
//! Change navigation (next/previous hunk) works on the same row indices, which is why this
//! lives here rather than in the view.

use crate::line_pairs::LinePairs;
use crate::repo::{DiffLine, FileDiff};
use crate::settings::DiffLayout;
use std::ops::Range;

mod gaps;
mod line_row;
pub use gaps::*;
pub use line_row::*;

/// A row of the inline (unified) layout.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DiffRow {
    /// The header of hunk `.0`.
    Header(usize),
    /// Line `line` of hunk `hunk`.
    Line { hunk: usize, line: usize },
    /// The action row of the whole-file change block for normal hunk `.0` (whole-file view only).
    Block(usize),
    /// An "N unchanged lines" row standing for the fold's lines (whole-file view only).
    Fold(crate::whole_file::Fold),
    /// Line `line` of the unchanged run before hunk `hunk`, filled in (hunk view only).
    GapLine { hunk: usize, line: usize },
}

/// A row of the side-by-side layout.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SplitRow {
    /// The header of hunk `.0`, spanning both columns.
    Header(usize),
    /// Line indices into `hunks[hunk].lines` for the old (left) and new (right) side. A context
    /// line is the same index on both sides; `None` is an empty cell.
    Pair { hunk: usize, old: Option<usize>, new: Option<usize> },
    /// The action row of the whole-file change block for normal hunk `.0` (whole-file view only).
    Block(usize),
    /// An "N unchanged lines" row standing for the fold's lines (whole-file view only).
    Fold(crate::whole_file::Fold),
    /// Line `line` of the unchanged run before hunk `hunk`, filled in (hunk view only), on both
    /// sides.
    GapLine { hunk: usize, line: usize },
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

/// The change runs of a hunk's lines: each maximal stretch of old-side lines (`-`, `>`)
/// followed by new-side lines (`+`, `<`), as the old and new index ranges (`old.end ==
/// new.start`). A deletion after additions starts a new run.
pub(crate) fn change_runs(lines: &[DiffLine]) -> Vec<(Range<usize>, Range<usize>)> {
    let mut runs = Vec::new();
    // The open run's first line and the first line of its new side (its end while it has none).
    let mut open: Option<(usize, usize)> = None;
    for (i, line) in lines.iter().enumerate() {
        match (side(line.origin), open) {
            (Side::Old, Some((start, mid))) if mid < i => {
                runs.push((start..mid, mid..i));
                open = Some((i, i + 1));
            }
            (Side::Old, Some((start, _))) => open = Some((start, i + 1)),
            (Side::Old, None) => open = Some((i, i + 1)),
            (Side::New, None) => open = Some((i, i)),
            (Side::New, Some(_)) | (Side::Both, None) => {}
            (Side::Both, Some((start, mid))) => {
                runs.push((start..mid, mid..i));
                open = None;
            }
        }
    }
    if let Some((start, mid)) = open {
        runs.push((start..mid, mid..lines.len()));
    }
    runs
}

/// Old and new side by side. In each change run (removed lines followed by added lines) the
/// lines paired by similarity in the diff's `intraline` share rows, and the lines between them
/// share rows by position, the longer side's extra lines getting an empty cell opposite. A diff
/// that was never decorated, or whose pairs don't fit it, pairs by position.
pub fn split_rows(d: &FileDiff) -> Vec<SplitRow> {
    split_rows_with(d, d.intraline.pairs())
}

/// `split_rows` with the anchors `pairs`, which are ignored when they don't fit `d`.
pub fn split_rows_with(d: &FileDiff, pairs: &LinePairs) -> Vec<SplitRow> {
    let fits = pairs.fits(d);
    let mut rows = Vec::new();
    for (h, hunk) in d.hunks.iter().enumerate() {
        rows.push(SplitRow::Header(h));
        let anchors = if fits { pairs.hunk(h) } else { &[] };
        let mut next = anchors.iter().map(|&(o, n)| (o as usize, n as usize)).peekable();
        let zip = |rows: &mut Vec<SplitRow>, old: Range<usize>, new: Range<usize>| {
            for k in 0..old.len().max(new.len()) {
                let (o, n) = (old.start + k, new.start + k);
                rows.push(SplitRow::Pair { hunk: h, old: (o < old.end).then_some(o), new: (n < new.end).then_some(n) });
            }
        };
        let mut line = 0;
        for (old, new) in change_runs(&hunk.lines) {
            rows.extend((line..old.start).map(|i| SplitRow::Pair { hunk: h, old: Some(i), new: Some(i) }));
            line = new.end;
            let (mut di, mut ai) = (old.start, new.start);
            while let Some((o, n)) = next.next_if(|&(o, _)| o < old.end) {
                if o < di || n < ai || n >= new.end {
                    continue;
                }
                zip(&mut rows, di..o, ai..n);
                rows.push(SplitRow::Pair { hunk: h, old: Some(o), new: Some(n) });
                (di, ai) = (o + 1, n + 1);
            }
            zip(&mut rows, di..old.end, ai..new.end);
        }
        rows.extend((line..hunk.lines.len()).map(|i| SplitRow::Pair { hunk: h, old: Some(i), new: Some(i) }));
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
mod tests;
