//! Row layout of the conflict view: the ours and theirs panes side by side (aligned, the
//! shorter side of a chunk padded), and the output as it will be saved (`conflict_pick`), long
//! clean runs folded into one row unless opened (`conflict_folds`). Also the chunks that can be
//! picked safely without asking (`trivial_pick`).

use crate::conflict_folds::{CleanFold, clean_fold};
use crate::conflict_pick::{ChunkPick, CleanEdits};
use crate::i18n::Lang;
use crate::repo::{Choice, Chunk, ConflictFile, Segment, Side};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PaneRow {
    /// Line `line` of clean segment `seg`, with its number in each side's file.
    Clean { seg: usize, line: usize, ours_no: u32, theirs_no: u32 },
    /// Clean lines folded into one row.
    Fold(CleanFold),
    /// The header of a conflict chunk.
    Header(usize),
    /// Row `row` of a chunk: `chunk.ours[row]` / `chunk.theirs[row]` with their line numbers,
    /// `None` past that side's length (padding). A chunk with no lines on either side still
    /// gets one row.
    Lines { chunk: usize, row: usize, ours: Option<u32>, theirs: Option<u32> },
}

/// The line indexes of clean segment `seg` (of `n` lines) to show one by one, around its fold
/// when it has one that isn't in `opened`: the lines before it, the fold, the lines after it.
fn clean_parts(
    segs: &[Segment],
    seg: usize,
    n: usize,
    opened: &BTreeSet<usize>,
) -> (std::ops::Range<usize>, Option<CleanFold>, std::ops::Range<usize>) {
    match clean_fold(segs, seg).filter(|_| !opened.contains(&seg)) {
        Some(f) => (0..f.start, Some(f), f.end..n),
        None => (0..n, None, n..n),
    }
}

/// The pane rows, and the first row (the header) of each chunk. Clean runs fold unless their
/// segment is in `opened`; the line numbers count the folded lines.
pub fn pane_rows(segs: &[Segment], opened: &BTreeSet<usize>) -> (Vec<PaneRow>, Vec<usize>) {
    let (mut rows, mut firsts) = (Vec::new(), Vec::new());
    let (mut ours_no, mut theirs_no) = (1u32, 1u32);
    let mut chunk = 0;
    for (seg, s) in segs.iter().enumerate() {
        match s {
            Segment::Clean(lines) => {
                let (before, fold, after) = clean_parts(segs, seg, lines.len(), opened);
                // The folded lines count too, so the numbers after a fold stay right.
                let (o, t) = (ours_no, theirs_no);
                let clean = |line: usize| PaneRow::Clean { seg, line, ours_no: o + line as u32, theirs_no: t + line as u32 };
                rows.extend(before.map(clean));
                rows.extend(fold.map(PaneRow::Fold));
                rows.extend(after.map(clean));
                ours_no += lines.len() as u32;
                theirs_no += lines.len() as u32;
            }
            Segment::Conflict(c) => {
                firsts.push(rows.len());
                rows.push(PaneRow::Header(chunk));
                for row in 0..c.ours.len().max(c.theirs.len()).max(1) {
                    let number = |len: usize, counter: &mut u32| {
                        (row < len).then(|| {
                            *counter += 1;
                            *counter - 1
                        })
                    };
                    let ours = number(c.ours.len(), &mut ours_no);
                    let theirs = number(c.theirs.len(), &mut theirs_no);
                    rows.push(PaneRow::Lines { chunk, row, ours, theirs });
                }
                chunk += 1;
            }
        }
    }
    (rows, firsts)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OutRow {
    Clean {
        seg: usize,
        line: usize,
    },
    /// Clean lines folded into one row.
    Fold(CleanFold),
    /// Line `line` of the text typed for clean segment `seg` (`CleanEdits`).
    CleanEdited {
        seg: usize,
        line: usize,
    },
    /// Line `line` of `side` in chunk `chunk`.
    Taken {
        chunk: usize,
        side: Side,
        line: usize,
    },
    /// Line `line` of the text typed for chunk `chunk`.
    Edited {
        chunk: usize,
        line: usize,
    },
    /// A placeholder for a chunk with no pick yet.
    Unresolved(usize),
}

/// The output rows for `picks` and the edited clean runs `cleans`, and the first row of each
/// chunk (for a picked chunk with no lines, the row after it). Typed text shows as typed,
/// markers and all. Clean runs not edited fold unless their segment is in `opened`; edited ones
/// never fold.
pub fn output_rows(segs: &[Segment], picks: &[ChunkPick], cleans: &CleanEdits, opened: &BTreeSet<usize>) -> (Vec<OutRow>, Vec<usize>) {
    let (mut rows, mut firsts) = (Vec::new(), Vec::new());
    let mut chunk = 0;
    let none = ChunkPick::default();
    for (seg, s) in segs.iter().enumerate() {
        match s {
            Segment::Clean(_) if cleans.contains_key(&seg) => {
                rows.extend((0..cleans[&seg].len()).map(|line| OutRow::CleanEdited { seg, line }));
            }
            Segment::Clean(lines) => {
                let (before, fold, after) = clean_parts(segs, seg, lines.len(), opened);
                rows.extend(before.map(|line| OutRow::Clean { seg, line }));
                rows.extend(fold.map(OutRow::Fold));
                rows.extend(after.map(|line| OutRow::Clean { seg, line }));
            }
            Segment::Conflict(c) => {
                firsts.push(rows.len());
                match picks.get(chunk).unwrap_or(&none) {
                    ChunkPick::Edited { lines, .. } => rows.extend((0..lines.len()).map(|line| OutRow::Edited { chunk, line })),
                    p if p.is_open() => rows.push(OutRow::Unresolved(chunk)),
                    p => rows.extend(p.refs(c).into_iter().map(|r| OutRow::Taken { chunk, side: r.side, line: r.line })),
                }
                chunk += 1;
            }
        }
    }
    (rows, firsts)
}

/// The row to scroll to so chunk `chunk` sits near the top with 3 rows of context.
pub fn scroll_target(first_rows: &[usize], chunk: usize) -> usize {
    first_rows.get(chunk).copied().unwrap_or(0).saturating_sub(3)
}

/// Why a chunk could be picked without asking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trivial {
    /// One side left the base as it was, so the other side's change is the whole story.
    Unchanged,
    /// Both sides made the same change.
    Identical,
    /// The sides (or one side and the base) differ only in spaces, tabs, blank lines or line
    /// endings.
    Whitespace,
}

