//! The diff line selection: `+`/`-` lines of one hunk of a diff (staging on working-tree diffs,
//! Revert lines on commit and comparison diffs, Comment on N lines on a pull request's), picked
//! in the lines' gutter (their numbers and sign, never their text) with click, Shift-click
//! (range), Ctrl/Cmd-click (toggle), or a press-and-drag across lines (kept to the hunk it starts
//! in). Nothing is ever selected by default: the selection is cleared whenever another diff
//! opens.

use glowfly_core::repo::{DiffTarget, FileDiff, Hunk};
use std::collections::BTreeSet;

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
    /// The press-and-drag in progress, if any.
    pub drag: Option<LineDrag>,
}

/// A press-and-drag across the lines of `LineSelection::hunk`.
#[derive(Clone, PartialEq, Debug)]
pub struct LineDrag {
    /// The line the range runs from: the press line, or the old anchor for a Shift drag.
    pub from: usize,
    /// Lines kept whatever the range covers: the earlier selection for a Ctrl/Cmd drag in the
    /// same hunk, otherwise none.
    pub base: BTreeSet<usize>,
    /// The last (hunk, line) the view reported, so it sends only changes.
    pub over: (usize, usize),
}

/// Whether line `i` of `h` is a `+` or `-` line.
fn changed_in(h: &Hunk, i: usize) -> bool {
    h.lines.get(i).is_some_and(|l| matches!(l.origin, '+' | '-'))
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

    /// Applies a click on line `line` of hunk `hunk` in `diff` (any diff: working-tree, commit
    /// or comparison). Only `+`/`-` lines can be selected; a plain click on any other line
    /// clears the selection, Shift or Ctrl on one does nothing.
    pub fn click(&mut self, target: &DiffTarget, diff: &FileDiff, hunk: usize, line: usize, how: LineClick) {
        self.drag = None;
        let Some(h) = diff.hunks.get(hunk) else { return };
        let changed = |i: usize| changed_in(h, i);
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
                *self = Self::only(target, hunk, h, line);
            }
        }
    }

    /// A selection of just line `line` of hunk `hunk` (`h`).
    fn only(target: &DiffTarget, hunk: usize, h: &Hunk, line: usize) -> Self {
        Self { target: Some(target.clone()), hunk, header: h.header.clone(), lines: BTreeSet::from([line]), anchor: line, drag: None }
    }

    /// A press on changed line `line` of hunk `hunk` became a drag. Plain: the selection starts
    /// over at the line. Shift (in the selection's hunk): the range runs from the anchor, which
    /// stays. Ctrl/Cmd (in the selection's hunk): the dragged range adds to the lines already
    /// selected. A press on any other line does nothing.
    pub fn start_drag(&mut self, target: &DiffTarget, diff: &FileDiff, hunk: usize, line: usize, how: LineClick) {
        let Some(h) = diff.hunks.get(hunk) else { return };
        if !changed_in(h, line) {
            return;
        }
        let same = self.is_in(target, hunk);
        let (from, base) = match how {
            LineClick::Range if same => {
                let (a, b) = (self.anchor.min(line), self.anchor.max(line));
                self.lines = (a..=b).filter(|&i| changed_in(h, i)).collect();
                (self.anchor, BTreeSet::new())
            }
            LineClick::Toggle if same => {
                let base = self.lines.clone();
                self.lines.insert(line);
                self.anchor = line;
                (line, base)
            }
            _ => {
                *self = Self::only(target, hunk, h, line);
                (line, BTreeSet::new())
            }
        };
        self.drag = Some(LineDrag { from, base, over: (hunk, line) });
    }

    /// During a drag, the pointer is over line `line` of hunk `hunk` (0 for a hunk header or
    /// the gap rows above it): the selection is every changed line from the drag's start to
    /// there, kept to the drag's hunk (a later hunk stops at its last changed line, an earlier
    /// one at its first).
    pub fn drag_to(&mut self, target: &DiffTarget, diff: &FileDiff, hunk: usize, line: usize) {
        if self.target.as_ref() != Some(target) {
            return;
        }
        let Some(h) = diff.hunks.get(self.hunk) else { return };
        let Some(drag) = self.drag.as_mut() else { return };
        drag.over = (hunk, line);
        let changed: Vec<usize> = (0..h.lines.len()).filter(|&i| changed_in(h, i)).collect();
        let (Some(&first), Some(&last)) = (changed.first(), changed.last()) else { return };
        let to = match hunk.cmp(&self.hunk) {
            std::cmp::Ordering::Less => first,
            std::cmp::Ordering::Greater => last,
            std::cmp::Ordering::Equal => line.min(h.lines.len() - 1),
        };
        let (a, b) = (drag.from.min(to), drag.from.max(to));
        self.lines = drag.base.iter().copied().chain(changed.into_iter().filter(|i| (a..=b).contains(i))).collect();
    }

    /// The button was released: the selection stays.
    pub fn end_drag(&mut self) {
        self.drag = None;
    }

    pub fn dragging(&self) -> Option<&LineDrag> {
        self.drag.as_ref()
    }
}
