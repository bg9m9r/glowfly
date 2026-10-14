//! Folds of the whole-file view: long runs of unchanged lines away from every change, drawn
//! as one "N unchanged lines" row until the user opens them.

use super::WholeFile;
use std::collections::BTreeSet;

/// The unchanged lines kept next to each change, as many as git's default context, so a block
/// shows what its normal hunk shows.
pub const FOLD_CONTEXT: usize = 3;

/// Fewer hidden lines than this stay shown: a fold row would save little.
pub const MIN_FOLD: usize = 4;

/// A run of unchanged lines folded into one row: line indexes `start..end` into the whole-file
/// hunk. `key` names it across reloads that leave the lines in place: the new-side number of
/// its first line (or the old one, or the index when the line has neither).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fold {
    pub start: usize,
    pub end: usize,
    pub key: u32,
}

impl Fold {
    /// How many lines the fold hides.
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Never true: a fold hides at least `MIN_FOLD` lines.
    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }
}

/// The folds the user opened, by key.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Expanded(BTreeSet<u32>);

impl Expanded {
    /// Opens the fold `key`; false when it was already open.
    pub fn insert(&mut self, key: u32) -> bool {
        self.0.insert(key)
    }

    pub fn contains(&self, key: u32) -> bool {
        self.0.contains(&key)
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Every fold of `w` opened.
    pub fn all(w: &WholeFile) -> Expanded {
        Expanded(folds(w).iter().map(|f| f.key).collect())
    }
}

/// Every foldable run of `w`, in order. Only unchanged (` `) lines outside the change blocks
/// fold, and each run keeps `FOLD_CONTEXT` lines next to whatever comes before and after it
/// (a block, a marker line, or a line that isn't context), but not at the file's ends. A file
/// without change blocks gets none.
pub fn folds(w: &WholeFile) -> Vec<Fold> {
    let Some(hunk) = w.diff.hunks.first() else { return Vec::new() };
    if w.blocks.is_empty() {
        return Vec::new();
    }
    let lines = &hunk.lines;
    let n = lines.len();
    let mut gaps = Vec::with_capacity(w.blocks.len() + 1);
    let mut from = 0;
    for b in &w.blocks {
        gaps.push(from..b.start.min(n));
        from = b.end.min(n);
    }
    gaps.push(from..n);

    let mut out = Vec::new();
    for gap in gaps {
        let mut s = gap.start;
        while s < gap.end {
            if lines[s].origin != ' ' {
                s += 1;
                continue;
            }
            let e = (s..gap.end).find(|&i| lines[i].origin != ' ').unwrap_or(gap.end);
            let lead = if s > 0 { FOLD_CONTEXT } else { 0 };
            let trail = if e < n { FOLD_CONTEXT } else { 0 };
            let start = s + lead;
            if let Some(end) = e.checked_sub(trail).filter(|&end| end >= start + MIN_FOLD) {
                let l = &lines[start];
                let key = l.new.or(l.old).unwrap_or(start as u32);
                out.push(Fold { start, end, key });
            }
            s = e;
        }
    }
    out
}

/// The folds of `w` still closed.
pub fn collapsed(w: &WholeFile, ex: &Expanded) -> Vec<Fold> {
    folds(w).into_iter().filter(|f| !ex.contains(f.key)).collect()
}
