//! Folds of the whole-file view: long runs of unchanged lines away from every change, drawn
//! as one "N unchanged lines" row until the user opens them.

use super::WholeFile;
use crate::diff_rows::GapText;
use std::collections::BTreeSet;
use std::sync::Arc;

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

/// The unchanged runs the user opened, whole-file folds and hunk-view gaps alike, by key, and the
/// gaps' text once read.
#[derive(Clone, Default, Debug)]
pub struct Expanded {
    keys: BTreeSet<u32>,
    text: Option<Arc<GapText>>,
}

/// Equal keys, and the same text by address: the rows cache compares this every frame, so the
/// text's content is never compared.
impl PartialEq for Expanded {
    fn eq(&self, other: &Self) -> bool {
        let same_text = match (&self.text, &other.text) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        same_text && self.keys == other.keys
    }
}

impl Eq for Expanded {}

impl Expanded {
    /// Opens the fold `key`; false when it was already open.
    pub fn insert(&mut self, key: u32) -> bool {
        self.keys.insert(key)
    }

    /// Closes the fold `key`; false when it was not open.
    pub fn remove(&mut self, key: u32) -> bool {
        self.keys.remove(&key)
    }

    pub fn contains(&self, key: u32) -> bool {
        self.keys.contains(&key)
    }

    /// Closes everything and drops the gaps' text.
    pub fn clear(&mut self) {
        self.keys.clear();
        self.text = None;
    }

    /// No run is open (the text may still be kept).
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// The keys of the open runs, in order.
    pub fn keys(&self) -> impl Iterator<Item = u32> + '_ {
        self.keys.iter().copied()
    }

    /// Every fold of `w` opened.
    pub fn all(w: &WholeFile) -> Expanded {
        Expanded { keys: folds(w).iter().map(|f| f.key).collect(), text: None }
    }

    /// Keeps `text` as the gaps' text.
    pub fn set_gap_text(&mut self, text: Arc<GapText>) {
        self.text = Some(text);
    }

    /// Drops the gaps' text, keeping which runs are open.
    pub fn drop_gap_text(&mut self) {
        self.text = None;
    }

    /// The gaps' text, once read.
    pub fn gap_text(&self) -> Option<&GapText> {
        self.text.as_deref()
    }

    /// `self` with `other`'s gap text.
    pub fn with_gap_text_of(mut self, other: &Expanded) -> Self {
        self.text = other.text.clone();
        self
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

/// The folds of `w` the user opened, in order. Keys in `ex` that are not folds of `w` are
/// ignored.
pub fn opened(w: &WholeFile, ex: &Expanded) -> Vec<Fold> {
    folds(w).into_iter().filter(|f| ex.contains(f.key)).collect()
}
