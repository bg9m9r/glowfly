//! Decorating loaded diffs (syntax spans and word marks), and finishing large diffs' syntax
//! highlighting in the background. `Runner::run` gives each background job a cancel ticket, so a
//! newer `Effect::HighlightDiff` cancels the older one.

use crate::msg::DiffMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use trellis_core::highlight::{self, DiffHighlight, OldText, SyntaxHint};
use trellis_core::intraline;
use trellis_core::repo::{self, DiffTarget, FileDiff, LoadDiffOptions};

/// Loads a diff for the viewer (`Effect::LoadDiff`) and decorates it, with the language its
/// gitattributes name, if any.
pub(super) fn load_job(
    workdir: PathBuf,
    target: DiffTarget,
    reload: bool,
    generation: u64,
    previous: Option<std::sync::Arc<FileDiff>>,
    opts: LoadDiffOptions,
) -> Msg {
    let result = repo::load_diff_for_view(&workdir, &target, opts)
        .map(|mut v| {
            decorate_in_context(target.path(), v.language.as_deref(), &mut v.diff, previous.as_deref(), v.old.as_ref());
            let d = &v.diff;
            let context = v.old.is_some();
            let language = v.language.as_deref().unwrap_or("-");
            let syntax = d.highlight.as_ref().map_or("plain", |h| h.syntax.as_str());
            let marked = (0..d.hunks.len())
                .map(|h| (0..d.hunks[h].lines.len()).filter(|&l| !d.intraline.line(h, l).is_empty()).count())
                .sum::<usize>();
            let whole = d.whole.is_some();
            let paired = d.intraline.pairs().len();
            let pending = d.highlight.as_ref().is_some_and(|h| h.is_pending());
            let ignore_whitespace = opts.ignore_whitespace;
            tracing::debug!(
                path = target.path(),
                syntax,
                language,
                pending,
                context,
                marked,
                paired,
                ignore_whitespace,
                whole,
                "diff highlighted"
            );
            v.diff
        })
        .map_err(|e| e.to_string());
    Msg::Repo(RepoMsg::DiffLoaded { target, reload, generation, result })
}

/// Syntax spans and word marks for a loaded diff and its whole file, reusing `previous`'s spans,
/// with each hunk highlighted on its own (`decorate_in_context` without old text).
#[cfg(test)]
pub(crate) fn decorate(path: &str, d: &mut FileDiff, previous: Option<&FileDiff>) {
    decorate_in_context(path, None, d, previous, None);
}

/// Syntax spans and word marks for a loaded diff and its whole file, reusing `previous`'s spans.
/// `language` is the path's `linguist-language` gitattribute (an override of the detected
/// language for both). With `old` (the old file's text), the hunks are highlighted in context;
/// the whole file needs none. Large diffs come back with a pending highlight, for `finish_job`.
pub(crate) fn decorate_in_context(
    path: &str,
    language: Option<&str>,
    d: &mut FileDiff,
    previous: Option<&FileDiff>,
    old: Option<&Arc<OldText>>,
) {
    let hint = SyntaxHint { path, language };
    d.highlight = highlight::highlight_in_context(hint, d, previous, old);
    d.intraline = intraline::intraline(d);
    if let Some(w) = d.whole.as_mut() {
        w.diff.highlight = highlight::highlight_diff(hint, &w.diff, previous.and_then(|p| p.whole.as_deref()).map(|w| &w.diff));
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
    Some(Msg::Diff(DiffMsg::DiffHighlighted { target, generation, hunks, whole }))
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
        let Some(Msg::Diff(DiffMsg::DiffHighlighted { target: t, generation: 7, hunks: Some(h), whole: None })) =
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
        decorate_in_context("a.rs", None, &mut d, None, old.as_ref());
        assert!(d.highlight.as_ref().is_some_and(DiffHighlight::is_pending));
        let target = DiffTarget::Unstaged("a.rs".into());
        let Some(Msg::Diff(DiffMsg::DiffHighlighted { hunks: Some(h), .. })) = finish_job(target, 1, &d, &|| false) else {
            panic!("expected the finished hunks");
        };
        assert!(!h.is_pending() && !h.line(0, 0).is_empty());
    }

    #[test]
    fn decorate_fills_line_pairs() {
        use trellis_core::repo::{DiffLine, Hunk};
        let l = |origin: char, old: Option<u32>, new: Option<u32>, text: &str| DiffLine { origin, old, new, text: text.into() };
        let lines = vec![
            l(' ', Some(1), Some(1), "[server]"),
            l('-', Some(2), None, "host = \"localhost\""),
            l('-', Some(3), None, "port = 8080"),
            l('+', None, Some(2), "# listen on all interfaces"),
            l('+', None, Some(3), "host = \"0.0.0.0\""),
            l('+', None, Some(4), "port = 9090"),
            l(' ', Some(4), Some(5), "timeout = 30"),
        ];
        let hunk = Hunk { header: "@@ -1,4 +1,5 @@".into(), lines };
        let whole = trellis_core::whole_file::WholeFile::new(std::slice::from_ref(&hunk), hunk.clone()).map(Box::new);
        assert!(whole.is_some());
        let mut d = FileDiff { binary: false, hunks: vec![hunk], highlight: None, intraline: Default::default(), whole };
        decorate("config.txt", &mut d, None);
        assert_eq!(d.intraline.pairs().hunk(0), [(1, 4), (2, 5)]);
        let w = d.whole.as_deref().expect("whole file");
        assert!(!w.diff.intraline.pairs().is_empty());
    }
}
