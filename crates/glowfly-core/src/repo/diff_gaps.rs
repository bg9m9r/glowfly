//! The unchanged lines between a diff's hunks, read for the hunk view when the user opens one of
//! those runs: the old file's text (which the hunks' context matches), with syntax spans.

use super::{DiffTarget, Hunk, LoadDiffOptions, load_diff_for_view};
use crate::diff_rows::{GapLines, GapText, gaps};
use crate::error::{Error, Result};
use crate::highlight;
use std::path::Path;

/// The text of every unchanged run between `hunks` (the hunks shown for `target`, read without
/// hiding whitespace), colored in `syntax` (a syntax name; `None` leaves them plain). Errors when
/// the file changed since (its hunks differ now), or when the old file can't be read in full
/// (over `highlight::OLD_TEXT_MAX_BYTES`, or a filter makes it differ from the hunks). A diff
/// with one hunk, or none, has no runs.
pub fn load_gap_text(workdir: &Path, target: &DiffTarget, hunks: &[Hunk], syntax: Option<&str>) -> Result<GapText> {
    let v = load_diff_for_view(workdir, target, LoadDiffOptions::default())?;
    if v.diff.hunks != hunks {
        return Err(Error::NotFound("the file changed".into()));
    }
    let spans = gaps(&v.diff);
    if spans.is_empty() {
        return Ok(GapText::default());
    }
    let unreadable = || Error::Unsupported("the old file can't be read".into());
    let old = v.old.ok_or_else(unreadable)?;
    let text = old.gaps(hunks).ok_or_else(unreadable)?;
    let colors = highlight::gap_spans(syntax, &old, hunks);
    let mut out = Vec::with_capacity(spans.len());
    for span in spans {
        let lines = text.get(span.hunk).filter(|l| l.len() == span.len).ok_or_else(unreadable)?;
        let spans = colors.as_ref().and_then(|c| c.get(span.hunk)).cloned().unwrap_or_else(|| vec![Vec::new(); span.len]);
        out.push(GapLines { span, lines: lines.to_vec(), spans });
    }
    Ok(GapText::new(out))
}

#[cfg(test)]
#[path = "diff_gaps_tests.rs"]
mod tests;
