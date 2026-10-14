//! Word-level changes between the removed and added lines that the split layout pairs, as byte
//! ranges into `DiffLine::text` (the drawn text, with tabs already turned into spaces). Pure: no
//! git, no UI.
//!
//! Each line is cut into tokens (a run of word characters, a run of whitespace, or a single
//! other character), the common prefix and suffix tokens are stripped, and the rest is compared
//! with a longest-common-subsequence table. The trimming makes the common one-word edit O(n);
//! the worst case per pair is a 301x301 table of `u16`, and a per-diff budget caps the total.

use crate::diff_rows::{SplitRow, split_rows};
use crate::repo::FileDiff;
use std::ops::Range;

/// Pairs where either line is longer than this (bytes) get no marks.
pub const MAX_LINE_BYTES: usize = 1_000;
/// Pairs where either side has more tokens than this, after the common prefix and suffix are
/// trimmed, get no marks.
pub const MAX_TOKENS: usize = 300;
/// Per-diff work budget: the sum of `(n+1)*(m+1)` over the comparison tables built. Pairs that
/// would go over it get no marks.
pub const MAX_CELLS: usize = 10_000_000;
/// Pairs whose matched non-whitespace bytes are less than this share of all non-whitespace
/// bytes (both sides together) are too different to mark.
pub const MIN_SIMILARITY: f32 = 0.4;

/// The changed ranges of a removed line and of the added line paired with it.
pub type PairMarks = (Vec<Range<u32>>, Vec<Range<u32>>);

/// Changed byte ranges for every line of a `FileDiff`, indexed like `hunks[h].lines[l]`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Intraline {
    lines: Vec<Vec<Vec<Range<u32>>>>,
}

impl Intraline {
    /// The changed ranges of one line; empty when out of range, so a stale value never panics.
    pub fn line(&self, hunk: usize, line: usize) -> &[Range<u32>] {
        self.lines.get(hunk).and_then(|h| h.get(line)).map_or(&[], Vec::as_slice)
    }

    /// Number of lines per hunk (for checking the shape against the diff).
    pub fn hunk_lens(&self) -> Vec<usize> {
        self.lines.iter().map(Vec::len).collect()
    }
}

/// Word marks for every removed/added pair that the split layout shows side by side.
pub fn intraline(diff: &FileDiff) -> Intraline {
    if diff.binary || diff.hunks.is_empty() {
        return Intraline::default();
    }
    let mut lines: Vec<Vec<Vec<Range<u32>>>> = diff.hunks.iter().map(|h| vec![Vec::new(); h.lines.len()]).collect();
    let mut budget = MAX_CELLS;
    for row in split_rows(diff) {
        let SplitRow::Pair { hunk, old: Some(o), new: Some(n) } = row else { continue };
        let hl = &diff.hunks[hunk].lines;
        if o == n || hl[o].origin != '-' || hl[n].origin != '+' {
            continue;
        }
        if let Some((old, new)) = pair_ranges(&hl[o].text, &hl[n].text, &mut budget) {
            lines[hunk][o] = old;
            lines[hunk][n] = new;
        }
    }
    Intraline { lines }
}

#[derive(Clone, Copy, PartialEq)]
enum Class {
    Word,
    Space,
    Other,
}

fn class(c: char) -> Class {
    if c.is_alphanumeric() || c == '_' {
        Class::Word
    } else if c.is_whitespace() {
        Class::Space
    } else {
        Class::Other
    }
}

/// Token byte ranges: runs of word characters, runs of whitespace, and single other chars.
fn tokens(s: &str) -> Vec<Range<usize>> {
    let mut out: Vec<Range<usize>> = Vec::new();
    let mut prev = None;
    for (i, c) in s.char_indices() {
        let k = class(c);
        let end = i + c.len_utf8();
        match out.last_mut() {
            Some(last) if prev == Some(k) && k != Class::Other => last.end = end,
            _ => out.push(i..end),
        }
        prev = Some(k);
    }
    out
}

fn is_space(t: &str) -> bool {
    t.chars().next().is_some_and(char::is_whitespace)
}

