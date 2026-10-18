//! What each conflict chunk resolves to beyond a whole side: single lines ticked in the order
//! they were ticked, or text typed in the output by hand. Also the file those picks make, with
//! or without conflict markers for the chunks not picked yet (`render_picks`).

use crate::repo::{Choice, Chunk, ConflictContent, ConflictFile, Segment, Side, marker_regions, marker_size};
use std::ops::Range;

/// One line of a chunk: line `line` of `side`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineRef {
    pub side: Side,
    pub line: usize,
}

/// What one chunk resolves to.
#[derive(Clone, Debug, PartialEq)]
pub enum ChunkPick {
    /// Whole sides; `Choice::Unresolved` means nothing yet.
    Sides(Choice),
    /// Single lines in the order they were ticked. Never empty, and never exactly whole sides
    /// (`normalize` turns those into `Sides`).
    Lines(Vec<LineRef>),
    /// Text typed in the output, each line keeping its terminator. `before` is the pick it
    /// replaced (never `Edited`), brought back by Discard edit.
    Edited { lines: Vec<String>, before: Box<ChunkPick> },
}

impl Default for ChunkPick {
    fn default() -> Self {
        ChunkPick::Sides(Choice::Unresolved)
    }
}

impl From<Choice> for ChunkPick {
    fn from(c: Choice) -> Self {
        ChunkPick::Sides(c)
    }
}

/// Every line of the sides `choice` takes, in output order.
fn expand(choice: Choice, c: &Chunk) -> Vec<LineRef> {
    choice.sides().iter().flat_map(|&side| (0..c.side(side).len()).map(move |line| LineRef { side, line })).collect()
}

/// `v` as the simplest pick that means the same: nothing, whole sides, or single lines.
fn normalize(v: Vec<LineRef>, c: &Chunk) -> ChunkPick {
    if v.is_empty() {
        return ChunkPick::Sides(Choice::Unresolved);
    }
    for choice in [Choice::Ours, Choice::Theirs, Choice::OursThenTheirs, Choice::TheirsThenOurs] {
        if v == expand(choice, c) {
            return ChunkPick::Sides(choice);
        }
    }
    ChunkPick::Lines(v)
}

impl ChunkPick {
    /// Not resolved yet: nothing picked, or typed text that still holds a whole marker region.
    pub fn is_open(&self) -> bool {
        match self {
            ChunkPick::Sides(c) => *c == Choice::Unresolved,
            ChunkPick::Lines(_) => false,
            ChunkPick::Edited { lines, .. } => marker_regions(lines.concat().as_bytes()) > 0,
        }
    }

    pub fn is_edited(&self) -> bool {
        matches!(self, ChunkPick::Edited { .. })
    }

    /// The lines taken, in output order; none for typed text.
    pub fn refs(&self, c: &Chunk) -> Vec<LineRef> {
        match self {
            ChunkPick::Sides(choice) => expand(*choice, c),
            ChunkPick::Lines(v) => v.clone(),
            ChunkPick::Edited { .. } => Vec::new(),
        }
    }

    pub fn takes_line(&self, c: &Chunk, side: Side, line: usize) -> bool {
        self.refs(c).contains(&LineRef { side, line })
    }

    /// Every line of `side` is taken (false for a side with no lines picked line by line).
    pub fn takes_side(&self, c: &Chunk, side: Side) -> bool {
        match self {
            ChunkPick::Sides(choice) => choice.takes(side),
            ChunkPick::Lines(v) => {
                let n = c.side(side).len();
                n > 0 && (0..n).all(|line| v.contains(&LineRef { side, line }))
            }
            ChunkPick::Edited { .. } => false,
        }
    }

    /// Some but not all lines of `side` are taken.
    pub fn partly_takes_side(&self, c: &Chunk, side: Side) -> bool {
        match self {
            ChunkPick::Lines(v) => !self.takes_side(c, side) && v.iter().any(|r| r.side == side),
            _ => false,
        }
    }

    /// Ticks or unticks one line: a ticked line goes last. Typed text is left as it is.
    pub fn toggle_line(&self, c: &Chunk, side: Side, line: usize) -> ChunkPick {
        if self.is_edited() || line >= c.side(side).len() {
            return self.clone();
        }
        let mut v = self.refs(c);
        let r = LineRef { side, line };
        match v.iter().position(|x| *x == r) {
            Some(i) => {
                v.remove(i);
            }
            None => v.push(r),
        }
        normalize(v, c)
    }

