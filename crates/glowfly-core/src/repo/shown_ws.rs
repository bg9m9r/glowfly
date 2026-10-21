//! Hunk and line operations on a diff shown with whitespace changes hidden: the patch is built
//! from the filtered hunk itself, so its lines keep the order shown, and only its unchanged lines
//! are swapped for their exact bytes on the side being patched, looked up by line number in the
//! unfiltered diff. Changed lines already carry their own bytes. The hidden whitespace changes on
//! unchanged lines are left as they are.

use crate::error::{Error, Result};
use git2::Patch;
use std::collections::{BTreeSet, HashMap};

/// One line of a hunk as libgit2 gives it: its origin, raw bytes (with the line ending) and line
/// numbers on the old and new sides.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct RawLine {
    pub origin: char,
    pub bytes: Vec<u8>,
    pub old: Option<u32>,
    pub new: Option<u32>,
}

/// Every line of hunk `hunk`, end-of-file markers included: the same indexing as `hunk_of` and
/// `load_diff`.
pub(super) fn raw_lines(patch: &Patch<'_>, hunk: usize) -> Result<Vec<RawLine>> {
    let count = patch.num_lines_in_hunk(hunk)?;
    let mut out = Vec::with_capacity(count);
    for l in 0..count {
        let line = patch.line_in_hunk(hunk, l)?;
        out.push(RawLine { origin: line.origin(), bytes: line.content().to_vec(), old: line.old_lineno(), new: line.new_lineno() });
    }
    Ok(out)
}

/// The exact bytes of the lines the unfiltered patch `full` shows on one side, by line number:
/// the old side (unchanged and removed lines), or with `reverse` the new side (unchanged and
/// added lines). A line number missing from the map is the same on both sides.
pub(super) fn side_lines(full: &Patch<'_>, reverse: bool) -> Result<HashMap<u32, Vec<u8>>> {
    let mut out = HashMap::new();
    for h in 0..full.num_hunks() {
        for l in 0..full.num_lines_in_hunk(h)? {
            let line = full.line_in_hunk(h, l)?;
            let n = match (line.origin(), reverse) {
                (' ' | '-', false) => line.old_lineno(),
                (' ' | '+', true) => line.new_lineno(),
                _ => None,
            };
            if let Some(n) = n {
                out.insert(n, line.content().to_vec());
            }
        }
    }
    Ok(out)
}

/// A hunk's lines as (origin, raw bytes) pairs, ready for `build_patch`.
pub(super) type Content = Vec<(char, Vec<u8>)>;

/// The refusal when an unchanged line of the shown hunk differs from the file by more than
/// whitespace.
pub(super) fn no_counterpart(line: u32) -> String {
    format!("with whitespace changes hidden, line {line} doesn't line up with the file; turn off Ignore whitespace and try again")
}

/// The shown hunk's lines as (origin, bytes) pairs for `build_patch`, with each unchanged line's
/// bytes taken from `side` (by its new line number with `reverse`, else its old one) and the
/// markers for an unchanged last line without a newline dropped (the bytes now say whether it
/// ends in one). `selected` is renumbered to match. Refused when an unchanged line differs from
/// its side's bytes by more than whitespace.
pub(super) fn exact_context(
    shown: &[RawLine],
    side: &HashMap<u32, Vec<u8>>,
    reverse: bool,
    selected: &BTreeSet<usize>,
) -> Result<(Content, BTreeSet<usize>)> {
    let mut out = Vec::with_capacity(shown.len());
    let mut kept = Vec::with_capacity(shown.len());
    for line in shown {
        let bytes = match line.origin {
            '=' => {
                kept.push(None);
                continue;
            }
            ' ' => {
                let n = if reverse { line.new } else { line.old };
                match n.and_then(|n| side.get(&n).map(|b| (n, b))) {
                    Some((n, b)) if !same_but_whitespace(&line.bytes, b) => return Err(Error::Unsupported(no_counterpart(n))),
                    Some((_, b)) => b.clone(),
                    None => line.bytes.clone(),
                }
            }
            _ => line.bytes.clone(),
        };
        kept.push(Some(out.len()));
        out.push((line.origin, bytes));
    }
    let selected = selected.iter().filter_map(|&i| kept.get(i).copied().flatten()).collect();
    Ok((out, selected))
}

fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n' | 0x0b | 0x0c)
}

/// Whether `a` and `b` are the same line but for the amount of whitespace and whitespace at the
/// end (git `-b --ignore-space-at-eol`): 4 or 8 spaces of indent are the same, "x" and " x" are
/// not.
pub(super) fn same_but_whitespace(a: &[u8], b: &[u8]) -> bool {
    fn norm(s: &[u8]) -> Vec<u8> {
        let end = s.iter().rposition(|&c| !is_space(c)).map_or(0, |i| i + 1);
        let mut out = Vec::with_capacity(end);
        let mut in_space = false;
        for &c in &s[..end] {
            if is_space(c) {
                if !in_space {
                    out.push(b' ');
                }
                in_space = true;
            } else {
                out.push(c);
                in_space = false;
            }
        }
        out
    }
    norm(a) == norm(b)
}

/// The indexes of the changed (`+`/`-`) lines, for an operation on the whole hunk.
pub(super) fn changed(lines: &[RawLine]) -> BTreeSet<usize> {
    lines.iter().enumerate().filter(|(_, l)| matches!(l.origin, '+' | '-')).map(|(i, _)| i).collect()
}

#[cfg(test)]
#[path = "shown_ws_tests.rs"]
mod tests;
