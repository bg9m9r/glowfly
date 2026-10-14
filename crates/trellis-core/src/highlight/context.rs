//! The old file's lines before a diff's hunks, so each hunk can be parsed from the state those
//! lines leave (a hunk that starts inside a block comment is then colored as a comment).
//! Outside the hunks the old and new files have the same lines, so the old text serves both
//! sides.

use crate::repo::Hunk;
use std::fmt;

/// The old side of a file diff up to its last hunk: the state each hunk's parses start from.
#[derive(Clone, PartialEq, Eq)]
pub struct OldText {
    /// Old lines 1.. up to (not including) the last hunk's first old line, without terminators.
    lines: Vec<String>,
}

impl fmt::Debug for OldText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OldText({} lines)", self.lines.len())
    }
}

impl OldText {
    /// From the old blob's bytes and the hunks loaded against it. Lines are split on `\n` and
    /// read like the diff's lines (lossy UTF-8, `\r` trimmed). `None` when a context or removed
    /// line of a hunk differs from the blob's line with its number (filters, a stale blob), or
    /// the hunks can't be placed. Only the lines before the last hunk are kept: nothing after it
    /// is parsed.
    pub fn new(content: &[u8], hunks: &[Hunk]) -> Option<OldText> {
        let starts = starts(hunks)?;
        let mut raw: Vec<&[u8]> = content.split(|&b| b == b'\n').collect();
        if content.is_empty() || content.ends_with(b"\n") {
            raw.pop();
        }
        let line = |n: usize| raw.get(n).map(|l| String::from_utf8_lossy(l).trim_end_matches(['\n', '\r']).to_owned());
        let numbered = hunks.iter().flat_map(|h| &h.lines).filter(|l| matches!(l.origin, ' ' | '-'));
        for l in numbered {
            let Some(n) = l.old else { continue };
            if n == 0 || line(n as usize - 1).as_deref() != Some(l.text.as_str()) {
                return None;
            }
        }
        let keep = starts.last().copied().unwrap_or(0);
        if keep > raw.len() {
            return None;
        }
        Some(OldText { lines: (0..keep).filter_map(line).collect() })
    }

    /// Line 1, for detecting a script by its shebang when no hunk starts at line 1.
    pub(super) fn first(&self) -> Option<&str> {
        self.lines.first().map(String::as_str)
    }

    /// Per hunk, the old lines between the previous hunk (or the top of the file) and it. `None`
    /// when the hunks don't fit this text.
    pub(super) fn gaps<'a>(&'a self, hunks: &[Hunk]) -> Option<Vec<&'a [String]>> {
        let starts = starts(hunks)?;
        if starts.last().copied().unwrap_or(0) != self.lines.len() {
            return None;
        }
        let mut end = 0;
        let mut gaps = Vec::with_capacity(hunks.len());
        for (hunk, &start) in hunks.iter().zip(&starts) {
            gaps.push(self.lines.get(end..start)?);
            end = start + old_count(hunk);
        }
        Some(gaps)
    }
}

/// Lines of `hunk` on the old side (context and removed lines).
fn old_count(hunk: &Hunk) -> usize {
    hunk.lines.iter().filter(|l| matches!(l.origin, ' ' | '-')).count()
}

/// Old lines before each hunk. From the first line with an old number `n` (`n - 1`), or, with
/// none, the header's `-a,b` (`b` omitted means 1): `a` when `b` is 0 (the hunk inserts after
/// line `a`), else `a - 1`. `None` when a header doesn't parse or the hunks overlap or go back.
fn starts(hunks: &[Hunk]) -> Option<Vec<usize>> {
    let mut starts = Vec::with_capacity(hunks.len());
    let mut end = 0;
    for hunk in hunks {
        let numbered = hunk.lines.iter().find(|l| matches!(l.origin, ' ' | '-')).and_then(|l| l.old);
        let start = match numbered {
            Some(n) => (n as usize).checked_sub(1)?,
            None => header_start(&hunk.header)?,
        };
        if start < end {
            return None;
        }
        starts.push(start);
        end = start + old_count(hunk);
    }
    Some(starts)
}

/// The old lines before a hunk from its `@@ -a,b +c,d @@` header.
fn header_start(header: &str) -> Option<usize> {
    let old = header.strip_prefix("@@ -")?.split(' ').next()?;
    let (a, b) = old.split_once(',').unwrap_or((old, "1"));
    let (a, b): (usize, usize) = (a.parse().ok()?, b.parse().ok()?);
    Some(if b == 0 { a } else { a.checked_sub(1)? })
}
