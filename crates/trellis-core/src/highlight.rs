//! Syntax highlighting of diff lines with syntect, reduced to a few token kinds that the UI maps
//! to its own palette (no syntect themes). It runs where diffs are loaded, off the UI thread.
//!
//! The old file (context and `-` lines) and the new file (context and `+` lines) are parsed in
//! separate passes. Hunks are parsed in context: `repo::load_diff_for_view` loads the old file's
//! text along with the diff (`OldText`), and each side's parse state starts at line 1, runs
//! through the unchanged lines before each hunk (the same on both sides, so taken from the old
//! text) and carries on from hunk to hunk. So a hunk that starts inside a block comment or a
//! multi-line string is colored as one from its first line. The lines before the hunks count
//! toward the limits below. Without old text (added or binary files, a blob that doesn't match
//! the hunks) or with too much of it, each hunk is parsed on its own from a fresh state; that
//! per-hunk parse also gives the provisional spans shown while an in-context pass is pending.
//! The whole-file view (`whole_file`) needs no old text: its one hunk is the whole file.
//!
//! Two passes. `highlight_in_context` parses small diffs while they load (the inline pass,
//! within `MAX_LINES` and `MAX_BYTES`). Larger ones come back pending: their hunks carry
//! provisional spans from the diff shown before (matched by line text, see `reuse`), from a
//! per-hunk parse, or none, and `finish` parses them later on a worker thread (the background
//! pass, within `BACKGROUND_MAX_LINES` and `BACKGROUND_MAX_BYTES`). Beyond that a diff stays
//! plain. Lines over `MAX_LINE_BYTES` (minified files) are never parsed in either pass and stay
//! plain.

use crate::repo::{DiffLine, FileDiff};
use parse::{Classifier, Stop, parse_hunk, parse_in_context};
use reuse::{Cost, Previous};
use std::sync::{Arc, LazyLock};
use syntect::parsing::{Scope, SyntaxReference, SyntaxSet};

mod context;
mod parse;
mod reuse;

