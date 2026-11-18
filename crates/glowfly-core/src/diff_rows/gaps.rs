//! The unchanged lines outside a diff's hunks in the hunk view: where each run sits (`gaps`),
//! its text once read (`GapText`), and the rows with the runs the user opened filled in: between
//! the previous hunk's last line and the next hunk's header, above the first header, and after
//! the last hunk's last line (where a closed run is one `GapBelow` row).
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

/// The unchanged run before hunk `hunk`: `len` lines from old line `old_start` and new line
/// `new_start` (1-based). `hunk` 0 is the run above the first hunk and `hunks.len()` the run
/// below the last. `key` is `new_start`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GapSpan {
    pub hunk: usize,
    pub old_start: u32,
    pub new_start: u32,
    pub len: usize,
    pub key: u32,
}

/// Every non-empty run of `d` outside its hunks (above the first, between two, below the last),
/// in order. The run below needs `d.old_lines`: without it there is none. None for a binary
/// diff, or when the hunks can't be placed. A run above or between exists only when both sides
/// have the same number of lines there.
pub fn gaps(d: &FileDiff) -> Vec<GapSpan> {
    if d.binary || d.hunks.is_empty() {
        return Vec::new();
    }
    let (Some(old), Some(new)) = (starts(&d.hunks), new_starts(&d.hunks)) else { return Vec::new() };
    let n = d.hunks.len();
    let mut out = Vec::new();
    // The lines before the run on each side.
    let (mut old_from, mut new_from) = (0, 0);
    for h in 0..=n {
        let len = if h < n {
            let len = old[h].saturating_sub(old_from);
            (new[h].saturating_sub(new_from) == len).then_some(len)
        } else {
            Some(d.old_lines.map_or(0, |total| (total as usize).saturating_sub(old_from)))
        };
        if let Some(len) = len.filter(|&len| len > 0)
            && let (Ok(old_start), Ok(new_start)) = (u32::try_from(old_from + 1), u32::try_from(new_from + 1))
        {
            out.push(GapSpan { hunk: h, old_start, new_start, len, key: new_start });
        }
        if h < n {
            old_from = old[h] + old_count(&d.hunks[h]);
            new_from = new[h] + new_count(&d.hunks[h]);
        }
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

/// `inline_rows`, with the opened runs' lines before their hunk's header and the run below the
/// last hunk after it (its lines when opened with their text read, else one `GapBelow` row).
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
    if let Some(g) = below(d) {
        match open.iter().find(|(f, _)| f.hunk == g.hunk) {
            Some(_) => rows.extend((0..g.len).map(|line| DiffRow::GapLine { hunk: g.hunk, line })),
            None => rows.push(DiffRow::GapBelow(g)),
        }
    }
    rows
}

/// `split_rows`, with the opened runs' lines before their hunk's header and the run below the
/// last hunk after it.
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
    if let Some(g) = below(d) {
        match open.iter().find(|(f, _)| f.hunk == g.hunk) {
            Some(_) => rows.extend((0..g.len).map(|line| SplitRow::GapLine { hunk: g.hunk, line })),
            None => rows.push(SplitRow::GapBelow(g)),
        }
    }
    rows
}

/// The run below the last hunk of `d`, if it has one.
fn below(d: &FileDiff) -> Option<GapSpan> {
    gaps(d).into_iter().find(|g| g.hunk == d.hunks.len())
}

/// `header_rows`, counted with the opened runs filled in. A run before a header shifts it and
/// every later one; the run below the last hunk shifts none.
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

/// The row of the control that opens or folds run `g`, in the rows `inline_rows_filled` or
/// `split_rows_filled` give: its hunk's header (the first line of an opened run is above it, and
/// the header keeps the control), or for the run below the last hunk the index of its `GapBelow`
/// row, which is the row after the last one while it is open. `None` when the hunks don't give
/// the run a row.
pub fn control_row(d: &FileDiff, layout: DiffLayout, ex: &Expanded, g: &GapSpan) -> Option<usize> {
    if g.hunk < d.hunks.len() {
        return header_rows_filled(d, layout, ex).get(g.hunk).copied();
    }
    let len = match layout {
        DiffLayout::Inline => inline_rows_filled(d, ex).len(),
        DiffLayout::Split => split_rows_filled(d, ex).len(),
    };
    let open = filled(d, ex).iter().any(|(f, _)| f.hunk == g.hunk);
    Some(if open { len - g.len } else { len - 1 })
}

#[cfg(test)]
#[path = "gaps_tests.rs"]
mod tests;
