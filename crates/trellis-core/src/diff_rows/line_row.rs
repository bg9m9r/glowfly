//! Where a line of the old or new file sits in a FileDiff and in its rows, so a comment made on
//! a file line (a pull request's review thread) can be marked on, and scrolled to, its row.

use super::{DiffRow, SplitRow, header_rows, split_rows};
use crate::repo::{DiffLine, FileDiff};
use crate::settings::{DiffLayout, DiffMode};
use crate::whole_file::{self, Expanded};

/// Which version of the file a line number counts in: the old (removed lines) or the new one.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LineSide {
    Old,
    New,
}

/// The file lines a diff line shows: a removed line its old number, an added line its new
/// number, a context line both. The no-newline markers show none.
pub fn line_keys(dl: &DiffLine) -> [Option<(LineSide, u32)>; 2] {
    match dl.origin {
        '-' => [dl.old.map(|n| (LineSide::Old, n)), None],
        '+' => [dl.new.map(|n| (LineSide::New, n)), None],
        ' ' => [dl.new.map(|n| (LineSide::New, n)), dl.old.map(|n| (LineSide::Old, n))],
        _ => [None, None],
    }
}

/// The first (hunk, line) index in `d` showing line `n` of the `side` file.
pub fn line_index(d: &FileDiff, side: LineSide, n: u32) -> Option<(usize, usize)> {
    d.hunks
        .iter()
        .enumerate()
        .find_map(|(h, hunk)| hunk.lines.iter().position(|dl| line_keys(dl).contains(&Some((side, n)))).map(|i| (h, i)))
}

/// The split row of `rows` showing line index `line` of hunk `hunk` on `side`'s cell.
fn split_row(rows: &[SplitRow], hunk: usize, side: LineSide, line: usize) -> Option<usize> {
    rows.iter().position(|r| match *r {
        SplitRow::Pair { hunk: h, old, new } => h == hunk && if side == LineSide::Old { old == Some(line) } else { new == Some(line) },
        _ => false,
    })
}

/// The row that shows line `n` of the `side` file in the view `mode` and `layout` draw, with
/// the whole-file folds in `ex` open: in the whole-file view, the line's own row or the fold
/// row hiding it. `None` when the view doesn't show the line (outside the hunks).
pub fn row_of_file_line(d: &FileDiff, mode: DiffMode, layout: DiffLayout, ex: &Expanded, side: LineSide, n: u32) -> Option<usize> {
    if let Some(w) = whole_file::shown(d, mode) {
        let (_, line) = line_index(&w.diff, side, n)?;
        // The line's own row, not a change block's action row starting at it; similarity
        // pairing can also put it after rows starting at later lines, so the split row is found
        // by its cell. A folded line has no row of its own and gets the fold row.
        let exact = match layout {
            DiffLayout::Inline => whole_file::inline_rows(w, ex).iter().position(|r| *r == DiffRow::Line { hunk: 0, line }),
            DiffLayout::Split => split_row(&whole_file::split_rows(w, ex), 0, side, line),
        };
        return Some(exact.unwrap_or_else(|| whole_file::row_of_line(w, layout, ex, line)));
    }
    let (h, line) = line_index(d, side, n)?;
    match layout {
        DiffLayout::Inline => Some(header_rows(d, DiffLayout::Inline)[h] + 1 + line),
        // A context line has the same index on both sides, so it is found by either.
        DiffLayout::Split => split_row(&split_rows(d), h, side, line),
    }
}

#[cfg(test)]
mod tests;
