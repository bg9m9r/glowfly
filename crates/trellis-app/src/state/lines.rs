//! The diff line selection for line-level staging: `+`/`-` lines of one hunk of a working-tree
//! diff, picked with click, Shift-click (range) and Ctrl/Cmd-click (toggle).

use std::collections::BTreeSet;
use trellis_core::repo::{DiffTarget, FileDiff};

/// How a line was clicked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineClick {
    /// A plain click: select just this line (or deselect it if it is the only one).
    Only,
    /// Shift: every changed line from the anchor to this one.
    Range,
    /// Ctrl/Cmd: add or remove this line.
    Toggle,
}

#[derive(Default, Clone, PartialEq, Debug)]
pub struct LineSelection {
    /// The diff the lines belong to; `None` when nothing is selected.
    pub target: Option<DiffTarget>,
    pub hunk: usize,
    /// The hunk's header when the selection started, so a changed diff is refused.
    pub header: String,
    /// Indexes into the hunk's lines; only `+` and `-` lines.
    pub lines: BTreeSet<usize>,
    /// Where a Shift range starts.
    pub anchor: usize,
}

impl LineSelection {
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    fn is_in(&self, target: &DiffTarget, hunk: usize) -> bool {
        !self.lines.is_empty() && self.hunk == hunk && self.target.as_ref() == Some(target)
    }

    /// Selected lines in this hunk of this diff.
    pub fn count_in(&self, target: &DiffTarget, hunk: usize) -> usize {
        if self.is_in(target, hunk) { self.lines.len() } else { 0 }
    }

    pub fn contains(&self, target: &DiffTarget, hunk: usize, line: usize) -> bool {
        self.is_in(target, hunk) && self.lines.contains(&line)
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Applies a click on line `line` of hunk `hunk` in `diff`. Only working-tree diffs can be
    /// selected; a plain click on any other line clears the selection, Shift or Ctrl on one does
    /// nothing.
    pub fn click(&mut self, target: &DiffTarget, diff: &FileDiff, hunk: usize, line: usize, how: LineClick) {
        if !target.is_working_tree() {
            return;
        }
        let Some(h) = diff.hunks.get(hunk) else { return };
        let changed = |i: usize| h.lines.get(i).is_some_and(|l| matches!(l.origin, '+' | '-'));
        if !changed(line) {
            if how == LineClick::Only {
                self.clear();
            }
            return;
        }
        let same = self.is_in(target, hunk);
        match how {
            LineClick::Only if same && self.lines.len() == 1 && self.lines.contains(&line) => self.clear(),
            LineClick::Range if same => {
                let (from, to) = (self.anchor.min(line), self.anchor.max(line));
                self.lines = (from..=to).filter(|&i| changed(i)).collect();
            }
            LineClick::Toggle if same => {
                if !self.lines.remove(&line) {
                    self.lines.insert(line);
                }
                self.anchor = line;
                if self.lines.is_empty() {
                    self.clear();
                }
            }
            _ => {
                *self = Self { target: Some(target.clone()), hunk, header: h.header.clone(), lines: BTreeSet::from([line]), anchor: line };
            }
        }
    }
}
