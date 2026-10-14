//! Syntax highlighting of diff lines with syntect, reduced to a few token kinds that the UI maps
//! to its own palette (no syntect themes). It runs where diffs are loaded, off the UI thread.
//!
//! The old file (context and `-` lines) and the new file (context and `+` lines) are parsed in
//! separate passes, and each hunk starts from a fresh parse state. Known limitation: a hunk that
//! starts inside a block comment or a multi-line string is colored as code until the delimiter
//! closes; highlighting from the whole old and new files would fix that. The whole-file view
//! (`whole_file`) does: its one hunk is the whole file.
//!
//! Two passes. `highlight_diff` parses small diffs while they load (the inline pass, within
//! `MAX_LINES` and `MAX_BYTES`). Larger ones come back pending: their hunks carry provisional
//! spans from the diff shown before (matched by line text, see `reuse`) or none, and `finish`
//! parses them later on a worker thread (the background pass, within `BACKGROUND_MAX_LINES`
//! and `BACKGROUND_MAX_BYTES`). Beyond that a diff stays plain. Lines over `MAX_LINE_BYTES`
//! (minified files) are never parsed in either pass and stay plain.

use crate::repo::{DiffLine, FileDiff, Hunk};
use reuse::{Cost, Previous};
use std::collections::HashMap;
use std::sync::LazyLock;
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet};

mod reuse;

// Parsing costs roughly 90 ms per 1,000 short lines and 1.4 ms per KB of text (release build),
// so the inline limits keep a load's worst case near 150-200 ms, and the background limits near
// 3.5-4 s on a worker thread (which a newer diff cancels).

/// Lines to parse while the diff loads (all hunks together; hunks reused from the diff shown
/// before don't count); more is finished in the background.
pub const MAX_LINES: usize = 2_000;
/// Text to parse while the diff loads (bytes, counted like `MAX_LINES`); more is finished in the
/// background.
pub const MAX_BYTES: usize = 100_000;
/// Lines longer than this (in bytes, e.g. minified files) are never parsed and stay plain. They
/// don't count toward any limit.
pub const MAX_LINE_BYTES: usize = 1_000;
/// Diffs with more lines to parse than this stay plain, even in the background.
pub const BACKGROUND_MAX_LINES: usize = 40_000;
/// Diffs with more text to parse than this (bytes) stay plain, even in the background.
pub const BACKGROUND_MAX_BYTES: usize = 2_000_000;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TokenKind {
    Plain,
    Comment,
    String,
    Number,
    Constant,
    Keyword,
    Type,
    Function,
    Attribute,
}

/// A run of one token kind: byte offsets into `DiffLine::text` (the raw line, tabs included;
/// the viewer maps them onto the tab-expanded text with `tabs::expand`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Span {
    pub start: u32,
    pub end: u32,
    pub kind: TokenKind,
}

/// Spans for every line of a `FileDiff`, indexed like `FileDiff::hunks[h].lines[l]`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiffHighlight {
    /// The syntax used, e.g. "Rust".
    pub syntax: String,
    /// One entry per line of every hunk, pending or not.
    pub(crate) lines: Vec<Vec<Vec<Span>>>,
    /// Per hunk: its spans are provisional (carried from the diff shown before, or empty) and
    /// `finish` still has to parse it. Empty means nothing is pending.
    pub(crate) pending: Vec<bool>,
}

impl DiffHighlight {
    /// The spans of one line; empty when out of range, so a mismatched highlight never panics.
    pub fn line(&self, hunk: usize, line: usize) -> &[Span] {
        self.lines.get(hunk).and_then(|h| h.get(line)).map_or(&[], Vec::as_slice)
    }

    /// Number of lines per hunk (for checking the shape against the diff).
    pub fn hunk_lens(&self) -> Vec<usize> {
        self.lines.iter().map(Vec::len).collect()
    }

    /// Some hunks still wait for the background pass (`finish`).
    pub fn is_pending(&self) -> bool {
        self.pending.iter().any(|&p| p)
    }

    fn hunk_pending(&self, hunk: usize) -> bool {
        self.pending.get(hunk).copied().unwrap_or(false)
    }
}

/// Immutable data loaded once on first use: the bundled syntax definitions and the
/// scope-to-kind rules.
struct Syntaxes {
    set: SyntaxSet,
    rules: Vec<(Scope, TokenKind)>,
}

