//! The unchanged lines outside a diff's hunks (above the first, between two, below the last),
//! read for the hunk view when the user opens one of those runs: the old file's text (which the
//! hunks' context matches), with syntax spans.

use super::diff_view::{OldSide, load};
use super::{DiffTarget, FileDiff, LoadDiffOptions};
use crate::diff_rows::{GapLines, GapText, gaps};
use crate::error::{ReadFailed, Result};
use crate::highlight;
use std::path::Path;

/// The text of every unchanged run of `diff` (the diff shown for `target`, read without hiding
/// whitespace), colored in `syntax` (a syntax name; `None` leaves them plain). Errors when the
/// file changed since (its hunks differ now, or the old file's line count the diff carries),
/// or when the old file can't be read in full (over `highlight::OLD_TEXT_MAX_BYTES`, or a filter
/// makes it differ from the hunks). A diff with no runs gives no text.
pub fn load_gap_text(workdir: &Path, target: &DiffTarget, diff: &FileDiff, syntax: Option<&str>) -> Result<GapText, ReadFailed> {
    let v = load(workdir, target, LoadDiffOptions::default(), OldSide::Whole, None)?;
    let hunks = &diff.hunks;
    if v.diff.hunks != *hunks || (diff.old_lines.is_some() && v.diff.old_lines != diff.old_lines) {
        return Err(ReadFailed::FileChanged);
    }
    let spans = gaps(diff);
    if spans.is_empty() {
        return Ok(GapText::default());
    }
    let unreadable = || ReadFailed::OldFileUnreadable;
    let old = v.old.ok_or_else(unreadable)?;
    let between = old.gaps(hunks).ok_or_else(unreadable)?;
    let tail = old.tail(hunks);
    let colors = highlight::gap_spans(syntax, &old, hunks);
    let mut out = Vec::with_capacity(spans.len());
    for span in spans {
        let lines = if span.hunk < hunks.len() { between.get(span.hunk).copied() } else { tail };
        let lines = lines.filter(|l| l.len() == span.len).ok_or_else(unreadable)?;
        let spans = colors.as_ref().and_then(|c| c.get(span.hunk)).filter(|c| c.len() == span.len).cloned();
        out.push(GapLines { span, lines: lines.to_vec(), spans: spans.unwrap_or_else(|| vec![Vec::new(); span.len]) });
    }
    Ok(GapText::new(out))
}

#[cfg(test)]
#[path = "diff_gaps_tests.rs"]
mod tests;
