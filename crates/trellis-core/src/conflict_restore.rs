//! The picks a working file saved with conflict markers holds, read back against the index
//! stages: which chunks are still marker regions, which were resolved to whole sides, single
//! lines or other text, and which clean runs between them were edited. Pure (the file is read
//! in `repo::saved_picks`); line endings are ignored when comparing.
//!
//! The clean runs are found in the working text first (a line diff of the clean lines against
//! the file); the lines between them hold the chunks, in order.

use crate::conflict_edit::split;
use crate::conflict_pick::{ChunkPick, CleanEdits, LineRef, expand, file_eol};
use crate::conflict_rows::trivial_pick;
use crate::repo::{Choice, Chunk, ConflictContent, ConflictFile, Segment, Side, marker_regions};
use git2::{DiffOptions, Patch};
use std::ops::Range;

/// What a saved file holds: one pick per chunk (nothing picked for a chunk still in markers),
/// and the clean runs whose text changed.
#[derive(Clone, Debug, PartialEq)]
pub struct SavedPicks {
    pub picks: Vec<ChunkPick>,
    pub cleans: CleanEdits,
}

/// The whole-side choices a chunk's text is tried against, after its trivial pick.
const CHOICES: [Choice; 4] = [Choice::Ours, Choice::Theirs, Choice::OursThenTheirs, Choice::TheirsThenOurs];

/// `l` without its line terminator.
fn bare(l: &str) -> &str {
    l.strip_suffix('\n').map_or(l, |l| l.strip_suffix('\r').unwrap_or(l))
}

fn bare_all(lines: &[String]) -> Vec<&str> {
    lines.iter().map(|l| bare(l)).collect()
}

/// The picks `working` (the working file's text) holds for `file`. `None` when nothing came
/// back (every chunk still in markers and no clean line changed) or the text can't be matched
/// with the stages: a clean run missing, or a chunk whose text holds marker lines that aren't
/// exactly one region with the chunk's own sides.
pub fn restore(file: &ConflictFile, working: &str) -> Option<SavedPicks> {
    let ConflictContent::Text(segs) = &file.content else { return None };
    let w: Vec<&str> = working.split_inclusive('\n').collect();
    let spans = clean_spans(segs, &w)?;
    let eol = file_eol(segs);
    let mut picks = Vec::with_capacity(file.chunk_count());
    let mut cleans = CleanEdits::new();
    let mut pending: Vec<&Chunk> = Vec::new();
    let mut at = 0;
    let mut spans = spans.into_iter();
    for (seg, s) in segs.iter().enumerate() {
        match s {
            Segment::Conflict(c) => pending.push(c),
            Segment::Clean(lines) => {
                let span = spans.next()?;
                fill(file, &std::mem::take(&mut pending), w.get(at..span.start)?, eol, &mut picks)?;
                let now = &w[span.clone()];
                if now.iter().map(|l| bare(l)).ne(lines.iter().map(|l| bare(l))) {
                    cleans.insert(seg, split(&now.concat(), eol));
                }
                at = span.end;
            }
        }
    }
    fill(file, &pending, w.get(at..)?, eol, &mut picks)?;
    if picks.len() != file.chunk_count() || (cleans.is_empty() && picks.iter().all(|p| *p == ChunkPick::default())) {
        return None;
    }
    Some(SavedPicks { picks, cleans })
}

/// Where each clean segment's lines are in `w`, in order: from its first matched line to its
/// last (the file's first segment from the start of `w`, its last to the end). `None` when a
/// clean run has no line left in `w`.
fn clean_spans(segs: &[Segment], w: &[&str]) -> Option<Vec<Range<usize>>> {
    let (mut s_text, mut owner) = (String::new(), Vec::new());
    for (i, seg) in segs.iter().enumerate() {
        if let Segment::Clean(lines) = seg {
            for l in lines {
                s_text.push_str(bare(l));
                s_text.push('\n');
                owner.push(i);
            }
        }
    }
    let mut w_text = String::with_capacity(w.iter().map(|l| l.len() + 1).sum());
    for l in w {
        w_text.push_str(bare(l));
        w_text.push('\n');
    }
    let matched = if owner.is_empty() { Vec::new() } else { align(&s_text, &w_text, owner.len(), w.len())? };
    let mut spans = Vec::new();
    let mut k = 0;
    for (i, seg) in segs.iter().enumerate() {
        let Segment::Clean(lines) = seg else { continue };
        let found: Vec<usize> = matched[k..k + lines.len()].iter().flatten().copied().collect();
        k += lines.len();
        let (first, last) = (*found.first()?, *found.last()?);
        let start = if i == 0 { 0 } else { first };
        let end = if i + 1 == segs.len() { w.len() } else { last + 1 };
        spans.push(start..end);
    }
    Some(spans)
}

/// For each of the `n_s` lines of `s`, the line of `w` (of `n_w`) it was matched with by a line
/// diff, if any.
fn align(s: &str, w: &str, n_s: usize, n_w: usize) -> Option<Vec<Option<usize>>> {
    let mut opts = DiffOptions::new();
    opts.context_lines(0).patience(true).force_text(true);
    let patch = Patch::from_buffers(s.as_bytes(), None, w.as_bytes(), None, Some(&mut opts)).ok()?;
    let (mut deleted, mut inserted) = (vec![false; n_s], vec![false; n_w]);
    for h in 0..patch.num_hunks() {
        for l in 0..patch.num_lines_in_hunk(h).ok()? {
            let line = patch.line_in_hunk(h, l).ok()?;
            match line.origin() {
                '-' => *deleted.get_mut(line.old_lineno()? as usize - 1)? = true,
                '+' => *inserted.get_mut(line.new_lineno()? as usize - 1)? = true,
                _ => {}
            }
        }
    }
    let mut kept = (0..n_w).filter(|&j| !inserted[j]);
    Some(deleted.iter().map(|&d| if d { None } else { kept.next() }).collect())
}

