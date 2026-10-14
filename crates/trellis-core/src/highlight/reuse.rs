//! What a reload can take over from the diff shown before: the exact spans of unchanged hunks,
//! and provisional spans for lines whose text appeared before. Plus the cost of what is left to
//! parse.

use super::{MAX_LINE_BYTES, Span};
use crate::repo::{FileDiff, Hunk};
use std::collections::HashMap;

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
}

impl<'a> Previous<'a> {
    pub(super) fn new(previous: Option<&'a FileDiff>, syntax: &str) -> Previous<'a> {
        let mut prev = Previous::default();
        let Some((diff, hl)) = previous.and_then(|p| p.highlight.as_ref().filter(|h| h.syntax == syntax).map(|h| (p, h))) else {
            return prev;
        };
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
}

/// What a hunk's spans depend on: its lines' origins and text (not their line numbers, which
/// shift when other hunks are staged).
fn hunk_key(hunk: &Hunk) -> Vec<(char, &str)> {
    hunk.lines.iter().map(|l| (l.origin, l.text.as_str())).collect()
}
