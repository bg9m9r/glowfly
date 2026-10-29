//! Checks for the diff and conflict parsers: what a patch file holds (the Apply patch dialog's
//! reading of it, without git), the marker regions left in a working file, and the merged text
//! with conflict markers that a conflict is split into.

use super::text;
use crate::patch_ops::summarize;
use crate::repo::{Segment, marker_regions, marker_size, parse_merged};
use std::collections::HashSet;

/// A patch file. Invariants: no panic; the files listed are non-empty and listed once; a first
/// subject is only read from an emailed commit; there are no more emailed commits than lines;
/// and mixed line endings are only reported when a CRLF is there.
pub fn patch_summary(data: &[u8]) {
    let s = summarize(data);
    let mut seen = HashSet::new();
    for f in &s.files {
        assert!(!f.is_empty() && seen.insert(f), "{f:?} listed empty or twice");
    }
    assert!(s.first_subject.is_none() || s.commits >= 1);
    assert!(s.commits <= text(data).lines().count());
    assert!(!s.crlf_content || data.windows(2).any(|w| w == b"\r\n"));
}

/// `segs` written back as merged text with markers of size `n`, a base marker in every chunk.
fn render(segs: &[Segment], n: usize) -> String {
    let marker = |c: char, label: &str| format!("{}{label}\n", c.to_string().repeat(n));
    let mut out = String::new();
    for seg in segs {
        match seg {
            Segment::Clean(lines) => out.extend(lines.iter().map(String::as_str)),
            Segment::Conflict(c) => {
                out.push_str(&marker('<', " ours"));
                out.extend(c.ours.iter().map(String::as_str));
                out.push_str(&marker('|', " base"));
                out.extend(c.base.iter().map(String::as_str));
                out.push_str(&marker('=', ""));
                out.extend(c.theirs.iter().map(String::as_str));
                out.push_str(&marker('>', " theirs"));
            }
        }
    }
    out
}

/// A working file with conflict markers, and merged text as libgit2 writes it. Invariants: no
/// panic; a marker region takes at least three lines; the marker size chosen for a text is at
/// least 7 and longer than any run of marker characters starting a line; and merged text that
/// parses, written back with markers, parses to the same clean runs and chunks.
pub fn conflict_markers(data: &[u8]) {
    let regions = marker_regions(data);
    assert!(regions * 3 <= data.split(|&b| b == b'\n').count());
    let text = text(data);
    let size = marker_size(std::iter::once(text.as_ref()));
    assert!(size >= 7);
    for line in text.lines() {
        let Some(c) = line.chars().next().filter(|c| "<|=>".contains(*c)) else { continue };
        assert!(line.chars().take_while(|&x| x == c).count() < size, "{line:?} could be a marker of size {size}");
    }
    for n in [7, size] {
        let Ok(segs) = parse_merged(&text, n) else { continue };
        let again = render(&segs, n);
        assert_eq!(parse_merged(&again, n).ok().as_ref(), Some(&segs), "{text:?} written back as {again:?}");
    }
}