/// The picks of `chunks` (adjacent, no clean run between them) from the lines `gap` between
/// the clean runs around them: each chunk but the last takes a marker region at the gap's start
/// that matches it, else the longest whole-side choice the gap starts with; the last takes the
/// rest.
fn fill(file: &ConflictFile, chunks: &[&Chunk], mut gap: &[&str], eol: &str, picks: &mut Vec<ChunkPick>) -> Option<()> {
    if chunks.is_empty() {
        return gap.is_empty().then_some(());
    }
    for (n, c) in chunks.iter().enumerate() {
        let len = if n + 1 == chunks.len() { gap.len() } else { prefix_len(c, gap)? };
        let trivial = trivial_pick(c, !file.no_ancestor).map(|t| t.0);
        picks.push(classify(c, &gap[..len], trivial, eol)?);
        gap = &gap[len..];
    }
    Some(())
}

/// How many lines at the start of `gap` are chunk `c`'s.
fn prefix_len(c: &Chunk, gap: &[&str]) -> Option<usize> {
    if let Some((len, ours, theirs)) = region(gap)
        && ours == bare_all(&c.ours)
        && theirs == bare_all(&c.theirs)
    {
        return Some(len);
    }
    let g: Vec<&str> = gap.iter().map(|l| bare(l)).collect();
    CHOICES.iter().map(|&ch| side_lines(ch, c)).filter(|lines| g.starts_with(lines)).map(|lines| lines.len()).max()
}

/// The lines (bare) whole-side `choice` takes from `c`.
fn side_lines(choice: Choice, c: &Chunk) -> Vec<&str> {
    expand(choice, c).iter().map(|r| bare(&c.side(r.side)[r.line])).collect()
}

/// What `text` (the chunk's lines in the working file) makes of chunk `c`; `None` when it holds
/// marker lines that aren't one region with the chunk's sides.
fn classify(c: &Chunk, text: &[&str], trivial: Option<Choice>, eol: &str) -> Option<ChunkPick> {
    if let Some((len, ours, theirs)) = region(text)
        && len == text.len()
        && ours == bare_all(&c.ours)
        && theirs == bare_all(&c.theirs)
    {
        return Some(ChunkPick::default());
    }
    let joined = text.concat();
    if text.iter().any(|l| run(bare(l), '<').is_some()) || marker_regions(joined.as_bytes()) > 0 {
        return None;
    }
    let t: Vec<&str> = text.iter().map(|l| bare(l)).collect();
    if let Some(ch) = trivial.into_iter().chain(CHOICES).find(|&ch| side_lines(ch, c) == t) {
        return Some(ChunkPick::Sides(ch));
    }
    if !t.is_empty()
        && let Some(v) = greedy_lines(c, &t)
    {
        return Some(ChunkPick::from_refs(v, c));
    }
    Some(ChunkPick::Edited { lines: split(&joined, eol), before: Box::default() })
}

/// `t` as distinct lines of `c`: each takes the first line not taken yet with its text, ours
/// before theirs.
fn greedy_lines(c: &Chunk, t: &[&str]) -> Option<Vec<LineRef>> {
    let all: Vec<LineRef> =
        [Side::Ours, Side::Theirs].into_iter().flat_map(|side| (0..c.side(side).len()).map(move |line| LineRef { side, line })).collect();
    let mut v: Vec<LineRef> = Vec::with_capacity(t.len());
    for l in t {
        let r = all.iter().find(|r| !v.contains(r) && bare(&c.side(r.side)[r.line]) == *l)?;
        v.push(*r);
    }
    Some(v)
}

/// The marker region at the start of `lines`, when it is one: its length and its ours and
/// theirs lines (bare). Any marker size of 7 or more and any labels; a base section (diff3) is
/// skipped.
fn region<'a>(lines: &[&'a str]) -> Option<(usize, Vec<&'a str>, Vec<&'a str>)> {
    let n = run(bare(lines.first()?), '<')?;
    let (mut i, mut ours, mut theirs) = (1, Vec::new(), Vec::new());
    let mut in_base = false;
    loop {
        let l = bare(lines.get(i)?);
        i += 1;
        if separator(l, n) {
            break;
        }
        if run(l, '<').is_some() {
            return None;
        }
        if !in_base && run(l, '|') == Some(n) {
            in_base = true;
        } else if !in_base {
            ours.push(l);
        }
    }
    loop {
        let l = bare(lines.get(i)?);
        i += 1;
        if run(l, '>') == Some(n) {
            return Some((i, ours, theirs));
        }
        if run(l, '<').is_some() {
            return None;
        }
        theirs.push(l);
    }
}

/// The length of the run of `c` that starts `line`, when it is a marker: at least 7 long and
/// followed by the end of the line or a space or tab (then the label).
fn run(line: &str, c: char) -> Option<usize> {
    let n = line.chars().take_while(|&x| x == c).count();
    (n >= 7 && matches!(line[n..].chars().next(), None | Some(' ' | '\t'))).then_some(n)
}

/// Whether `line` is exactly `n` `=`, optionally followed by whitespace.
fn separator(line: &str, n: usize) -> bool {
    let b = line.as_bytes();
    b.len() >= n && b[..n].iter().all(|&x| x == b'=') && b[n..].iter().all(u8::is_ascii_whitespace)
}

#[cfg(test)]
#[path = "conflict_restore_tests.rs"]
mod tests;