static SYNTAXES: LazyLock<Syntaxes> = LazyLock::new(|| {
    use TokenKind::*;
    let table: &[(&[&str], TokenKind)] = &[
        (&["comment"], Comment),
        (&["string", "markup.raw"], String),
        (&["constant.numeric"], Number),
        (&["constant", "support.constant", "variable.language"], Constant),
        (&["keyword", "storage", "entity.name.tag", "markup.heading"], Keyword),
        (
            &[
                "entity.name.type",
                "entity.name.class",
                "entity.name.struct",
                "entity.name.enum",
                "entity.name.trait",
                "entity.name.union",
                "entity.name.interface",
                "entity.other.inherited-class",
                "support.type",
                "support.class",
            ],
            Type,
        ),
        (&["entity.name.function", "support.function", "variable.function"], Function),
        (&["entity.other.attribute-name", "meta.annotation"], Attribute),
    ];
    let rules = table.iter().flat_map(|(scopes, kind)| scopes.iter().map(move |s| (Scope::new(s).expect("valid scope"), *kind))).collect();
    Syntaxes { set: SyntaxSet::load_defaults_newlines(), rules }
});

fn find_syntax(path: &str, first_line: Option<&str>) -> Option<&'static SyntaxReference> {
    let set = &SYNTAXES.set;
    let name = path.rsplit('/').next().unwrap_or(path);
    let by_ext = name.rsplit_once('.').and_then(|(_, ext)| set.find_syntax_by_extension(ext));
    let found =
        by_ext.or_else(|| set.find_syntax_by_extension(name)).or_else(|| first_line.and_then(|l| set.find_syntax_by_first_line(l)))?;
    (found.name != "Plain Text").then_some(found)
}

/// The language for `path`: by extension, then by whole file name (`Makefile`), then by the
/// file's first line (shebangs). Never touches the disk. `None` for unknown or plain text.
pub fn syntax_name(path: &str, first_line: Option<&str>) -> Option<&'static str> {
    find_syntax(path, first_line).map(|s| s.name.as_str())
}

/// Token spans for every line of `diff`, or `None` to show it plain: binary, empty, an unknown
/// language, too large even for the background pass (`BACKGROUND_MAX_LINES`,
/// `BACKGROUND_MAX_BYTES`), or a parse error in the inline pass.
///
/// `previous` is the diff shown before a reload: hunks whose lines are unchanged keep its
/// finished spans instead of being parsed again (each hunk is parsed on its own, so its spans
/// depend only on its own lines), and only the other hunks count toward the inline limits.
/// Over those limits nothing is parsed: the result is pending, with spans carried by line text
/// from `previous`, for `finish` to complete.
pub fn highlight_diff(path: &str, diff: &FileDiff, previous: Option<&FileDiff>) -> Option<DiffHighlight> {
    if diff.binary || diff.hunks.is_empty() {
        return None;
    }
    // The file's first line (for shebang scripts), from the new side or, for a deleted file,
    // the old side.
    let first_of = |origins: &[char], number: fn(&DiffLine) -> Option<u32>| {
        diff.hunks[0].lines.iter().find(|l| origins.contains(&l.origin)).filter(|l| number(l) == Some(1)).map(|l| l.text.as_str())
    };
    let first_line = first_of(&[' ', '+'], |l| l.new).or_else(|| first_of(&[' ', '-'], |l| l.old));
    let syntax = find_syntax(path, first_line)?;

    let prev = Previous::new(previous, &syntax.name);
    let exact: Vec<_> = diff.hunks.iter().map(|h| prev.exact(h)).collect();
    let cost = Cost::of(diff.hunks.iter().zip(&exact).filter(|(_, e)| e.is_none()).map(|(h, _)| h));
    if !cost.within(BACKGROUND_MAX_LINES, BACKGROUND_MAX_BYTES) {
        return None;
    }
    let n = diff.hunks.len();
    let (lines, pending) = if cost.within(MAX_LINES, MAX_BYTES) {
        let mut cx = Classifier::new();
        let parsed = diff.hunks.iter().zip(exact).map(|(hunk, e)| match e {
            Some(spans) => Ok(spans.clone()),
            None => parse_hunk(syntax, hunk, &mut cx, &|| false),
        });
        (parsed.collect::<Result<Vec<_>, Stop>>().ok()?, vec![false; n])
    } else {
        diff.hunks.iter().zip(exact).map(|(hunk, e)| e.map_or_else(|| (prev.carry(hunk), true), |s| (s.clone(), false))).unzip()
    };
    Some(DiffHighlight { syntax: syntax.name.clone(), lines, pending })
}