    /// Ticks or unticks a whole side. Whole-side picks follow `Choice::toggle`; with single
    /// lines, a side wholly taken is left out, otherwise its lines move to the end in file
    /// order. Typed text is left as it is.
    pub fn toggle_side(&self, c: &Chunk, side: Side) -> ChunkPick {
        match self {
            ChunkPick::Sides(choice) => ChunkPick::Sides(choice.toggle(side)),
            ChunkPick::Edited { .. } => self.clone(),
            ChunkPick::Lines(_) if c.side(side).is_empty() => self.clone(),
            ChunkPick::Lines(v) => {
                let all = self.takes_side(c, side);
                let mut v: Vec<LineRef> = v.iter().copied().filter(|r| r.side != side).collect();
                if !all {
                    v.extend((0..c.side(side).len()).map(|line| LineRef { side, line }));
                }
                normalize(v, c)
            }
        }
    }

    /// The chunk's output lines, terminators kept.
    pub fn out_lines<'a>(&'a self, c: &'a Chunk) -> Vec<&'a str> {
        match self {
            ChunkPick::Edited { lines, .. } => lines.iter().map(String::as_str).collect(),
            _ => self.refs(c).iter().map(|r| c.side(r.side)[r.line].as_str()).collect(),
        }
    }
}

/// The file's line ending: CR LF when the first line that has one ends that way, else LF.
pub fn file_eol(segs: &[Segment]) -> &'static str {
    for l in segs.iter().flat_map(seg_lines) {
        if l.ends_with('\n') {
            return if l.ends_with("\r\n") { "\r\n" } else { "\n" };
        }
    }
    "\n"
}

/// A side's name for a marker line: on one line.
fn label(s: &str) -> String {
    s.replace(['\r', '\n'], " ")
}

/// The file `picks` make (one per chunk), in repository form: each line keeps its stage's
/// ending. `None` for anything but text, a wrong number of picks, or (without `keep_markers`)
/// any chunk still open. With `keep_markers`, a chunk not picked yet is written as a conflict
/// region with the side names (and the base, for `merge.conflictStyle` diff3 and zdiff3);
/// typed text is written as typed. `lf` makes every line end in LF (for the editor). `ranges`
/// gets each chunk's byte range, each starting at a line start.
pub fn render_picks(
    file: &ConflictFile,
    picks: &[ChunkPick],
    keep_markers: bool,
    lf: bool,
    mut ranges: Option<&mut Vec<Range<usize>>>,
) -> Option<String> {
    let ConflictContent::Text(segs) = &file.content else { return None };
    if picks.len() != file.chunk_count() || (!keep_markers && picks.iter().any(ChunkPick::is_open)) {
        return None;
    }
    let eol = if lf { "\n" } else { file_eol(segs) };
    let mut out = String::new();
    let push = |out: &mut String, line: &str| {
        // A side without a final newline followed by more text still needs a line break.
        if !out.is_empty() && !out.ends_with('\n') {
            out.push_str(eol);
        }
        match line.strip_suffix("\r\n").filter(|_| lf) {
            Some(bare) => {
                out.push_str(bare);
                out.push('\n');
            }
            None => out.push_str(line),
        }
    };
    let n = if keep_markers { marker_size(segs.iter().flat_map(seg_lines).map(String::as_str)) } else { 0 };
    let marker = |c: char, text: &str| format!("{}{text}{eol}", c.to_string().repeat(n));
    if let Some(r) = ranges.as_deref_mut() {
        r.clear();
    }
    let mut k = 0;
    for seg in segs {
        let c = match seg {
            Segment::Clean(lines) => {
                lines.iter().for_each(|l| push(&mut out, l));
                continue;
            }
            Segment::Conflict(c) => c,
        };
        if ranges.is_some() && !out.is_empty() && !out.ends_with('\n') {
            out.push_str(eol);
        }
        let start = out.len();
        match &picks[k] {
            ChunkPick::Sides(Choice::Unresolved) if keep_markers => {
                push(&mut out, &marker('<', &format!(" {}", label(&file.labels.ours))));
                c.ours.iter().for_each(|l| push(&mut out, l));
                if file.base_markers && !file.no_ancestor {
                    push(&mut out, &marker('|', " base"));
                    c.base.iter().for_each(|l| push(&mut out, l));
                }
                push(&mut out, &marker('=', ""));
                c.theirs.iter().for_each(|l| push(&mut out, l));
                push(&mut out, &marker('>', &format!(" {}", label(&file.labels.theirs))));
            }
            pick => pick.out_lines(c).into_iter().for_each(|l| push(&mut out, l)),
        }
        if let Some(r) = ranges.as_deref_mut() {
            r.push(start..out.len());
        }
        k += 1;
    }
    Some(out)
}

/// Every line a segment holds (all three versions of a chunk).
fn seg_lines(s: &Segment) -> Box<dyn Iterator<Item = &String> + '_> {
    match s {
        Segment::Clean(l) => Box::new(l.iter()),
        Segment::Conflict(c) => Box::new(c.ours.iter().chain(&c.base).chain(&c.theirs)),
    }
}

#[cfg(test)]
#[path = "conflict_pick_tests.rs"]
mod tests;
