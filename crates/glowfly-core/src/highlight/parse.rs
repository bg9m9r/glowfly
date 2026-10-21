//! Parsing diff lines with syntect: one parse state per side of the diff (`Side`), a hunk on
//! its own (`parse_hunk`) or every hunk in context (`parse_in_context`, through the checkpoints
//! of `memo`), and the scope-to-kind mapping (`Classifier`, with an optional budget).

use super::memo::{Memo, parse_stream, streams};
use super::{MAX_LINE_BYTES, SYNTAXES, Span, TokenKind};
use crate::repo::{DiffLine, Hunk};
use std::collections::HashMap;
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference};

/// Why parsing stopped early.
pub(super) enum Stop {
    Parse,
    Cancelled,
    /// More than the `Classifier`'s budget would have to be parsed.
    Budget,
}

/// The parse state of one side of a diff (the old file or the new file) at some line.
#[derive(Clone, PartialEq)]
pub(super) struct Side {
    state: ParseState,
    stack: ScopeStack,
}

impl Side {
    /// The state at the top of a file.
    pub(super) fn new(syntax: &SyntaxReference) -> Side {
        Side { state: ParseState::new(syntax), stack: ScopeStack::new() }
    }

    /// Parses one line (`text` without its terminator) and returns its spans. Counts the line
    /// in `cx`, checking `cancelled` every 256 lines and stopping once `cx`'s budget is spent.
    pub(super) fn line(&mut self, text: &str, cx: &mut Classifier, cancelled: &dyn Fn() -> bool) -> Result<Vec<Span>, Stop> {
        cx.parsed += 1;
        cx.bytes += text.len();
        if cx.limit.is_some_and(|(lines, bytes)| cx.parsed > lines || cx.bytes > bytes) {
            return Err(Stop::Budget);
        }
        if cx.parsed.is_multiple_of(256) && cancelled() {
            return Err(Stop::Cancelled);
        }
        let ops = self.state.parse_line(&format!("{text}\n"), &SYNTAXES.set).map_err(|_| Stop::Parse)?;
        let mut spans = Vec::new();
        let mut pos = 0;
        for (off, op) in ops {
            if off > pos {
                push(&mut spans, pos, off.min(text.len()), cx.classify(self.stack.as_slice()));
            }
            self.stack.apply(&op).map_err(|_| Stop::Parse)?;
            pos = pos.max(off);
        }
        push(&mut spans, pos.min(text.len()), text.len(), cx.classify(self.stack.as_slice()));
        Ok(spans)
    }

    /// Parses the lines whose origin passes `include`, continuing from this state, and stores
    /// spans for those that also pass `keep`. EOF markers (`<`, `>`, `=`) are never included.
    /// Lines over `MAX_LINE_BYTES` are skipped and keep no spans; parsing goes on as if they
    /// were absent, so a long line that opens a string or a comment isn't seen (an accepted
    /// approximation).
    fn lines(
        &mut self,
        lines: &[DiffLine],
        out: &mut [Vec<Span>],
        cx: &mut Classifier,
        include: impl Fn(char) -> bool,
        keep: impl Fn(char) -> bool,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), Stop> {
        let included = lines.iter().enumerate().filter(|(_, l)| include(l.origin) && l.text.len() <= MAX_LINE_BYTES);
        for (i, dl) in included {
            let spans = self.line(&dl.text, cx, cancelled)?;
            if keep(dl.origin) {
                out[i] = spans;
            }
        }
        Ok(())
    }

