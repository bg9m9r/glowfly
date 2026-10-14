//! The whole-file diff view: every line of the file in one hunk, with each normal hunk's
//! changed region (its change block) found in it, and the rows the viewer draws from them.
//!
//! The normal hunks stay the ones staging works on, so each block keeps its hunk's index and
//! the action row above it acts on that hunk.

use crate::diff_rows::{self, DiffRow, SplitRow, header_rows};
use crate::repo::{DiffLine, FileDiff, Hunk};
use crate::settings::{DiffLayout, DiffMode};
use std::collections::HashMap;

/// The changed region of normal hunk `hunk`, as line indexes `start..end` into the whole-file
/// hunk: from its first `+`/`-` line to its last, plus any `<`/`>` no-newline markers right
/// after it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChangeBlock {
    pub hunk: usize,
    pub start: usize,
    pub end: usize,
}

/// A file loaded with unlimited context. `diff` holds exactly one hunk, the whole file. Its
/// `highlight` and `intraline` are filled in by the runner like any FileDiff, so
/// `highlight_diff`, `intraline` and `diff_rows::split_rows` work on it unchanged.
#[derive(Clone, PartialEq, Debug)]
pub struct WholeFile {
    pub diff: FileDiff,
    pub blocks: Vec<ChangeBlock>,
}

impl WholeFile {
    /// `whole` with the change block of each of `hunks`, or `None` when they can't all be found.
    pub fn new(hunks: &[Hunk], whole: Hunk) -> Option<WholeFile> {
        let blocks = change_blocks(hunks, &whole.lines)?;
        let diff = FileDiff { binary: false, hunks: vec![whole], highlight: None, intraline: Default::default(), whole: None };
        Some(WholeFile { diff, blocks })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Side {
    Old,
    New,
}

/// A changed line's identity in both loads: removed lines by their old number, added lines by
/// their new number. Positions can't be used because the normal hunks skip unchanged lines.
fn key(l: &DiffLine) -> Option<(Side, u32)> {
    match l.origin {
        '-' => l.old.map(|n| (Side::Old, n)),
        '+' => l.new.map(|n| (Side::New, n)),
        _ => None,
    }
}

/// The change block of each normal hunk in the whole-file hunk's `whole` lines. `None` if a
/// changed line can't be found, or blocks come out of order or overlapping, so the caller falls
/// back to hunk rows rather than show buttons on the wrong lines.
pub fn change_blocks(hunks: &[Hunk], whole: &[DiffLine]) -> Option<Vec<ChangeBlock>> {
    let index: HashMap<(Side, u32), usize> = whole.iter().enumerate().filter_map(|(i, l)| Some((key(l)?, i))).collect();
    let mut blocks: Vec<ChangeBlock> = Vec::with_capacity(hunks.len());
    for (h, hunk) in hunks.iter().enumerate() {
        let mut changed = hunk.lines.iter().filter(|l| matches!(l.origin, '+' | '-'));
        let Some(first) = changed.next() else { continue };
        let last = changed.next_back().unwrap_or(first);
        let start = *index.get(&key(first)?)?;
        let mut end = *index.get(&key(last)?)? + 1;
        while whole.get(end).is_some_and(|l| matches!(l.origin, '<' | '>')) {
            end += 1;
        }
        if end <= start || blocks.last().is_some_and(|b| b.end > start) {
            return None;
        }
        blocks.push(ChangeBlock { hunk: h, start, end });
    }
    Some(blocks)
}

/// Inline rows of the whole file: no hunk headers, every line, and each block's action row
/// right above its first changed line.
pub fn inline_rows(w: &WholeFile) -> Vec<DiffRow> {
    let lines = w.diff.hunks.first().map_or(0, |h| h.lines.len());
    let mut blocks = w.blocks.iter().peekable();
    let mut rows = Vec::with_capacity(lines + w.blocks.len());
    for line in 0..lines {
        if let Some(b) = blocks.next_if(|b| b.start == line) {
            rows.push(DiffRow::Block(b.hunk));
        }
        rows.push(DiffRow::Line { hunk: 0, line });
    }
    rows
}

/// Split rows of the whole file: removed and added runs paired as in `diff_rows::split_rows`,
/// without its hunk header, and each block's action row before the pair holding its first
/// line. A block starts after context or at the top of the file, so its first line opens a new
/// removed/added run and sits in that run's first pair.
pub fn split_rows(w: &WholeFile) -> Vec<SplitRow> {
    let pairs = diff_rows::split_rows(&w.diff);
    let mut blocks = w.blocks.iter().peekable();
    let mut rows = Vec::with_capacity(pairs.len() + w.blocks.len());
    for row in pairs {
        let SplitRow::Pair { old, new, .. } = row else { continue };
        if let Some(b) = blocks.next_if(|b| old == Some(b.start) || new == Some(b.start)) {
            rows.push(SplitRow::Block(b.hunk));
        }
        rows.push(row);
    }
    rows
}

/// The row index of each block's action row in `layout`, in block order.
pub fn block_rows(w: &WholeFile, layout: DiffLayout) -> Vec<usize> {
    let at = |is_block: Vec<bool>| is_block.iter().enumerate().filter(|(_, b)| **b).map(|(i, _)| i).collect();
    match layout {
        DiffLayout::Inline => at(inline_rows(w).iter().map(|r| matches!(r, DiffRow::Block(_))).collect()),
        DiffLayout::Split => at(split_rows(w).iter().map(|r| matches!(r, SplitRow::Block(_))).collect()),
    }
}

/// The whole file when `mode` shows it and it loaded. Views and `update` both decide which
/// rows are on screen through this.
pub fn shown(d: &FileDiff, mode: DiffMode) -> Option<&WholeFile> {
    d.whole.as_deref().filter(|_| mode == DiffMode::WholeFile)
}

/// The rows that next/previous change steps between: each block's action row in the whole-file
/// view, else each hunk header. Entry i is hunk i in the hunk view and `blocks[i].hunk` in the
/// whole-file view.
pub fn change_rows(d: &FileDiff, mode: DiffMode, layout: DiffLayout) -> Vec<usize> {
    match shown(d, mode) {
        Some(w) => block_rows(w, layout),
        None => header_rows(d, layout),
    }
}

/// The normal hunk index of change `i` (an index into `change_rows`).
pub fn hunk_of_change(d: &FileDiff, mode: DiffMode, i: usize) -> Option<usize> {
    match shown(d, mode) {
        Some(w) => w.blocks.get(i).map(|b| b.hunk),
        None => (i < d.hunks.len()).then_some(i),
    }
}

/// The change index (into `change_rows`) of normal hunk `h`.
pub fn change_of_hunk(d: &FileDiff, mode: DiffMode, h: usize) -> Option<usize> {
    match shown(d, mode) {
        Some(w) => w.blocks.iter().position(|b| b.hunk == h),
        None => (h < d.hunks.len()).then_some(h),
    }
}

#[cfg(test)]
mod tests;