impl Trivial {
    /// A few words for the chunk header.
    pub fn label(self, t: Lang) -> &'static str {
        match self {
            Trivial::Unchanged => t.tr("conflict.auto.unchanged"),
            Trivial::Identical => t.tr("conflict.auto.identical"),
            Trivial::Whitespace => t.tr("conflict.auto.whitespace"),
        }
    }
}

/// The lines without their terminators.
fn bare(lines: &[String]) -> Vec<&str> {
    lines.iter().map(|l| l.strip_suffix('\n').map_or(l.as_str(), |l| l.strip_suffix('\r').unwrap_or(l))).collect()
}

/// The lines with each run of whitespace made one space, ends trimmed and blank lines dropped.
fn loose(lines: &[String]) -> Vec<String> {
    lines.iter().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()).collect()
}

/// The pick for `chunk` when one is safe without asking, and why. Checked in order: one side
/// equal to the base (line endings aside) takes the other side; both sides equal takes ours;
/// then the same three tests with whitespace ignored. Without a real base (`has_base` false,
/// both sides added the file) the base tests are skipped: that base is an empty stand-in.
pub fn trivial_pick(chunk: &Chunk, has_base: bool) -> Option<(Choice, Trivial)> {
    let (base, ours, theirs) = (bare(&chunk.base), bare(&chunk.ours), bare(&chunk.theirs));
    if has_base && ours == base {
        return Some((Choice::Theirs, Trivial::Unchanged));
    }
    if has_base && theirs == base {
        return Some((Choice::Ours, Trivial::Unchanged));
    }
    if chunk.ours == chunk.theirs {
        return Some((Choice::Ours, Trivial::Identical));
    }
    let (ours, theirs) = (loose(&chunk.ours), loose(&chunk.theirs));
    if ours == theirs {
        return Some((Choice::Ours, Trivial::Whitespace));
    }
    if has_base {
        let base = loose(&chunk.base);
        if ours == base {
            return Some((Choice::Theirs, Trivial::Whitespace));
        }
        if theirs == base {
            return Some((Choice::Ours, Trivial::Whitespace));
        }
    }
    None
}

/// `trivial_pick` for each of `file`'s chunks, in order.
pub fn trivial_picks(file: &ConflictFile) -> Vec<Option<(Choice, Trivial)>> {
    file.chunks().map(|c| trivial_pick(c, !file.no_ancestor)).collect()
}

#[cfg(test)]
#[path = "conflict_rows_tests.rs"]
mod tests;
