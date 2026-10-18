//! Editing the conflict view's output as text: the whole file as it would be saved (chunks not
//! picked yet as conflict markers, every line ending in LF) with each chunk's byte range, and
//! which chunk an edit of that text falls in (`locate_edit`). Only the chunks can be edited.

use crate::conflict_pick::{ChunkPick, render_picks};
use crate::repo::ConflictFile;
use std::ops::Range;

/// The output as editable text, and where each chunk's text is in it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EditText {
    pub text: String,
    pub chunks: Vec<Range<usize>>,
}

/// The editor's text for `picks`; `None` for anything but text, or a wrong number of picks.
pub fn edit_text(file: &ConflictFile, picks: &[ChunkPick]) -> Option<EditText> {
    let mut chunks = Vec::new();
    let text = render_picks(file, picks, true, true, Some(&mut chunks))?;
    Some(EditText { text, chunks })
}

/// Where an edit of the text landed.
#[derive(Debug, PartialEq)]
pub enum EditResult {
    Unchanged,
    /// Inside chunk `chunk`, whose text is now `lines` (each ending in the file's line ending,
    /// a last line without one kept as it is).
    Chunk {
        chunk: usize,
        lines: Vec<String>,
    },
    /// It touches a clean line or more than one chunk.
    Outside,
}

/// Which chunk of `before` the edit that made `after` is in. The changed span is the text
/// between the common start and end; an insertion or deletion that could have been made at
/// several places (repeated text) is placed in a chunk if any of those places is in one. An
/// insertion at a chunk's very end belongs to the chunk only if it ends a line (otherwise it
/// joins the clean line after it). `eol` is the file's line ending, given to the new lines.
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
    let mut q = p;
    loop {
        if old.is_char_boundary(q) && after.is_char_boundary(q) {
            let inserted = &after[q..q + ins];
            if let Some(k) = chunk_at(before, q, q + del, inserted) {
                let r = &before.chunks[k];
                let end = r.end + nb.len() - ob.len();
                let text = &after[r.start..end];
                return EditResult::Chunk { chunk: k, lines: split(text, eol) };
            }
        }
        if q == 0 || q <= lowest {
            return EditResult::Outside;
        }
        q -= 1;
    }
}

/// The chunk whose range holds the old span `a..b`, replaced with `inserted`; the first that
/// doesn't start before `a` when several do (an empty chunk at a boundary), else the first.
fn chunk_at(before: &EditText, a: usize, b: usize, inserted: &str) -> Option<usize> {
    let joins_next = |r: &Range<usize>| a == b && a == r.end && r.end < before.text.len() && !inserted.ends_with('\n');
    let hits: Vec<usize> = (0..before.chunks.len())
        .filter(|&k| {
            let r = &before.chunks[k];
            r.start <= a && b <= r.end && !joins_next(r)
        })
        .collect();
    hits.iter().copied().find(|&k| before.chunks[k].start >= a).or(hits.first().copied())
}

/// `text` split into lines, each LF (or CR LF) made `eol`.
fn split(text: &str, eol: &str) -> Vec<String> {
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
