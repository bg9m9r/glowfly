//! Parsing diff lines with syntect: one parse state per side of the diff (`Side`), a hunk on
//! its own (`parse_hunk`) or every hunk in context (`parse_in_context`), and the scope-to-kind
//! mapping (`Classifier`).

use super::{MAX_LINE_BYTES, SYNTAXES, Span, TokenKind};
use crate::repo::{DiffLine, Hunk};
use std::collections::HashMap;
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference};

/// Why parsing stopped early.
pub(super) enum Stop {
    Parse,
    Cancelled,
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
    /// in `cx`, checking `cancelled` every 256 lines.
    pub(super) fn line(&mut self, text: &str, cx: &mut Classifier, cancelled: &dyn Fn() -> bool) -> Result<Vec<Span>, Stop> {
        cx.parsed += 1;
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

    /// Parses an unchanged line before a hunk, only for the state it leaves. Lines over
    /// `MAX_LINE_BYTES` are skipped, as in hunks.
    fn skip(&mut self, text: &str, cx: &mut Classifier, cancelled: &dyn Fn() -> bool) -> Result<(), Stop> {
        if text.len() <= MAX_LINE_BYTES {
            self.line(text, cx, cancelled)?;
        }
        Ok(())
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

/// Spans for every hunk in context: the old and new sides start at line 1 and carry on through
/// `gaps[h]` (the unchanged lines before hunk `h`, the same on both sides) and each hunk. While
/// both sides are in the same state, a gap is parsed once.
pub(super) fn parse_in_context(
    syntax: &SyntaxReference,
    hunks: &[Hunk],
    gaps: &[&[String]],
    cx: &mut Classifier,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<Vec<Span>>>, Stop> {
    let mut old = Side::new(syntax);
    let mut new = old.clone();
    let mut out = Vec::with_capacity(hunks.len());
    for (hunk, gap) in hunks.iter().zip(gaps) {
        if old == new {
            for l in *gap {
                old.skip(l, cx, cancelled)?;
            }
            new = old.clone();
        } else {
            // An earlier hunk left the sides in different states (e.g. it opened a comment on
            // one side only): each side parses the gap from its own state.
            for l in *gap {
                old.skip(l, cx, cancelled)?;
                new.skip(l, cx, cancelled)?;
            }
        }
        out.push(Side::hunk(&mut old, &mut new, hunk, cx, cancelled)?);
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
/// diff, so it also counts the lines parsed, for checking cancellation every 256 of them.
pub(super) struct Classifier {
    rules: &'static [(Scope, TokenKind)],
    cache: HashMap<Scope, Option<TokenKind>>,
    parsed: usize,
}

impl Classifier {
    pub(super) fn new() -> Self {
        Classifier { rules: &SYNTAXES.rules, cache: HashMap::new(), parsed: 0 }
    }

    /// The innermost scope that some rule matches decides; scopes no rule covers (punctuation,
    /// meta, source, plain variables) are skipped, so `//` resolves to its comment.
    fn classify(&mut self, stack: &[Scope]) -> TokenKind {
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
