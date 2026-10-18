//! Editing the conflict view's output as text: the whole file as it would be saved (chunks not
//! picked yet as conflict markers, every line ending in LF) with the byte range of each chunk and
//! each clean run between them, and which of those an edit of that text falls in
//! (`locate_edit`). Each conflict or clean run can be edited, one at a time.

use crate::conflict_pick::{ChunkPick, CleanEdits, Regions, render};
use crate::repo::ConflictFile;
use std::ops::Range;

/// The output as editable text, and where each chunk's and each clean run's text is in it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EditText {
    pub text: String,
    pub chunks: Vec<Range<usize>>,
    /// Each clean run by segment index, in text order.
    pub cleans: Vec<(usize, Range<usize>)>,
}

/// The editor's text for `picks` and the edited clean runs `cleans`; `None` for anything but
/// text, or a wrong number of picks.
pub fn edit_text(file: &ConflictFile, picks: &[ChunkPick], cleans: &CleanEdits) -> Option<EditText> {
    let mut regions = Regions::default();
    let text = render(file, picks, cleans, true, true, Some(&mut regions))?;
    Some(EditText { text, chunks: regions.chunks, cleans: regions.cleans })
}

/// Where an edit of the text landed. New lines end in the file's line ending, a last line
/// without one kept as it is.
#[derive(Debug, PartialEq)]
pub enum EditResult {
    Unchanged,
    /// Inside chunk `chunk`, whose text is now `lines`.
    Chunk {
        chunk: usize,
        lines: Vec<String>,
    },
    /// Inside the clean run of segment `seg`, whose text is now `lines`.
    Clean {
        seg: usize,
        lines: Vec<String>,
    },
    /// It spans more than one region (a chunk and a clean run, or two chunks).
    Outside,
}

/// A region an edit can fall in.
enum Hit {
    Chunk(usize),
    /// An index into `EditText::cleans`.
    Clean(usize),
}

/// Which region of `before` the edit that made `after` is in. The changed span is the text
/// between the common start and end; an insertion or deletion that could have been made at
/// several places (repeated text) is placed in a chunk if any of those places is in one, else in
/// a clean run. An insertion at a region's very end belongs to it only if it ends a line
/// (otherwise it joins the region after it); one between a clean run and a chunk goes to the
/// chunk. `eol` is the file's line ending, given to the new lines.
pub fn locate_edit(before: &EditText, after: &str, eol: &str) -> EditResult {
    let old = before.text.as_str();
    if old == after {
        return EditResult::Unchanged;
    }
    let (ob, nb) = (old.as_bytes(), after.as_bytes());
    let min = ob.len().min(nb.len());
    let mut p = ob.iter().zip(nb).take_while(|(a, b)| a == b).count();
    while !old.is_char_boundary(p) || !after.is_char_boundary(p) {
        p -= 1;
    }
    let mut s_full = ob.iter().rev().zip(nb.iter().rev()).take_while(|(a, b)| a == b).count().min(min);
    while !old.is_char_boundary(ob.len() - s_full) || !after.is_char_boundary(nb.len() - s_full) {
        s_full -= 1;
    }
    let s = s_full.min(min - p);
    let (del, ins) = (ob.len() - s - p, nb.len() - s - p);
    // An insertion or deletion of `len` bytes may also sit further left, down to where the
    // common end starts.
    let pure = del == 0 || ins == 0;
    let lowest = if pure { ob.len().max(nb.len()).saturating_sub(s_full + del.max(ins)) } else { p };
    // The region's new end, once the edit is in.
    let new_end = |r: &Range<usize>| r.end + nb.len() - ob.len();
    for chunks_only in [true, false] {
        let mut q = p;
        loop {
            if old.is_char_boundary(q)
                && let Some(inserted) = after.get(q..q + ins)
            {
                match region_at(before, q, q + del, inserted, chunks_only) {
                    Some(Hit::Chunk(k)) => {
                        let r = &before.chunks[k];
                        return EditResult::Chunk { chunk: k, lines: split(&after[r.start..new_end(r)], eol) };
                    }
                    Some(Hit::Clean(i)) => {
                        let (seg, r) = &before.cleans[i];
                        return EditResult::Clean { seg: *seg, lines: split(&after[r.start..new_end(r)], eol) };
                    }
                    None => {}
                }
            }
            if q == 0 || q <= lowest {
                break;
            }
            q -= 1;
        }
    }
    EditResult::Outside
}

/// The region whose range holds the old span `a..b`, replaced with `inserted`: a chunk before a
/// clean run (only chunks with `chunks_only`), and among several (an empty one at a boundary)
/// the first that doesn't start before `a`, else the first.
fn region_at(before: &EditText, a: usize, b: usize, inserted: &str, chunks_only: bool) -> Option<Hit> {
    let joins_next = |r: &Range<usize>| a == b && a == r.end && r.end < before.text.len() && !inserted.ends_with('\n');
    let holds = |r: &Range<usize>| r.start <= a && b <= r.end && !joins_next(r);
    let best = |hits: Vec<(usize, &Range<usize>)>| hits.iter().find(|(_, r)| r.start >= a).or(hits.first()).map(|(i, _)| *i);
    let chunks = before.chunks.iter().enumerate().filter(|(_, r)| holds(r)).collect();
    if let Some(k) = best(chunks) {
        return Some(Hit::Chunk(k));
    }
    if chunks_only {
        return None;
    }
    let cleans = before.cleans.iter().enumerate().map(|(i, (_, r))| (i, r)).filter(|(_, r)| holds(r)).collect();
    best(cleans).map(Hit::Clean)
}

/// `text` split into lines, each LF (or CR LF) made `eol`.
pub(crate) fn split(text: &str, eol: &str) -> Vec<String> {
    text.split_inclusive('\n')
        .map(|l| match l.strip_suffix('\n') {
            Some(bare) => format!("{}{eol}", bare.strip_suffix('\r').unwrap_or(bare)),
            None => l.to_owned(),
        })
        .collect()
}

#[cfg(test)]
#[path = "conflict_edit_tests.rs"]
mod tests;
