//! Parse checkpoints: the spans and the parse states before and after each chunk of a stream of
//! lines, kept with a finished highlight so the next load of the same file (after staging a
//! hunk, or an edit far from the others) parses only the chunks it hasn't seen.
//!
//! Chunk boundaries depend only on the lines' text (a chunk ends after a line whose hash has its
//! low bits clear, or at `CHUNK_MAX` lines), so the same run of text splits the same way wherever
//! it sits, and chunks line up again right after an inserted block. A chunk is reused only when
//! it is reached in the same parse state it was parsed from, so the spans are always those a
//! fresh parse would give. Hashes come from `DefaultHasher`, deterministic within a process; the
//! memo is never saved.

use super::parse::{Classifier, Side, Stop};
use super::{MAX_LINE_BYTES, Span};
use crate::repo::Hunk;
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;
use syntect::parsing::SyntaxReference;

/// A chunk ends after a line whose hash has these bits clear (about one line in 64).
pub(super) const CHUNK_MASK: u64 = 63;
/// A chunk ends after this many lines at the latest.
pub(super) const CHUNK_MAX: usize = 256;

pub(super) fn line_hash(line: &str) -> u64 {
    let mut h = DefaultHasher::new();
    line.hash(&mut h);
    h.finish()
}

/// The chunks of `lines`, in order and covering them all.
#[cfg(test)]
pub(super) fn chunks(lines: &[&str]) -> Vec<Range<usize>> {
    keyed(lines).into_iter().map(|(r, _)| r).collect()
}

/// What a chunk's spans depend on (with the state it starts in): its lines' text.
#[derive(Hash, Eq, PartialEq, Clone, Copy, Debug)]
pub(super) struct ChunkKey {
    hash: u64,
    lines: u32,
    bytes: u32,
}

/// The chunks of `lines` with their keys.
fn keyed(lines: &[&str]) -> Vec<(Range<usize>, ChunkKey)> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut combined = DefaultHasher::new();
    let mut bytes = 0usize;
    for (i, line) in lines.iter().enumerate() {
        let h = line_hash(line);
        combined.write_u64(h);
        bytes += line.len();
        if h & CHUNK_MASK == 0 || i + 1 - start >= CHUNK_MAX || i + 1 == lines.len() {
            let key = ChunkKey {
                hash: combined.finish(),
                lines: u32::try_from(i + 1 - start).unwrap_or(u32::MAX),
                bytes: u32::try_from(bytes).unwrap_or(u32::MAX),
            };
            out.push((start..i + 1, key));
            start = i + 1;
            combined = DefaultHasher::new();
            bytes = 0;
        }
    }
    out
}

/// One parsed chunk: the state it started in, the state it left, and one span list per line
/// (empty for lines over `MAX_LINE_BYTES`, which are never parsed).
struct Entry {
    start: Side,
    end: Side,
    spans: Arc<[Vec<Span>]>,
}

/// Parsed chunks by key; a key can have entries from several start states.
#[derive(Default)]
pub(super) struct Memo {
    entries: HashMap<ChunkKey, Vec<Entry>>,
}

impl Memo {
    /// The number of chunks held.
    pub(super) fn len(&self) -> usize {
        self.entries.values().map(Vec::len).sum()
    }

    fn find(&self, key: &ChunkKey, start: &Side) -> Option<&Entry> {
        self.entries.get(key)?.iter().find(|e| e.start == *start)
    }

    fn insert(&mut self, key: ChunkKey, entry: Entry) {
        if self.find(&key, &entry.start).is_none() {
            self.entries.entry(key).or_default().push(entry);
        }
    }
}

/// Spans for each of `lines`, parsed in order from the top of a file. A chunk found in
/// `memo_in`, then `memo_out`, with the current state as its start takes the spans and end state
/// stored there (nothing is parsed or counted); others are parsed. Every chunk passed is
/// recorded in `memo_out`.
pub(super) fn parse_stream(
    syntax: &SyntaxReference,
    lines: &[&str],
    memo_in: Option<&Memo>,
    memo_out: &mut Memo,
    cx: &mut Classifier,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<Span>>, Stop> {
    let mut side = Side::new(syntax);
    let mut out = Vec::with_capacity(lines.len());
    for (range, key) in keyed(lines) {
        let hit = memo_in.and_then(|m| m.find(&key, &side)).or_else(|| memo_out.find(&key, &side));
        let (end, spans) = match hit {
            Some(e) => (e.end.clone(), e.spans.clone()),
            None => {
                let mut spans = Vec::with_capacity(range.len());
                let mut state = side.clone();
                for line in &lines[range] {
                    spans.push(if line.len() <= MAX_LINE_BYTES { state.line(line, cx, cancelled)? } else { Vec::new() });
                }
                (state, Arc::from(spans))
            }
        };
        out.extend(spans.iter().cloned());
        let start = std::mem::replace(&mut side, end.clone());
        memo_out.insert(key, Entry { start, end, spans });
    }
    Ok(out)
}

/// Lines and bytes (lines over `MAX_LINE_BYTES` aside) of the chunks of `lines` whose key isn't in
/// `memo` and wasn't counted into `seen` yet: the least a parse with `memo` has to do (a chunk in
/// the memo may still be reached in another state).
pub(super) fn unseen(lines: &[&str], memo: &Memo, seen: &mut HashSet<ChunkKey>) -> (usize, usize) {
    let (mut n, mut bytes) = (0, 0);
    for (range, key) in keyed(lines) {
        if memo.entries.contains_key(&key) || !seen.insert(key) {
            continue;
        }
        for line in lines[range].iter().filter(|l| l.len() <= MAX_LINE_BYTES) {
            n += 1;
            bytes += line.len();
        }
    }
    (n, bytes)
}

/// One side's stream of lines, with where each came from: `Some((hunk, line))` for a hunk's line,
/// `None` for an unchanged line between hunks.
#[derive(Default)]
pub(super) struct Stream<'a> {
    pub(super) lines: Vec<&'a str>,
    pub(super) at: Vec<Option<(usize, usize)>>,
}

/// The old and new files' streams as far as the last hunk.
pub(super) struct Streams<'a> {
    pub(super) old: Stream<'a>,
    pub(super) new: Stream<'a>,
}

/// Per hunk `h`, `gaps[h]` then the hunk's context and removed lines (old) or its context and
/// added lines (new). EOF markers are in neither.
pub(super) fn streams<'a>(hunks: &'a [Hunk], gaps: &[&'a [String]]) -> Streams<'a> {
    let mut s = Streams { old: Stream::default(), new: Stream::default() };
    for (h, (hunk, gap)) in hunks.iter().zip(gaps).enumerate() {
        for side in [&mut s.old, &mut s.new] {
            side.lines.extend(gap.iter().map(String::as_str));
            side.at.extend(std::iter::repeat_n(None, gap.len()));
        }
        for (l, line) in hunk.lines.iter().enumerate() {
            let (old, new) = match line.origin {
                ' ' => (true, true),
                '-' => (true, false),
                '+' => (false, true),
                _ => continue,
            };
            for side in [old.then_some(&mut s.old), new.then_some(&mut s.new)].into_iter().flatten() {
                side.lines.push(&line.text);
                side.at.push(Some((h, l)));
            }
        }
    }
    s
}
