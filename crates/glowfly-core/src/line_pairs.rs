//! Which removed line sits beside which added line in a change run of the split layout, chosen
//! by token similarity (the measure `intraline` marks words with), with limits past which a run
//! keeps the plain positional rows. Pure: no git, no UI.
//!
//! In each run the removed and added lines are aligned without crossing so the total similarity
//! of the matched pairs (anchors) is as high as possible. `diff_rows::split_rows_with` lays the
//! lines between anchors out by position.

use crate::diff_rows::change_runs;
use crate::intraline::{self, MIN_SIMILARITY};
use crate::repo::{DiffLine, FileDiff};
use std::ops::Range;

/// Runs whose removed and added candidate counts multiply to more than this keep positional rows.
pub const MAX_RUN_PAIRS: usize = 40_000;
/// Once this many candidate pairs have been scored in a diff, later runs keep positional rows.
pub const MAX_DIFF_PAIRS: usize = 400_000;
/// Per-diff budget of LCS table cells for scoring. The run that runs out, and every later run,
/// keeps positional rows.
pub const MAX_PAIR_CELLS: usize = 4_000_000;

/// The work limits of one pairing (small ones in tests).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    pub(crate) run_pairs: usize,
    pub(crate) diff_pairs: usize,
    pub(crate) cells: usize,
}

impl Limits {
    pub(crate) const DEFAULT: Limits = Limits { run_pairs: MAX_RUN_PAIRS, diff_pairs: MAX_DIFF_PAIRS, cells: MAX_PAIR_CELLS };
}

/// The anchors of each hunk: (removed line, added line) indices into `hunks[h].lines`,
/// increasing in both. Lines not in an anchor are paired by position between the anchors.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LinePairs {
    hunks: Vec<Vec<(u32, u32)>>,
}

impl LinePairs {
    /// The anchors of hunk `h`; empty when out of range.
    pub fn hunk(&self, h: usize) -> &[(u32, u32)] {
        self.hunks.get(h).map_or(&[], Vec::as_slice)
    }

    /// The number of anchors in all hunks.
    pub fn len(&self) -> usize {
        self.hunks.iter().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether these pairs can belong to `d`: one entry per hunk, each anchor a `-` line before
    /// a `+` line, increasing in both. A stale or default value fails this for a non-empty diff.
    pub fn fits(&self, d: &FileDiff) -> bool {
        self.hunks.len() == d.hunks.len()
            && self.hunks.iter().zip(&d.hunks).all(|(anchors, hunk)| {
                let line = |i: u32| hunk.lines.get(i as usize).map(|l| l.origin);
                let valid = anchors.iter().all(|&(o, n)| o < n && line(o) == Some('-') && line(n) == Some('+'));
                valid && anchors.windows(2).all(|w| w[0].0 < w[1].0 && w[0].1 < w[1].1)
            })
    }
}

/// The removed and added lines of `d` paired by similarity, within the default limits.
pub fn pair_lines(d: &FileDiff) -> LinePairs {
    pair_lines_with(d, Limits::DEFAULT)
}

pub(crate) fn pair_lines_with(d: &FileDiff, limits: Limits) -> LinePairs {
    if d.binary || d.hunks.is_empty() {
        return LinePairs::default();
    }
    let mut work = Work { limits, scored: 0, cells: limits.cells, stopped: false };
    let hunks = d
        .hunks
        .iter()
        .map(|hunk| change_runs(&hunk.lines).into_iter().flat_map(|(old, new)| work.run(&hunk.lines, old, new)).collect())
        .collect();
    LinePairs { hunks }
}

/// The budgets left for one diff. `stopped` once a limit is reached: later runs get no anchors.
struct Work {
    limits: Limits,
    scored: usize,
    cells: usize,
    stopped: bool,
}

/// A candidate line's non-whitespace byte count and its non-whitespace tokens, sorted.
struct Profile<'a> {
    bytes: usize,
    tokens: Vec<&'a str>,
}

