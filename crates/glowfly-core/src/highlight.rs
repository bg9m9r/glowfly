//! Syntax highlighting of diff lines with syntect, reduced to a few token kinds that the UI maps
//! to its own palette (no syntect themes). It runs where diffs are loaded, off the UI thread.
//! The syntax set is syntect's defaults plus extra bundled definitions (TOML, TypeScript,
//! PowerShell, Dockerfile, Kotlin, Swift and more), dumped at build time (see `syntaxes`). The
//! language comes from a `linguist-language` gitattribute override (`SyntaxHint::language`) when
//! it names a bundled syntax, else it is detected from the path or the first line.
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
//! The whole-file view (`whole_file`) needs no old text: its one hunk is the whole file. Whole
//! files outside a diff (the blame view's lines) go through `highlight_file` (`file`).
//!
//! Two passes. `highlight_in_context` parses small diffs while they load (the inline pass,
//! within `MAX_LINES` and `MAX_BYTES`). An in-context parse keeps checkpoints (`memo`): each
//! chunk of the old and new files' lines with the parse states before and after it. A reload of
//! the same file (a hunk staged, unstaged or discarded, an edit far from the other hunks)
//! resumes from them, parsing only the chunks it hasn't seen from the same state, so it usually
//! finishes inline whatever the file's size, with the colours of a fresh parse. Larger ones come
//! back pending: their hunks carry
//! provisional spans from the diff shown before (matched by line text, see `reuse`), from a
//! per-hunk parse, or none, and `finish` parses them later on a worker thread (the background
//! pass, within `BACKGROUND_MAX_LINES` and `BACKGROUND_MAX_BYTES`). Beyond that a diff stays
//! plain. Lines over `MAX_LINE_BYTES` (minified files) are never parsed in either pass and stay
//! plain.
//!
//! Hunk headers' function context (`@@ -a,b +c,d @@ fn main() {`) is highlighted too, each on
//! its own from a fresh state (`header`): inline with the lines, or by `finish` when pending.
//! A reload carries those spans by the context's text, so they survive changed line numbers.

use crate::repo::{DiffLine, FileDiff};
use header::parse_headers;
use memo::Memo;
use parse::{Classifier, Stop, parse_hunk, parse_in_context};
use reuse::{Cost, Previous};
use std::fmt;
use std::sync::Arc;
use syntaxes::{SYNTAXES, find_syntax};
use syntect::parsing::SyntaxReference;

pub(crate) mod context;
mod file;
mod gaps;
mod header;
mod linguist;
mod memo;
mod parse;
mod reuse;
mod syntaxes;

pub use context::OldText;
pub use file::highlight_file;
pub use gaps::gap_spans;
pub use header::context_start;
pub use linguist::SyntaxHint;
pub use syntaxes::syntax_name;

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
    /// Per hunk: spans of the function context in the hunk header, relative to
    /// `context_start(header)`. Empty when the header has none, it is too long, or it isn't
    /// parsed yet (pending without carried spans).
    pub(crate) headers: Vec<Vec<Span>>,
    /// The checkpoints of the in-context parse (while pending, those it will start from), for
    /// the next load of this file.
    pub(crate) memo: Checkpoints,
}

/// Parse checkpoints carried by a highlight (see `memo`). A cache only: it never makes two
/// highlights differ.
#[derive(Clone, Default)]
pub(crate) struct Checkpoints(Option<Arc<Memo>>);

impl PartialEq for Checkpoints {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl fmt::Debug for Checkpoints {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Checkpoints({})", self.len())
    }
}

impl Checkpoints {
    fn new(memo: Memo) -> Self {
        Checkpoints(Some(Arc::new(memo)))
    }

    fn len(&self) -> usize {
        self.0.as_ref().map_or(0, |m| m.len())
    }
}

impl DiffHighlight {
    /// The spans of one line; empty when out of range, so a mismatched highlight never panics.
    pub fn line(&self, hunk: usize, line: usize) -> &[Span] {
        self.lines.get(hunk).and_then(|h| h.get(line)).map_or(&[], Vec::as_slice)
    }

