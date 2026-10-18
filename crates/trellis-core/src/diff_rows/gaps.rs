//! The unchanged lines between hunks in the hunk view: where each run sits (`gaps`), its text
//! once read (`GapText`), and the rows with the runs the user opened filled in, between the
//! previous hunk's last line and the next hunk's header.
//!
//! A run is named by the new-side number of its first line, the key the whole-file view gives
//! the fold of the same lines (hunks keep `FOLD_CONTEXT` lines of context, as folds do), so one
//! opened in either view is open in the other.

use super::{DiffRow, SplitRow, header_rows, inline_rows, split_rows};
use crate::highlight::Span;
use crate::highlight::context::{new_count, new_starts, old_count, starts};
use crate::repo::FileDiff;
use crate::settings::DiffLayout;
use crate::whole_file::Expanded;

/// The unchanged run before hunk `hunk` (never the first): `len` lines from old line
/// `old_start` and new line `new_start` (1-based). `key` is `new_start`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GapSpan {
    pub hunk: usize,
    pub old_start: u32,
    pub new_start: u32,
    pub len: usize,
    pub key: u32,
}

/// Every non-empty run between two hunks of `d`, in order. None for a binary diff, or when the
/// hunks can't be placed.
pub fn gaps(d: &FileDiff) -> Vec<GapSpan> {
    if d.binary {
        return Vec::new();
    }
    let (Some(old), Some(new)) = (starts(&d.hunks), new_starts(&d.hunks)) else { return Vec::new() };
    let mut out = Vec::new();
    for h in 1..d.hunks.len() {
        let (prev, hunk) = (&d.hunks[h - 1], h);
        let old_from = old[h - 1] + old_count(prev);
        let new_from = new[h - 1] + new_count(prev);
        let len = old[hunk].saturating_sub(old_from);
        if len == 0 || new[hunk].saturating_sub(new_from) != len {
            continue;
        }
        let (Ok(old_start), Ok(new_start)) = (u32::try_from(old_from + 1), u32::try_from(new_from + 1)) else { continue };
        out.push(GapSpan { hunk, old_start, new_start, len, key: new_start });
    }
    out
}

/// The text of a run, with its syntax spans (empty when plain).
#[derive(Clone, Debug, PartialEq)]
pub struct GapLines {
    pub span: GapSpan,
    pub lines: Vec<String>,
    pub spans: Vec<Vec<Span>>,
}

/// The runs' text read for one diff.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct GapText {
    gaps: Vec<GapLines>,
}

impl GapText {
    pub fn new(gaps: Vec<GapLines>) -> GapText {
        GapText { gaps }
    }

    /// Every run read.
    pub fn all(&self) -> &[GapLines] {
        &self.gaps
    }

    /// The text of run `g`, matched by where it sits (not its hunk index), so text read for a
    /// run that has since moved or changed size never fills it.
    pub fn lines_for(&self, g: &GapSpan) -> Option<&GapLines> {
        self.gaps.iter().find(|t| (t.span.old_start, t.span.new_start, t.span.len) == (g.old_start, g.new_start, g.len))
    }
}

/// The runs of `d` opened in `ex` whose text is there, in order.
pub fn filled<'a>(d: &FileDiff, ex: &'a Expanded) -> Vec<(GapSpan, &'a GapLines)> {
    let Some(text) = ex.gap_text() else { return Vec::new() };
    gaps(d).into_iter().filter(|g| ex.contains(g.key)).filter_map(|g| Some((g, text.lines_for(&g)?))).collect()
}

/// `inline_rows`, with the opened runs' lines before their hunk's header.
pub fn inline_rows_filled(d: &FileDiff, ex: &Expanded) -> Vec<DiffRow> {
    let open = filled(d, ex);
    let mut rows = Vec::new();
    for row in inline_rows(d) {
        if let DiffRow::Header(h) = row
            && let Some((g, _)) = open.iter().find(|(g, _)| g.hunk == h)
        {
            rows.extend((0..g.len).map(|line| DiffRow::GapLine { hunk: h, line }));
        }
        rows.push(row);
    }
    rows
}

/// `split_rows`, with the opened runs' lines before their hunk's header.
pub fn split_rows_filled(d: &FileDiff, ex: &Expanded) -> Vec<SplitRow> {
    let open = filled(d, ex);
    let mut rows = Vec::new();
    for row in split_rows(d) {
        if let SplitRow::Header(h) = row
            && let Some((g, _)) = open.iter().find(|(g, _)| g.hunk == h)
        {
            rows.extend((0..g.len).map(|line| SplitRow::GapLine { hunk: h, line }));
        }
        rows.push(row);
    }
    rows
}

/// `header_rows`, counted with the opened runs filled in.
pub fn header_rows_filled(d: &FileDiff, layout: DiffLayout, ex: &Expanded) -> Vec<usize> {
    let open = filled(d, ex);
    let mut shift = 0;
    let mut rows = header_rows(d, layout);
    for (h, row) in rows.iter_mut().enumerate() {
        shift += open.iter().filter(|(g, _)| g.hunk == h).map(|(g, _)| g.len).sum::<usize>();
        *row += shift;
    }
    rows
}

#[cfg(test)]
#[path = "gaps_tests.rs"]
mod tests;
