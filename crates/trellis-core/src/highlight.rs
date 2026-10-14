//! Syntax highlighting of diff lines with syntect, reduced to a few token kinds that the UI maps
//! to its own palette (no syntect themes). It runs where diffs are loaded, off the UI thread.
//!
//! The old file (context and `-` lines) and the new file (context and `+` lines) are parsed in
//! separate passes, and each hunk starts from a fresh parse state. Known limitation: a hunk that
//! starts inside a block comment or a multi-line string is colored as code until the delimiter
//! closes; highlighting from the whole old and new files would fix that. The whole-file view
//! (`whole_file`) does: its one hunk is the whole file, within the size limits below.

use crate::repo::{DiffLine, FileDiff, Hunk};
use std::collections::HashMap;
use std::sync::LazyLock;
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet};

// Parsing costs roughly 90 ms per 1,000 short lines and 1.4 ms per KB of text (release build),
// so these keep the worst case near 150-200 ms. Larger diffs are shown plain.

/// Diffs with more lines than this, in all hunks together, are shown plain.
pub const MAX_LINES: usize = 2_000;
/// Diffs with more text than this (bytes, all lines together) are shown plain.
pub const MAX_BYTES: usize = 100_000;
/// Diffs with any line longer than this (in bytes, e.g. minified files) are shown plain.
pub const MAX_LINE_BYTES: usize = 1_000;

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
    pub(crate) lines: Vec<Vec<Vec<Span>>>,
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

/// Token spans for every line of `diff`, or `None` to show it plain: binary, empty, too large
/// (see `MAX_LINES`, `MAX_BYTES`, `MAX_LINE_BYTES`), an unknown language, or a parse error.
///
/// `previous` is the diff shown before a reload: hunks whose lines are unchanged keep its spans
/// instead of being parsed again (each hunk is parsed on its own, so its spans depend only on
/// its own lines).
pub fn highlight_diff(path: &str, diff: &FileDiff, previous: Option<&FileDiff>) -> Option<DiffHighlight> {
    if diff.binary || diff.hunks.is_empty() {
        return None;
    }
    let all = || diff.hunks.iter().flat_map(|h| &h.lines);
    if all().count() > MAX_LINES || all().map(|l| l.text.len()).sum::<usize>() > MAX_BYTES || all().any(|l| l.text.len() > MAX_LINE_BYTES) {
        return None;
    }
    // The file's first line (for shebang scripts), from the new side or, for a deleted file,
    // the old side.
    let first_of = |origins: &[char], number: fn(&DiffLine) -> Option<u32>| {
        diff.hunks[0].lines.iter().find(|l| origins.contains(&l.origin)).filter(|l| number(l) == Some(1)).map(|l| l.text.as_str())
    };
    let first_line = first_of(&[' ', '+'], |l| l.new).or_else(|| first_of(&[' ', '-'], |l| l.old));
    let syntax = find_syntax(path, first_line)?;

    let reusable: HashMap<Vec<(char, &str)>, &Vec<Vec<Span>>> = previous
        .and_then(|p| p.highlight.as_ref().filter(|h| h.syntax == syntax.name).map(|h| (p, h)))
        .map(|(p, h)| p.hunks.iter().zip(&h.lines).map(|(hunk, spans)| (hunk_key(hunk), spans)).collect())
        .unwrap_or_default();
    let mut cx = Classifier { rules: &SYNTAXES.rules, cache: HashMap::new() };
    let mut lines = Vec::with_capacity(diff.hunks.len());
    for hunk in &diff.hunks {
        if let Some(spans) = reusable.get(&hunk_key(hunk)) {
            lines.push((*spans).clone());
            continue;
        }
        let mut spans = vec![Vec::new(); hunk.lines.len()];
        // Old side: context and removed lines; only the removed lines keep these colors.
        pass(syntax, &hunk.lines, &mut spans, &mut cx, |o| matches!(o, ' ' | '-'), |o| o == '-')?;
        // New side: context and added lines; context lines are colored from this side.
        pass(syntax, &hunk.lines, &mut spans, &mut cx, |o| matches!(o, ' ' | '+'), |_| true)?;
        lines.push(spans);
    }
    Some(DiffHighlight { syntax: syntax.name.clone(), lines })
}

/// What a hunk's spans depend on: its lines' origins and text (not their line numbers, which
/// shift when other hunks are staged).
fn hunk_key(hunk: &Hunk) -> Vec<(char, &str)> {
    hunk.lines.iter().map(|l| (l.origin, l.text.as_str())).collect()
}

/// Parses the lines whose origin passes `include` from a fresh state, storing spans for those
/// that also pass `keep`. EOF markers (`<`, `>`, `=`) are never included.
fn pass(
    syntax: &SyntaxReference,
    lines: &[DiffLine],
    out: &mut [Vec<Span>],
    cx: &mut Classifier,
    include: impl Fn(char) -> bool,
    keep: impl Fn(char) -> bool,
) -> Option<()> {
    let set = &SYNTAXES.set;
    let mut state = ParseState::new(syntax);
    let mut stack = ScopeStack::new();
    for (i, dl) in lines.iter().enumerate().filter(|(_, l)| include(l.origin)) {
        let text = &dl.text;
        let ops = state.parse_line(&format!("{text}\n"), set).ok()?;
        let mut spans = Vec::new();
        let mut pos = 0;
        for (off, op) in ops {
            if off > pos {
                push(&mut spans, pos, off.min(text.len()), cx.classify(stack.as_slice()));
            }
            stack.apply(&op).ok()?;
            pos = pos.max(off);
        }
        push(&mut spans, pos.min(text.len()), text.len(), cx.classify(stack.as_slice()));
        if keep(dl.origin) {
            out[i] = spans;
        }
    }
    Some(())
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

/// Maps a scope stack to a token kind, caching per scope.
struct Classifier<'a> {
    rules: &'a [(Scope, TokenKind)],
    cache: HashMap<Scope, Option<TokenKind>>,
}

impl Classifier<'_> {
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
mod tests;