    /// The spans of a hunk header's function context, relative to `context_start`; empty when
    /// out of range or not highlighted.
    pub fn header(&self, hunk: usize) -> &[Span] {
        self.headers.get(hunk).map_or(&[], Vec::as_slice)
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

    /// The number of parse checkpoints carried (for logging).
    pub fn checkpoints(&self) -> usize {
        self.memo.len()
    }

    fn hunk_pending(&self, hunk: usize) -> bool {
        self.pending.get(hunk).copied().unwrap_or(false)
    }
}

/// Token spans for every line of `diff`, each hunk parsed on its own (no old text): see
/// `highlight_in_context`.
pub fn highlight_diff<'a>(file: impl Into<SyntaxHint<'a>>, diff: &FileDiff, previous: Option<&FileDiff>) -> Option<DiffHighlight> {
    highlight_in_context(file, diff, previous, None)
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
/// text keep its finished spans, and otherwise the parse resumes from its checkpoints: when the
/// chunks it hasn't seen fit the inline limits, the diff is finished inline (unless reaching
/// them in other states makes the parse go over the limits). Per hunk, hunks whose lines are unchanged keep its finished
/// spans instead of being parsed again (their spans depend only on their own lines), and only
/// the other hunks count toward the inline limits. Over those limits nothing is parsed: the
/// result is pending, with spans carried by line text from `previous`.
pub fn highlight_in_context<'a>(
    file: impl Into<SyntaxHint<'a>>,
    diff: &FileDiff,
    previous: Option<&FileDiff>,
    old: Option<&Arc<OldText>>,
) -> Option<DiffHighlight> {
    if diff.binary || diff.hunks.is_empty() {
        return None;
    }
    // The file's first line (for shebang scripts), from the new side or, for a deleted file,
    // the old side; else from the old text.
    let first_of = |origins: &[char], number: fn(&DiffLine) -> Option<u32>| {
        diff.hunks[0].lines.iter().find(|l| origins.contains(&l.origin)).filter(|l| number(l) == Some(1)).map(|l| l.text.as_str())
    };
    let first_line = first_of(&[' ', '+'], |l| l.new).or_else(|| first_of(&[' ', '-'], |l| l.old)).or_else(|| old.and_then(|o| o.first()));
    let syntax = find_syntax(file.into(), first_line)?;

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
    // The checkpoints of the highlight shown before, finished or pending (a pending one carries
    // those it was built from).
    let memo_in = previous.and_then(|p| p.highlight.as_ref()).filter(|h| h.syntax == syntax.name).and_then(|h| h.memo.0.clone());
    let n = diff.hunks.len();
    let inline = |mut cx: Classifier| {
        let mut memo_out = Memo::default();
        let lines = parse_in_context(syntax, &diff.hunks, &gaps, memo_in.as_deref(), &mut memo_out, &mut cx, &|| false)?;
        cx.unlimited();
        let headers = parse_headers(syntax, &diff.hunks, &mut cx, &|| false).unwrap_or_default();
        let (pending, old_text, memo) = (vec![false; n], Some(o.clone()), Checkpoints::new(memo_out));
        Ok(DiffHighlight { syntax: syntax.name.clone(), lines, pending, old_text, headers, memo })
    };
    if context_cost(diff, &gaps).within(MAX_LINES, MAX_BYTES) {
        return inline(Classifier::new()).ok();
    }
    if let Some(m) = memo_in.as_deref()
        && Cost::unseen_in_context(diff, &gaps, m).within(MAX_LINES, MAX_BYTES)
    {
        match inline(Classifier::with_budget(MAX_LINES, MAX_BYTES)) {
            Ok(hl) => return Some(hl),
            Err(Stop::Budget) => {}
            Err(Stop::Parse | Stop::Cancelled) => return None,
        }
    }
    // Pending: provisional spans from the per-hunk parse (exact reuse, an inline parse when the
    // hunks alone are small enough, or carried by line text), all re-parsed in context later.
    let mut hl = per_hunk(syntax, diff, previous)?;
    hl.pending = vec![true; n];
    hl.old_text = Some(o.clone());
    hl.memo = Checkpoints(memo_in);
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
    let (lines, pending, headers) = if cost.within(MAX_LINES, MAX_BYTES) {
        let mut cx = Classifier::new();
        let parsed = diff.hunks.iter().zip(exact).map(|(hunk, e)| match e {
            Some(spans) => Ok(spans.clone()),
            None => parse_hunk(syntax, hunk, &mut cx, &|| false),
        });
        let lines = parsed.collect::<Result<Vec<_>, Stop>>().ok()?;
        (lines, vec![false; n], parse_headers(syntax, &diff.hunks, &mut cx, &|| false).ok()?)
    } else {
        let (lines, pending) =
            diff.hunks.iter().zip(exact).map(|(hunk, e)| e.map_or_else(|| (prev.carry(hunk), true), |s| (s.clone(), false))).unzip();
        (lines, pending, diff.hunks.iter().map(|h| prev.header(h)).collect())
    };
    Some(DiffHighlight { syntax: syntax.name.clone(), lines, pending, old_text: None, headers, memo: Checkpoints::default() })
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
        let mut cx = Classifier::new();
        let mut memo_out = Memo::default();
        // No budget here, so `Stop::Budget` can't occur; it is treated like a parse error.
        match parse_in_context(syntax, &diff.hunks, &gaps, hl.memo.0.as_deref(), &mut memo_out, &mut cx, cancelled) {
            Ok(lines) => {
                let headers = match parse_headers(syntax, &diff.hunks, &mut cx, cancelled) {
                    Ok(headers) => headers,
                    Err(Stop::Cancelled) => return None,
                    Err(Stop::Parse | Stop::Budget) => vec![Vec::new(); diff.hunks.len()],
                };
                let (old_text, memo) = (Some(o.clone()), Checkpoints::new(memo_out));
                return Some(DiffHighlight { syntax: hl.syntax.clone(), lines, pending: vec![false; n], old_text, headers, memo });
            }
            Err(Stop::Cancelled) => return None,
            Err(Stop::Parse | Stop::Budget) => {}
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
            Err(Stop::Parse | Stop::Budget) => lines.push(vec![Vec::new(); hunk.lines.len()]),
        }
    }
    let headers = match syntax.map(|s| parse_headers(s, &diff.hunks, &mut cx, cancelled)) {
        Some(Ok(headers)) => headers,
        Some(Err(Stop::Cancelled)) => return None,
        Some(Err(Stop::Parse | Stop::Budget)) | None => hl.headers.clone(),
    };
    let memo = Checkpoints::default();
    Some(DiffHighlight { syntax: hl.syntax.clone(), lines, pending: vec![false; n], old_text: None, headers, memo })
}

#[cfg(test)]
mod background_tests;
#[cfg(test)]
pub(crate) mod context_tests;
#[cfg(test)]
mod header_tests;
#[cfg(test)]
mod language_tests;
#[cfg(test)]
mod linguist_tests;
#[cfg(test)]
mod memo_tests;
#[cfg(test)]
mod tests;
