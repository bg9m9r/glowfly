//! Row layout of the conflict view: the ours and theirs panes side by side (aligned, the
//! shorter side of a chunk padded), and the output as it will be saved. Also the chunks that
//! can be picked safely without asking (`trivial_pick`).

use crate::repo::{Choice, Chunk, ConflictFile, Segment, Side};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PaneRow {
    /// Line `line` of clean segment `seg`, with its number in each side's file.
    Clean { seg: usize, line: usize, ours_no: u32, theirs_no: u32 },
    /// The header of a conflict chunk.
    Header(usize),
    /// Row `row` of a chunk: `chunk.ours[row]` / `chunk.theirs[row]` with their line numbers,
    /// `None` past that side's length (padding). A chunk with no lines on either side still
    /// gets one row.
    Lines { chunk: usize, row: usize, ours: Option<u32>, theirs: Option<u32> },
}

/// The pane rows, and the first row (the header) of each chunk.
pub fn pane_rows(segs: &[Segment]) -> (Vec<PaneRow>, Vec<usize>) {
    let (mut rows, mut firsts) = (Vec::new(), Vec::new());
    let (mut ours_no, mut theirs_no) = (1u32, 1u32);
    let mut chunk = 0;
    for (seg, s) in segs.iter().enumerate() {
        match s {
            Segment::Clean(lines) => {
                for line in 0..lines.len() {
                    rows.push(PaneRow::Clean { seg, line, ours_no, theirs_no });
                    ours_no += 1;
                    theirs_no += 1;
                }
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
    /// Line `line` of `side` in chunk `chunk`.
    Taken {
        chunk: usize,
        side: Side,
        line: usize,
    },
    /// A placeholder for a chunk with no pick yet.
    Unresolved(usize),
}

/// The output rows for `choices`, and the first row of each chunk (for a picked chunk with no
/// lines, the row after it).
pub fn output_rows(segs: &[Segment], choices: &[Choice]) -> (Vec<OutRow>, Vec<usize>) {
    let (mut rows, mut firsts) = (Vec::new(), Vec::new());
    let mut chunk = 0;
    for (seg, s) in segs.iter().enumerate() {
        match s {
            Segment::Clean(lines) => rows.extend((0..lines.len()).map(|line| OutRow::Clean { seg, line })),
            Segment::Conflict(c) => {
                firsts.push(rows.len());
                let choice = choices.get(chunk).copied().unwrap_or_default();
                if choice == Choice::Unresolved {
                    rows.push(OutRow::Unresolved(chunk));
                }
                for &side in choice.sides() {
                    rows.extend((0..c.side(side).len()).map(|line| OutRow::Taken { chunk, side, line }));
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
    pub fn label(self) -> &'static str {
        match self {
            Trivial::Unchanged => "unchanged on one side",
            Trivial::Identical => "same on both sides",
            Trivial::Whitespace => "whitespace only",
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
mod tests {
    use super::*;

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| format!("{s}\n")).collect()
    }

    fn sample() -> Vec<Segment> {
        vec![
            Segment::Clean(lines(&["a", "b"])),
            Segment::Conflict(Chunk { base: lines(&["c"]), ours: lines(&["o1", "o2", "o3"]), theirs: lines(&["t1"]) }),
            Segment::Clean(lines(&["d"])),
        ]
    }

    #[test]
    fn pane_rows_pad_the_shorter_side() {
        let (rows, firsts) = pane_rows(&sample());
        assert_eq!(firsts, [2]);
        assert_eq!(rows[2], PaneRow::Header(0));
        assert_eq!(rows[3], PaneRow::Lines { chunk: 0, row: 0, ours: Some(3), theirs: Some(3) });
        assert_eq!(rows[4], PaneRow::Lines { chunk: 0, row: 1, ours: Some(4), theirs: None });
        assert_eq!(rows[5], PaneRow::Lines { chunk: 0, row: 2, ours: Some(5), theirs: None });
        // After the chunk each side continues from its own count.
        assert_eq!(rows[6], PaneRow::Clean { seg: 2, line: 0, ours_no: 6, theirs_no: 4 });
        assert_eq!(rows.len(), 7);
        // A chunk with nothing on either side still gets a row.
        let empty = [Segment::Conflict(Chunk { base: lines(&["x"]), ours: Vec::new(), theirs: Vec::new() })];
        let (rows, _) = pane_rows(&empty);
        assert_eq!(rows, [PaneRow::Header(0), PaneRow::Lines { chunk: 0, row: 0, ours: None, theirs: None }]);
    }

    #[test]
    fn output_rows_show_placeholders() {
        let segs = sample();
        let (rows, firsts) = output_rows(&segs, &[Choice::Unresolved]);
        assert_eq!(rows.len(), 4);
        assert_eq!((rows[2], firsts[0]), (OutRow::Unresolved(0), 2));
        let (rows, _) = output_rows(&segs, &[Choice::OursThenTheirs]);
        let taken: Vec<_> = rows
            .iter()
            .filter_map(|r| match r {
                OutRow::Taken { side, line, .. } => Some((*side, *line)),
                _ => None,
            })
            .collect();
        assert_eq!(taken, [(Side::Ours, 0), (Side::Ours, 1), (Side::Ours, 2), (Side::Theirs, 0)]);
        assert_eq!(rows.last(), Some(&OutRow::Clean { seg: 2, line: 0 }));
        // A picked side with no lines leaves no rows; the chunk starts at the next row.
        let empty = [Segment::Conflict(Chunk { base: Vec::new(), ours: Vec::new(), theirs: lines(&["t"]) }), Segment::Clean(lines(&["z"]))];
        let (rows, firsts) = output_rows(&empty, &[Choice::Ours]);
        assert_eq!((rows, firsts), (vec![OutRow::Clean { seg: 1, line: 0 }], vec![0]));
    }

    #[test]
    fn scroll_target_clamps_at_zero() {
        assert_eq!(scroll_target(&[1, 10], 0), 0);
        assert_eq!(scroll_target(&[1, 10], 1), 7);
        assert_eq!(scroll_target(&[], 3), 0);
    }

    fn chunk(base: &[&str], ours: &[&str], theirs: &[&str]) -> Chunk {
        Chunk { base: lines(base), ours: lines(ours), theirs: lines(theirs) }
    }

    #[test]
    fn trivial_pick_table() {
        use Choice::{Ours, Theirs};
        use Trivial::{Identical, Unchanged, Whitespace};
        let crlf = |v: &[&str]| v.iter().map(|s| format!("{s}\r\n")).collect::<Vec<_>>();
        type Row = (&'static str, Chunk, Option<(Choice, Trivial)>);
        let table: [Row; 9] = [
            ("ours is the base", chunk(&["a"], &["a"], &["b"]), Some((Theirs, Unchanged))),
            ("theirs is the base", chunk(&["a"], &["b"], &["a"]), Some((Ours, Unchanged))),
            ("the same change", chunk(&["a"], &["b", "c"], &["b", "c"]), Some((Ours, Identical))),
            ("indentation", chunk(&["a"], &["    b(x)"], &["\tb(x)"]), Some((Ours, Whitespace))),
            ("trailing spaces", chunk(&["a"], &["b  c"], &["b c  "]), Some((Ours, Whitespace))),
            ("a blank line added", chunk(&["a"], &["b", "", "c"], &["b", "c"]), Some((Ours, Whitespace))),
            ("ours only reindents", chunk(&["  a"], &["a"], &["z"]), Some((Theirs, Whitespace))),
            ("theirs only reindents", chunk(&["  a"], &["z"], &["a "]), Some((Ours, Whitespace))),
            ("a real conflict", chunk(&["a"], &["b"], &["c"]), None),
        ];
        for (what, c, want) in table {
            assert_eq!(trivial_pick(&c, true), want, "{what}");
        }
        // Line endings alone: CR LF against LF.
        let c = Chunk { base: lines(&["a"]), ours: crlf(&["b"]), theirs: lines(&["b"]) };
        assert_eq!(trivial_pick(&c, true), Some((Ours, Whitespace)));
        // Line endings are ignored against the base too.
        let c = Chunk { base: crlf(&["a"]), ours: lines(&["a"]), theirs: lines(&["b"]) };
        assert_eq!(trivial_pick(&c, true), Some((Theirs, Unchanged)));
        // Without a base, an empty side isn't "unchanged": the empty base is made up.
        let c = chunk(&[], &[], &["t"]);
        assert_eq!(trivial_pick(&c, false), None);
        assert_eq!(trivial_pick(&c, true), Some((Theirs, Unchanged)));
        // Equal sides are still equal without a base.
        assert_eq!(trivial_pick(&chunk(&[], &["x"], &["x"]), false), Some((Ours, Identical)));
        assert_eq!(Whitespace.label(), "whitespace only");
    }
}