    /// Spans for `hunk`, continuing `old` and `new` through it.
    fn hunk(
        old: &mut Side,
        new: &mut Side,
        hunk: &Hunk,
        cx: &mut Classifier,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<Vec<Span>>, Stop> {
        let mut spans = vec![Vec::new(); hunk.lines.len()];
        // Old side: context and removed lines; only the removed lines keep these colors.
        old.lines(&hunk.lines, &mut spans, cx, |o| matches!(o, ' ' | '-'), |o| o == '-', cancelled)?;
        // New side: context and added lines; context lines are colored from this side.
        new.lines(&hunk.lines, &mut spans, cx, |o| matches!(o, ' ' | '+'), |_| true, cancelled)?;
        Ok(spans)
    }
}

/// Spans for one hunk on its own: the old side, then the new side, each from a fresh state.
pub(super) fn parse_hunk(
    syntax: &SyntaxReference,
    hunk: &Hunk,
    cx: &mut Classifier,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<Span>>, Stop> {
    let mut old = Side::new(syntax);
    let mut new = old.clone();
    Side::hunk(&mut old, &mut new, hunk, cx, cancelled)
}

/// Spans for every hunk in context. Two streams are parsed from line 1: the old file (`gaps[h]`,
/// the unchanged lines before hunk `h`, then its context and removed lines, for every hunk) and
/// the new file (the same gaps, then the context and added lines). Removed lines take the old
/// stream's spans, context and added lines the new stream's. Chunks found in `memo_in` (or
/// already parsed into `memo_out`, which is how the new side reuses the gaps the old side
/// parsed) from the same state aren't parsed again; every chunk passed ends up in `memo_out`.
pub(super) fn parse_in_context(
    syntax: &SyntaxReference,
    hunks: &[Hunk],
    gaps: &[&[String]],
    memo_in: Option<&Memo>,
    memo_out: &mut Memo,
    cx: &mut Classifier,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<Vec<Span>>>, Stop> {
    let s = streams(hunks, gaps);
    let mut out: Vec<Vec<Vec<Span>>> = hunks.iter().map(|h| vec![Vec::new(); h.lines.len()]).collect();
    let old = parse_stream(syntax, &s.old.lines, memo_in, memo_out, cx, cancelled)?;
    for (spans, at) in old.into_iter().zip(&s.old.at) {
        if let Some((h, l)) = *at
            && hunks[h].lines[l].origin == '-'
        {
            out[h][l] = spans;
        }
    }
    let new = parse_stream(syntax, &s.new.lines, memo_in, memo_out, cx, cancelled)?;
    for (spans, at) in new.into_iter().zip(&s.new.at) {
        if let Some((h, l)) = *at {
            out[h][l] = spans;
        }
    }
    Ok(out)
}

/// Appends `[start, end)` unless empty, merging it into the previous span of the same kind.
fn push(spans: &mut Vec<Span>, start: usize, end: usize, kind: TokenKind) {
    if start >= end {
        return;
    }
    let (start, end) = (start as u32, end as u32);
    match spans.last_mut() {
        Some(last) if last.kind == kind && last.end == start => last.end = end,
        _ => spans.push(Span { start, end, kind }),
    }
}

/// Maps a scope stack to a token kind, caching per scope. One is shared by all the hunks of a
/// diff, so it also counts the lines (and bytes) parsed, for checking cancellation every 256
/// lines and for an optional budget.
pub(super) struct Classifier {
    rules: &'static [(Scope, TokenKind)],
    cache: HashMap<Scope, Option<TokenKind>>,
    parsed: usize,
    bytes: usize,
    /// At most this many lines and bytes may be parsed (`Stop::Budget` past them).
    limit: Option<(usize, usize)>,
}

impl Classifier {
    pub(super) fn new() -> Self {
        Classifier { rules: &SYNTAXES.rules, cache: HashMap::new(), parsed: 0, bytes: 0, limit: None }
    }

    /// One that stops with `Stop::Budget` once more than `lines` lines or `bytes` bytes are parsed.
    pub(super) fn with_budget(lines: usize, bytes: usize) -> Self {
        Classifier { limit: Some((lines, bytes)), ..Classifier::new() }
    }

    /// Lifts the budget (for what is parsed after the budgeted part).
    pub(super) fn unlimited(&mut self) {
        self.limit = None;
    }

    /// The innermost scope that some rule matches decides; scopes no rule covers (punctuation,
    /// meta, source, plain variables) are skipped, so `//` resolves to its comment.
    pub(super) fn classify(&mut self, stack: &[Scope]) -> TokenKind {
        for &scope in stack.iter().rev() {
            let rules = self.rules;
            let kind = *self.cache.entry(scope).or_insert_with(|| rules.iter().find(|(r, _)| r.is_prefix_of(scope)).map(|(_, k)| *k));
            if let Some(kind) = kind {
                return kind;
            }
        }
        TokenKind::Plain
    }
}