impl<'a> Profile<'a> {
    fn new(s: &'a str) -> Profile<'a> {
        let mut tokens: Vec<&str> = intraline::tokens(s).into_iter().map(|t| &s[t]).filter(|t| !intraline::is_space(t)).collect();
        tokens.sort_unstable();
        Profile { bytes: tokens.iter().map(|t| t.len()).sum(), tokens }
    }

    /// The bytes of the tokens both lines have, counted as multisets: never less than what an
    /// in-order match shares, so it bounds the similarity from above.
    fn shared(&self, other: &Profile) -> usize {
        let (mut i, mut j, mut bytes) = (0, 0, 0);
        while i < self.tokens.len() && j < other.tokens.len() {
            match self.tokens[i].cmp(other.tokens[j]) {
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
                std::cmp::Ordering::Equal => {
                    bytes += self.tokens[i].len();
                    i += 1;
                    j += 1;
                }
            }
        }
        bytes
    }
}

impl Work {
    /// The anchors of the run of removed lines `old` and added lines `new` (indices into `lines`).
    fn run(&mut self, lines: &[DiffLine], old: Range<usize>, new: Range<usize>) -> Vec<(u32, u32)> {
        let candidates = |r: Range<usize>, origin: char| -> Vec<usize> { r.filter(|&i| lines[i].origin == origin).collect() };
        let (dels, adds) = (candidates(old, '-'), candidates(new, '+'));
        let (n, m) = (dels.len(), adds.len());
        // With one line a side the rows are the same either way.
        if self.stopped || n == 0 || m == 0 || (n == 1 && m == 1) || n * m > self.limits.run_pairs {
            return Vec::new();
        }
        if self.scored + n * m > self.limits.diff_pairs {
            self.stopped = true;
            return Vec::new();
        }
        self.scored += n * m;
        let Some(w) = self.weights(lines, &dels, &adds) else {
            self.stopped = true;
            return Vec::new();
        };
        align(&w, n, m).into_iter().map(|(i, j)| (dels[i] as u32, adds[j] as u32)).collect()
    }

    /// The `n x m` table of match weights (similarity x 1000, 0 when the lines can't match), or
    /// `None` when the cell budget runs out.
    fn weights(&mut self, lines: &[DiffLine], dels: &[usize], adds: &[usize]) -> Option<Vec<u32>> {
        let text = |i: usize| lines[i].text.as_str();
        let usable = |i: usize| text(i).len() <= intraline::MAX_LINE_BYTES;
        let profile = |i: usize| usable(i).then(|| Profile::new(text(i))).filter(|p| p.bytes > 0);
        let (pd, pa): (Vec<_>, Vec<_>) = (dels.iter().map(|&i| profile(i)).collect(), adds.iter().map(|&i| profile(i)).collect());
        let mut w = vec![0u32; dels.len() * adds.len()];
        for (i, a) in pd.iter().enumerate() {
            let Some(a) = a else { continue };
            for (j, b) in pa.iter().enumerate() {
                let Some(b) = b else { continue };
                // Same form as `similarity`, so the bound never rejects a pair it would accept.
                if (2 * a.shared(b)) as f32 / ((a.bytes + b.bytes) as f32) < MIN_SIMILARITY {
                    continue;
                }
                let s = intraline::similarity_checked(text(dels[i]), text(adds[j]), &mut self.cells).ok()?;
                if let Some(s) = s.filter(|&s| s >= MIN_SIMILARITY) {
                    w[i * adds.len() + j] = (s * 1000.0).round() as u32;
                }
            }
        }
        Some(w)
    }
}

/// The order-keeping matching of the most total weight in the `n x m` table `w` (row-major), as
/// (row, column) pairs. Ties prefer a match, then the earliest lines.
fn align(w: &[u32], n: usize, m: usize) -> Vec<(usize, usize)> {
    let cols = m + 1;
    let mut best = vec![0u32; (n + 1) * cols];
    let at = |best: &[u32], i: usize, j: usize| best[i * cols + j];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let skip = at(&best, i + 1, j).max(at(&best, i, j + 1));
            let wij = w[i * m + j];
            let take = if wij > 0 { wij + at(&best, i + 1, j + 1) } else { 0 };
            best[i * cols + j] = skip.max(take);
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        let wij = w[i * m + j];
        let (down, right) = (at(&best, i + 1, j), at(&best, i, j + 1));
        if wij > 0 && wij + at(&best, i + 1, j + 1) >= down.max(right) {
            out.push((i, j));
            i += 1;
            j += 1;
        } else if down >= right {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests;