pub use context::OldText;

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
/// Old blobs larger than this (bytes) aren't read for highlighting in context.
pub const OLD_TEXT_MAX_BYTES: usize = 2 * BACKGROUND_MAX_BYTES;

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
    /// Per hunk: its spans are provisional (carried from the diff shown before, parsed without
    /// context, or empty) and `finish` still has to parse it. Empty means nothing is pending.
    pub(crate) pending: Vec<bool>,
    /// The old text the spans were (or, while pending, will be) computed from; `None` when each
    /// hunk is parsed on its own.
    pub(crate) old_text: Option<Arc<OldText>>,
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

    /// Computed on the same basis: the same syntax, and the same old text (or none for both).
    pub fn same_basis(&self, other: &DiffHighlight) -> bool {
        self.syntax == other.syntax && self.old_text == other.old_text
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

/// Token spans for every line of `diff`, each hunk parsed on its own (no old text): see
/// `highlight_in_context`.
pub fn highlight_diff(path: &str, diff: &FileDiff, previous: Option<&FileDiff>) -> Option<DiffHighlight> {
    highlight_in_context(path, diff, previous, None)
}

/// Token spans for every line of `diff`, or `None` to show it plain: binary, empty, an unknown
/// language, too large even for the background pass (`BACKGROUND_MAX_LINES`,
/// `BACKGROUND_MAX_BYTES`), or a parse error in the inline pass.
///
/// With `old` (the old file's text, from `repo::load_diff_for_view`), the hunks are parsed in
/// context and the lines before them count toward the limits. Over the inline limits the result
/// is pending, with provisional per-hunk spans, for `finish` to parse in context. Without `old`,
/// or with too much context even for the background pass, each hunk is parsed on its own.
///
/// `previous` is the diff shown before a reload. In context, the same hunks with the same old
/// text keep its finished spans. Per hunk, hunks whose lines are unchanged keep its finished
/// spans instead of being parsed again (their spans depend only on their own lines), and only
/// the other hunks count toward the inline limits. Over those limits nothing is parsed: the
/// result is pending, with spans carried by line text from `previous`.
pub fn highlight_in_context(path: &str, diff: &FileDiff, previous: Option<&FileDiff>, old: Option<&Arc<OldText>>) -> Option<DiffHighlight> {
    if diff.binary || diff.hunks.is_empty() {
        return None;
    }
    // The file's first line (for shebang scripts), from the new side or, for a deleted file,
    // the old side; else from the old text.
    let first_of = |origins: &[char], number: fn(&DiffLine) -> Option<u32>| {
        diff.hunks[0].lines.iter().find(|l| origins.contains(&l.origin)).filter(|l| number(l) == Some(1)).map(|l| l.text.as_str())
    };
    let first_line = first_of(&[' ', '+'], |l| l.new).or_else(|| first_of(&[' ', '-'], |l| l.old)).or_else(|| old.and_then(|o| o.first()));
    let syntax = find_syntax(path, first_line)?;

    let ctx = old
        .and_then(|o| Some((o, o.gaps(&diff.hunks)?)))
        .filter(|(_, gaps)| context_cost(diff, gaps).within(BACKGROUND_MAX_LINES, BACKGROUND_MAX_BYTES));
    let Some((o, gaps)) = ctx else { return per_hunk(syntax, diff, previous) };
    // A reload with the same hunks and old text (e.g. a file watcher's): the spans can't differ.
    let unchanged = previous
        .filter(|p| p.hunks == diff.hunks)
        .and_then(|p| p.highlight.as_ref())
        .filter(|h| !h.is_pending() && h.syntax == syntax.name && h.old_text.as_deref() == Some(&**o));
    if let Some(h) = unchanged {
        return Some(h.clone());
    }
    let n = diff.hunks.len();
    if context_cost(diff, &gaps).within(MAX_LINES, MAX_BYTES) {
        let lines = parse_in_context(syntax, &diff.hunks, &gaps, &mut Classifier::new(), &|| false).ok()?;
        return Some(DiffHighlight { syntax: syntax.name.clone(), lines, pending: vec![false; n], old_text: Some(o.clone()) });
    }
    // Pending: provisional spans from the per-hunk parse (exact reuse, an inline parse when the
    // hunks alone are small enough, or carried by line text), all re-parsed in context later.
    let mut hl = per_hunk(syntax, diff, previous)?;
    hl.pending = vec![true; n];
    hl.old_text = Some(o.clone());
    Some(hl)
}

/// Lines and bytes to parse for every hunk in context.
fn context_cost(diff: &FileDiff, gaps: &[&[String]]) -> Cost {
    Cost::of(diff.hunks.iter()).plus_gaps(gaps)
}

/// Each hunk parsed on its own, reusing `previous`'s finished spans for unchanged hunks.
fn per_hunk(syntax: &SyntaxReference, diff: &FileDiff, previous: Option<&FileDiff>) -> Option<DiffHighlight> {
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
    Some(DiffHighlight { syntax: syntax.name.clone(), lines, pending, old_text: None })
}

/// Finishes a highlight left pending by `highlight_in_context`. With old text, every hunk is
/// parsed in context; when that can't be done (the hunks don't fit the text, too much to parse,
/// a parse error), each pending hunk is parsed on its own instead. Without old text, the pending
/// hunks are parsed on their own and the others kept. Long lines stay plain, and a hunk that
/// fails to parse on its own is left plain. `None` when `cancelled` returns true (it is checked
/// every few hundred lines), or when `diff` has no pending highlight.
pub fn finish(diff: &FileDiff, cancelled: &dyn Fn() -> bool) -> Option<DiffHighlight> {
    let hl = diff.highlight.as_ref().filter(|h| h.is_pending())?;
    if cancelled() {
        return None;
    }
    let n = hl.lines.len();
    if let Some(o) = hl.old_text.as_ref()
        && let Some(gaps) = o.gaps(&diff.hunks).filter(|g| context_cost(diff, g).within(BACKGROUND_MAX_LINES, BACKGROUND_MAX_BYTES))
        && let Some(syntax) = SYNTAXES.set.find_syntax_by_name(&hl.syntax)
    {
        match parse_in_context(syntax, &diff.hunks, &gaps, &mut Classifier::new(), cancelled) {
            Ok(lines) => {
                return Some(DiffHighlight { syntax: hl.syntax.clone(), lines, pending: vec![false; n], old_text: Some(o.clone()) });
            }
            Err(Stop::Cancelled) => return None,
            Err(Stop::Parse) => {}
        }
    }
    let pending_hunks = || diff.hunks.iter().enumerate().filter(|&(i, _)| hl.hunk_pending(i)).map(|(_, h)| h);
    // Unknown syntax, or (as a defence) too much to parse: the pending hunks end up plain.
    let syntax = SYNTAXES
        .set
        .find_syntax_by_name(&hl.syntax)
        .filter(|_| Cost::of(pending_hunks()).within(BACKGROUND_MAX_LINES, BACKGROUND_MAX_BYTES));
    let mut cx = Classifier::new();
    let mut lines = Vec::with_capacity(n);
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
    Some(DiffHighlight { syntax: hl.syntax.clone(), lines, pending: vec![false; n], old_text: None })
}

#[cfg(test)]
mod background_tests;
#[cfg(test)]
mod context_tests;
#[cfg(test)]
mod tests;