/// The changed ranges of `old` and `new`, or `None` when the pair is identical, too long, too
/// different, or over `budget` (which is reduced by the work done).
pub fn pair_ranges(old: &str, new: &str, budget: &mut usize) -> Option<PairMarks> {
    if old.len() > MAX_LINE_BYTES || new.len() > MAX_LINE_BYTES || old == new {
        return None;
    }
    let (a, b) = (tokens(old), tokens(new));
    let ta = |i: usize| &old[a[i].clone()];
    let tb = |j: usize| &new[b[j].clone()];
    let mut pre = 0;
    while pre < a.len() && pre < b.len() && ta(pre) == tb(pre) {
        pre += 1;
    }
    let mut suf = 0;
    while suf < a.len() - pre && suf < b.len() - pre && ta(a.len() - 1 - suf) == tb(b.len() - 1 - suf) {
        suf += 1;
    }
    let (n, m) = (a.len() - pre - suf, b.len() - pre - suf);
    if n > MAX_TOKENS || m > MAX_TOKENS {
        return None;
    }
    let cells = (n + 1) * (m + 1);
    if cells > *budget {
        return None;
    }
    *budget -= cells;

    // dp[i][j] = LCS(old middle[i..], new middle[j..]).
    let w = m + 1;
    let eq = |i: usize, j: usize| ta(pre + i) == tb(pre + j);
    let mut dp = vec![0u16; (n + 1) * w];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i * w + j] = if eq(i, j) { dp[(i + 1) * w + j + 1] + 1 } else { dp[(i + 1) * w + j].max(dp[i * w + j + 1]) };
        }
    }
    let (mut ca, mut cb) = (vec![false; a.len()], vec![false; b.len()]);
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if eq(i, j) {
            i += 1;
            j += 1;
        } else if dp[(i + 1) * w + j] >= dp[i * w + j + 1] {
            ca[pre + i] = true;
            i += 1;
        } else {
            cb[pre + j] = true;
            j += 1;
        }
    }
    ca[pre + i..pre + n].fill(true);
    cb[pre + j..pre + m].fill(true);

    let nonws = |s: &str, toks: &[Range<usize>], keep: &dyn Fn(usize) -> bool| -> usize {
        toks.iter().enumerate().filter(|&(k, t)| keep(k) && !is_space(&s[t.clone()])).map(|(_, t)| t.len()).sum()
    };
    let common = nonws(old, &a, &|k| !ca[k]);
    let total = nonws(old, &a, &|_| true) + nonws(new, &b, &|_| true);
    if total == 0 || ((2 * common) as f32) < MIN_SIMILARITY * total as f32 {
        return None;
    }
    let (ra, rb) = (ranges(old, &a, &mut ca), ranges(new, &b, &mut cb));
    if ra.is_empty() && rb.is_empty() {
        return None;
    }
    Some((ra, rb))
}

/// Merges changed tokens into ranges: consecutive ones, and across one unchanged whitespace
/// token between two changed ones. Each range is trimmed of surrounding whitespace unless it
/// is all whitespace (an indentation or trailing-space change).
fn ranges(s: &str, toks: &[Range<usize>], changed: &mut [bool]) -> Vec<Range<u32>> {
    for k in 1..toks.len().saturating_sub(1) {
        if !changed[k] && changed[k - 1] && changed[k + 1] && is_space(&s[toks[k].clone()]) {
            changed[k] = true;
        }
    }
    let mut merged: Vec<Range<usize>> = Vec::new();
    for (k, t) in toks.iter().enumerate() {
        if !changed[k] {
            continue;
        }
        match merged.last_mut() {
            Some(last) if k > 0 && changed[k - 1] => last.end = t.end,
            _ => merged.push(t.clone()),
        }
    }
    merged
        .into_iter()
        .map(|r| {
            let t = &s[r.clone()];
            let trimmed = t.trim();
            if trimmed.is_empty() {
                return r.start as u32..r.end as u32;
            }
            let start = r.start + (t.len() - t.trim_start().len());
            let end = r.end - (t.len() - t.trim_end().len());
            start as u32..end as u32
        })
        .collect()
}

#[cfg(test)]
mod tests;