/// Finishes a highlight left pending by `highlight_diff`: parses its pending hunks and keeps the
/// others. Long lines stay plain, and a hunk that fails to parse is left plain. `None` when
/// `cancelled` returns true (it is checked every few hundred lines), or when `diff` has no
/// pending highlight.
pub fn finish(diff: &FileDiff, cancelled: &dyn Fn() -> bool) -> Option<DiffHighlight> {
    let hl = diff.highlight.as_ref().filter(|h| h.is_pending())?;
    if cancelled() {
        return None;
    }
    let pending_hunks = || diff.hunks.iter().enumerate().filter(|&(i, _)| hl.hunk_pending(i)).map(|(_, h)| h);
    // Unknown syntax, or (as a defence) too much to parse: the pending hunks end up plain.
    let syntax = SYNTAXES
        .set
        .find_syntax_by_name(&hl.syntax)
        .filter(|_| Cost::of(pending_hunks()).within(BACKGROUND_MAX_LINES, BACKGROUND_MAX_BYTES));
    let mut cx = Classifier::new();
    let mut lines = Vec::with_capacity(hl.lines.len());
    for (i, spans) in hl.lines.iter().enumerate() {
        let Some(hunk) = diff.hunks.get(i).filter(|_| hl.hunk_pending(i)) else {
            lines.push(spans.clone());
            continue;
        };
        match syntax.map_or(Err(Stop::Parse), |s| parse_hunk(s, hunk, &mut cx, cancelled)) {
            Ok(spans) => lines.push(spans),
            Err(Stop::Cancelled) => return None,
            Err(Stop::Parse) => lines.push(vec![Vec::new(); hunk.lines.len()]),
        }
    }
    let n = lines.len();
    Some(DiffHighlight { syntax: hl.syntax.clone(), lines, pending: vec![false; n] })
}

/// Why parsing stopped early.
enum Stop {
    Parse,
    Cancelled,
}

/// Spans for one hunk: the old side, then the new side, each from a fresh parse state.
fn parse_hunk(syntax: &SyntaxReference, hunk: &Hunk, cx: &mut Classifier, cancelled: &dyn Fn() -> bool) -> Result<Vec<Vec<Span>>, Stop> {
    let mut spans = vec![Vec::new(); hunk.lines.len()];
    // Old side: context and removed lines; only the removed lines keep these colors.
    pass(syntax, &hunk.lines, &mut spans, cx, |o| matches!(o, ' ' | '-'), |o| o == '-', cancelled)?;
    // New side: context and added lines; context lines are colored from this side.
    pass(syntax, &hunk.lines, &mut spans, cx, |o| matches!(o, ' ' | '+'), |_| true, cancelled)?;
    Ok(spans)
}

/// Parses the lines whose origin passes `include` from a fresh state, storing spans for those
/// that also pass `keep`. EOF markers (`<`, `>`, `=`) are never included. Lines over
/// `MAX_LINE_BYTES` are skipped and keep no spans; parsing goes on as if they were absent, so a
/// long line that opens a string or a comment isn't seen (an accepted approximation).
fn pass(
    syntax: &SyntaxReference,
    lines: &[DiffLine],
    out: &mut [Vec<Span>],
    cx: &mut Classifier,
    include: impl Fn(char) -> bool,
    keep: impl Fn(char) -> bool,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Stop> {
    let set = &SYNTAXES.set;
    let mut state = ParseState::new(syntax);
    let mut stack = ScopeStack::new();
    let included = lines.iter().enumerate().filter(|(_, l)| include(l.origin) && l.text.len() <= MAX_LINE_BYTES);
    for (i, dl) in included {
        cx.parsed += 1;
        if cx.parsed.is_multiple_of(256) && cancelled() {
            return Err(Stop::Cancelled);
        }
        let text = &dl.text;
        let ops = state.parse_line(&format!("{text}\n"), set).map_err(|_| Stop::Parse)?;
        let mut spans = Vec::new();
        let mut pos = 0;
        for (off, op) in ops {
            if off > pos {
                push(&mut spans, pos, off.min(text.len()), cx.classify(stack.as_slice()));
            }
            stack.apply(&op).map_err(|_| Stop::Parse)?;
            pos = pos.max(off);
        }
        push(&mut spans, pos.min(text.len()), text.len(), cx.classify(stack.as_slice()));
        if keep(dl.origin) {
            out[i] = spans;
        }
    }
    Ok(())
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
struct Classifier {
    rules: &'static [(Scope, TokenKind)],
    cache: HashMap<Scope, Option<TokenKind>>,
    parsed: usize,
}

impl Classifier {
    fn new() -> Self {
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

#[cfg(test)]
mod background_tests;
#[cfg(test)]
mod tests;
