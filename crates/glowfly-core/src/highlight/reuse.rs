//! What a reload can take over from the diff shown before: the exact spans of unchanged hunks,
//! provisional spans for lines whose text appeared before, hunk-header spans by their function
//! context's text, and parse checkpoints (`memo`) to resume an in-context parse from. Plus the
//! cost of what is left to parse.

use super::memo::{Memo, streams, unseen};
use super::{MAX_LINE_BYTES, Span, context_start};
use crate::repo::{FileDiff, Hunk};
use std::collections::{HashMap, HashSet};

/// Lines and bytes to parse. Lines over `MAX_LINE_BYTES` are never parsed, so they don't count.
pub(super) struct Cost {
    lines: usize,
    bytes: usize,
}

impl Cost {
    pub(super) fn of<'a>(hunks: impl Iterator<Item = &'a Hunk>) -> Cost {
        let mut cost = Cost { lines: 0, bytes: 0 };
        let parsed = hunks.flat_map(|h| &h.lines).filter(|l| matches!(l.origin, ' ' | '+' | '-') && l.text.len() <= MAX_LINE_BYTES);
        for l in parsed {
            cost.lines += 1;
            cost.bytes += l.text.len();
        }
        cost
    }

    /// Adds the unchanged lines before each hunk, parsed for an in-context highlight. They count
    /// once: while both sides are in the same state (the usual case) a gap is parsed once. When a
    /// hunk leaves the sides in different states, the later gaps are parsed twice, so the true
    /// cost can reach about twice this.
    pub(super) fn plus_gaps(mut self, gaps: &[&[String]]) -> Cost {
        for l in gaps.iter().flat_map(|g| g.iter()).filter(|l| l.len() <= MAX_LINE_BYTES) {
            self.lines += 1;
            self.bytes += l.len();
        }
        self
    }

    /// The least an in-context parse resuming from `memo` has to parse: the chunks of both
    /// sides whose text `memo` hasn't seen, each counted once.
    pub(super) fn unseen_in_context(diff: &FileDiff, gaps: &[&[String]], memo: &Memo) -> Cost {
        let s = streams(&diff.hunks, gaps);
        let mut seen = HashSet::new();
        let (old_lines, old_bytes) = unseen(&s.old.lines, memo, &mut seen);
        let (new_lines, new_bytes) = unseen(&s.new.lines, memo, &mut seen);
        Cost { lines: old_lines + new_lines, bytes: old_bytes + new_bytes }
    }

    pub(super) fn within(&self, max_lines: usize, max_bytes: usize) -> bool {
        self.lines <= max_lines && self.bytes <= max_bytes
    }
}

/// The spans of the diff shown before a reload. Empty when it had no highlight or one in
/// another language.
#[derive(Default)]
pub(super) struct Previous<'a> {
    /// Finished hunks by their lines (pending hunks hold provisional spans: never reused exactly).
    exact: HashMap<Vec<(char, &'a str)>, &'a Vec<Vec<Span>>>,
    /// Spans by (removed line, text), over every line of every hunk; the first occurrence wins.
    /// Context and added lines key on the new side, as context is colored from the new side;
    /// removed lines key on the old side.
    carry: HashMap<(bool, &'a str), &'a [Span]>,
    /// Hunk-header spans by function context text (pending ones too: they depend only on the
    /// text and the syntax); the first occurrence wins.
    headers: HashMap<&'a str, &'a [Span]>,
}

impl<'a> Previous<'a> {
    pub(super) fn new(previous: Option<&'a FileDiff>, syntax: &str) -> Previous<'a> {
        let mut prev = Previous::default();
        let Some((diff, hl)) = previous.and_then(|p| p.highlight.as_ref().filter(|h| h.syntax == syntax).map(|h| (p, h))) else {
            return prev;
        };
        for (hunk, spans) in diff.hunks.iter().zip(&hl.headers).filter(|(_, s)| !s.is_empty()) {
            if let Some(at) = context_start(&hunk.header) {
                prev.headers.entry(&hunk.header[at..]).or_insert(spans.as_slice());
            }
        }
        for (i, (hunk, spans)) in diff.hunks.iter().zip(&hl.lines).enumerate() {
            if !hl.hunk_pending(i) {
                prev.exact.entry(hunk_key(hunk)).or_insert(spans);
            }
            for (line, s) in hunk.lines.iter().zip(spans) {
                // A line without spans (not parsed yet, too long, empty) has nothing to carry.
                if matches!(line.origin, ' ' | '+' | '-') && !s.is_empty() {
                    prev.carry.entry((line.origin == '-', line.text.as_str())).or_insert(s.as_slice());
                }
            }
        }
        prev
    }

    /// The finished spans of a hunk with exactly these lines, if there was one. They also serve as
    /// provisional spans while an in-context pass is pending.
    pub(super) fn exact(&self, hunk: &Hunk) -> Option<&'a Vec<Vec<Span>>> {
        self.exact.get(&hunk_key(hunk)).copied()
    }

    /// Provisional spans for `hunk`: each line takes the spans of a previous line on the same
    /// side with the same text (so the byte offsets still fit), or none. EOF markers get none.
    pub(super) fn carry(&self, hunk: &Hunk) -> Vec<Vec<Span>> {
        let spans = |l: &crate::repo::DiffLine| match l.origin {
            ' ' | '+' | '-' => self.carry.get(&(l.origin == '-', l.text.as_str())).map_or_else(Vec::new, |s| s.to_vec()),
            _ => Vec::new(),
        };
        hunk.lines.iter().map(spans).collect()
    }

    /// Provisional spans for `hunk`'s header context: those of a previous header with the same
    /// context text (the spans are relative to the context, so they still fit), or none.
    pub(super) fn header(&self, hunk: &Hunk) -> Vec<Span> {
        let context = context_start(&hunk.header).map(|at| &hunk.header[at..]);
        context.and_then(|c| self.headers.get(c)).map_or_else(Vec::new, |s| s.to_vec())
    }
}

/// What a hunk's spans depend on: its lines' origins and text (not their line numbers, which
/// shift when other hunks are staged).
fn hunk_key(hunk: &Hunk) -> Vec<(char, &str)> {
    hunk.lines.iter().map(|l| (l.origin, l.text.as_str())).collect()
}
