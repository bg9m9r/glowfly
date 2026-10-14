//! Rows of the whole-file view mapped to the file lines they show and back, so a change to the
//! open folds, which adds or removes rows, can keep the same line at the top of the viewport.

use super::{Expanded, WholeFile, inline_rows, split_rows};
use crate::diff_rows::{DiffRow, SplitRow};
use crate::settings::DiffLayout;

/// Each row's first line (an index into the whole-file hunk) and, for a fold row, the end of
/// its fold, in `layout` with the folds in `ex` open. A header row (never in whole-file rows)
/// has no line.
fn starts(w: &WholeFile, layout: DiffLayout, ex: &Expanded) -> Vec<(Option<usize>, Option<usize>)> {
    let block = |h: usize| w.blocks.iter().find(|b| b.hunk == h).map(|b| b.start);
    match layout {
        DiffLayout::Inline => inline_rows(w, ex)
            .into_iter()
            .map(|r| match r {
                DiffRow::Line { line, .. } => (Some(line), None),
                DiffRow::Fold(f) => (Some(f.start), Some(f.end)),
                DiffRow::Block(h) => (block(h), None),
                DiffRow::Header(_) => (None, None),
            })
            .collect(),
        DiffLayout::Split => split_rows(w, ex)
            .into_iter()
            .map(|r| match r {
                SplitRow::Pair { old, new, .. } => (old.into_iter().chain(new).min(), None),
                SplitRow::Fold(f) => (Some(f.start), Some(f.end)),
                SplitRow::Block(h) => (block(h), None),
                SplitRow::Header(_) => (None, None),
            })
            .collect(),
    }
}

/// The first whole-file line that row `row` shows in `layout`, with the folds in `ex` open: a
/// fold row's first hidden line, a block's action row the block's first line, a split pair the
/// earlier of its two lines. `None` past the last row.
pub fn line_at_row(w: &WholeFile, layout: DiffLayout, ex: &Expanded, row: usize) -> Option<usize> {
    starts(w, layout, ex).get(row).and_then(|&(line, _)| line)
}

/// The row showing whole-file line `line` in `layout`, with the folds in `ex` open: the fold row
/// hiding it, else the first row starting at or after it. A block's action row comes before its
/// first line's row, so the action row stays with its change. Past the last line, the last row
/// (0 with no rows).
pub fn row_of_line(w: &WholeFile, layout: DiffLayout, ex: &Expanded, line: usize) -> usize {
    let rows = starts(w, layout, ex);
    let at = rows.iter().position(|&(start, end)| match (start, end) {
        (Some(s), Some(e)) => (s..e).contains(&line) || s > line,
        (Some(s), None) => s >= line,
        (None, _) => false,
    });
    at.unwrap_or(rows.len().saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::super::tests::whole_file;
    use super::super::{Expanded, block_rows, folds, inline_rows, split_rows};
    use super::*;

    const LAYOUTS: [DiffLayout; 2] = [DiffLayout::Inline, DiffLayout::Split];

    fn row_count(w: &WholeFile, layout: DiffLayout, ex: &Expanded) -> usize {
        match layout {
            DiffLayout::Inline => inline_rows(w, ex).len(),
            DiffLayout::Split => split_rows(w, ex).len(),
        }
    }

    #[test]
    fn line_at_row_and_row_of_line_round_trip() {
        let w = whole_file();
        for layout in LAYOUTS {
            for ex in [Expanded::default(), Expanded::all(&w)] {
                let n = row_count(&w, layout, &ex);
                assert!(n > 0);
                let mut last = 0;
                for r in 0..n {
                    let line = line_at_row(&w, layout, &ex, r).expect("every whole-file row shows a line");
                    assert!(line >= last, "{layout:?}: starts never go back (row {r})");
                    if r == 0 || line > last {
                        assert_eq!(row_of_line(&w, layout, &ex, line), r, "{layout:?} row {r}");
                    }
                    last = line;
                }
                assert_eq!(line_at_row(&w, layout, &ex, n), None, "{layout:?}: past the end");
            }
        }
    }

    #[test]
    fn anchors_land_on_folds_and_blocks() {
        let w = whole_file();
        let f = folds(&w)[0];
        let none = Expanded::default();
        for layout in LAYOUTS {
            // A line inside a closed fold is shown by the fold row.
            let fold_row = row_of_line(&w, layout, &none, f.start);
            for line in f.start..f.end {
                assert_eq!(row_of_line(&w, layout, &none, line), fold_row, "{layout:?} line {line}");
            }
            assert_eq!(line_at_row(&w, layout, &none, fold_row), Some(f.start));
            // A block's first line maps to its action row, not to the line row after it.
            for ex in [Expanded::default(), Expanded::all(&w)] {
                let blocks = block_rows(&w, layout, &ex);
                for (b, &row) in w.blocks.iter().zip(&blocks) {
                    assert_eq!(row_of_line(&w, layout, &ex, b.start), row, "{layout:?}");
                    assert_eq!(line_at_row(&w, layout, &ex, row + 1), Some(b.start), "{layout:?}: the block's first line follows");
                }
                // Past the last line: the last row.
                let n = row_count(&w, layout, &ex);
                assert_eq!(row_of_line(&w, layout, &ex, 10_000), n - 1, "{layout:?}");
            }
        }
        let empty = WholeFile { diff: super::super::tests::diff(Vec::new()), blocks: Vec::new() };
        assert_eq!(row_of_line(&empty, DiffLayout::Inline, &none, 3), 0);
        assert_eq!(line_at_row(&empty, DiffLayout::Split, &none, 0), None);
    }
}
