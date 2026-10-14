//! Decorating loaded diffs (syntax spans and word marks), and finishing large diffs' syntax
//! highlighting in the background. `Runner::run` gives each background job a cancel ticket, so a
//! newer `Effect::HighlightDiff` cancels the older one.

use crate::msg::Msg;
use std::sync::Arc;
use std::time::Instant;
use trellis_core::highlight::{self, DiffHighlight, OldText};
use trellis_core::intraline;
use trellis_core::repo::{DiffTarget, FileDiff};

/// Syntax spans and word marks for a loaded diff and its whole file, reusing `previous`'s spans,
/// with each hunk highlighted on its own (`decorate_in_context` without old text).
#[cfg(test)]
pub(crate) fn decorate(path: &str, d: &mut FileDiff, previous: Option<&FileDiff>) {
    decorate_in_context(path, d, previous, None);
}

/// Syntax spans and word marks for a loaded diff and its whole file, reusing `previous`'s spans.
/// With `old` (the old file's text), the hunks are highlighted in context; the whole file needs
/// none. Large diffs come back with a pending highlight, for `finish_job`.
pub(crate) fn decorate_in_context(path: &str, d: &mut FileDiff, previous: Option<&FileDiff>, old: Option<&Arc<OldText>>) {
    d.highlight = highlight::highlight_in_context(path, d, previous, old);
    d.intraline = intraline::intraline(d);
    if let Some(w) = d.whole.as_mut() {
        w.diff.highlight = highlight::highlight_diff(path, &w.diff, previous.and_then(|p| p.whole.as_deref()).map(|w| &w.diff));
        w.diff.intraline = intraline::intraline(&w.diff);
    }
}

/// Finishes `diff`'s pending highlights (the whole file first, as the whole-file view shows it).
/// `None` when cancelled.
pub(super) fn finish_job(target: DiffTarget, generation: u64, diff: &FileDiff, cancelled: &dyn Fn() -> bool) -> Option<Msg> {
    let started = Instant::now();
    let pending = |d: &FileDiff| d.highlight.as_ref().is_some_and(DiffHighlight::is_pending);
    let whole = match diff.whole.as_deref().filter(|w| pending(&w.diff)) {
        Some(w) => Some(highlight::finish(&w.diff, cancelled)?),
        None => None,
    };
    let hunks = if pending(diff) { Some(highlight::finish(diff, cancelled)?) } else { None };
    let ms = started.elapsed().as_millis() as u64;
    tracing::debug!(path = target.path(), ms, "diff highlighted in the background");
    Some(Msg::DiffHighlighted { target, generation, hunks, whole })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finish_job_reports_unless_cancelled() {
        let text: String = (0..=highlight::MAX_LINES).map(|i| format!("let x{i} = {i};\n")).collect();
        let lines = text.lines().map(|t| trellis_core::repo::DiffLine { origin: '+', old: None, new: None, text: t.into() }).collect();
        let hunk = trellis_core::repo::Hunk { header: "@@ -0,0 +1 @@".into(), lines };
        let mut d = FileDiff { binary: false, hunks: vec![hunk], highlight: None, intraline: Default::default(), whole: None };
        decorate("big.rs", &mut d, None);
        assert!(d.highlight.as_ref().is_some_and(DiffHighlight::is_pending));
        let target = DiffTarget::Unstaged("big.rs".into());

        assert!(finish_job(target.clone(), 7, &d, &|| true).is_none(), "a cancelled job sends nothing");
        let Some(Msg::DiffHighlighted { target: t, generation: 7, hunks: Some(h), whole: None }) =
            finish_job(target.clone(), 7, &d, &|| false)
        else {
            panic!("expected the finished hunks for generation 7");
        };
        assert_eq!(t, target);
        assert!(!h.is_pending());
    }

    #[test]
    fn finish_job_parses_in_context() {
        // One added line after the old file's MAX_LINES lines: pending, as those lines count.
        let text: String = (0..highlight::MAX_LINES)
            .map(|i| {
                format!(
                    "let x{i} = {i};
"
                )
            })
            .collect();
        let at = highlight::MAX_LINES as u32 + 1;
        let line = trellis_core::repo::DiffLine { origin: '+', old: None, new: Some(at), text: "let y = 1;".into() };
        let hunk = trellis_core::repo::Hunk { header: format!("@@ -{},0 +{at} @@", at - 1), lines: vec![line] };
        let old = OldText::new(text.as_bytes(), std::slice::from_ref(&hunk)).map(Arc::new);
        assert!(old.is_some());
        let mut d = FileDiff { binary: false, hunks: vec![hunk], highlight: None, intraline: Default::default(), whole: None };
        decorate_in_context("a.rs", &mut d, None, old.as_ref());
        assert!(d.highlight.as_ref().is_some_and(DiffHighlight::is_pending));
        let target = DiffTarget::Unstaged("a.rs".into());
        let Some(Msg::DiffHighlighted { hunks: Some(h), .. }) = finish_job(target, 1, &d, &|| false) else {
            panic!("expected the finished hunks");
        };
        assert!(!h.is_pending() && !h.line(0, 0).is_empty());
    }
}
