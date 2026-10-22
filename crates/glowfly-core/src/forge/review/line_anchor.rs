//! Where a new code comment goes in a file's diff: the file line (and side) it ends on and, for
//! a range, the one it starts on, from a span of a hunk's lines; and the suggested-change block
//! that offers new text for the commented lines.

use crate::diff_rows::LineSide;
use crate::repo::{DiffLine, Hunk};

/// The lines a new code comment is on.
#[derive(Clone, Debug, PartialEq)]
pub struct LineAnchor {
    /// The file, repository-relative and `/`-separated (the new path of a renamed file).
    pub path: String,
    /// The (last) line, counted in the `side` file.
    pub line: u32,
    pub side: LineSide,
    /// The first line and its side for a comment on several lines; `None` for one line.
    pub start: Option<(u32, LineSide)>,
}

/// The file line diff line `dl` is: a removed line its old number, an added or context line
/// its new one. The no-newline markers are none.
pub fn line_key(dl: &DiffLine) -> Option<(LineSide, u32)> {
    match dl.origin {
        '-' => dl.old.map(|n| (LineSide::Old, n)),
        '+' | ' ' => dl.new.map(|n| (LineSide::New, n)),
        _ => None,
    }
}

/// The anchor of a comment on lines `first..=last` of hunk `h` (in either order, clamped to the
/// hunk). Marker lines at either end are skipped; `None` when no line can take a comment. A
/// context line that starts a range takes the side its end is on, so both ends count in the
/// same file when the service compares them.
pub fn line_anchor(path: &str, h: &Hunk, first: usize, last: usize) -> Option<LineAnchor> {
    let (first, last) = span(h, first, last)?;
    let first = (first..=last).find(|&i| line_key(&h.lines[i]).is_some())?;
    let last = (first..=last).rev().find(|&i| line_key(&h.lines[i]).is_some())?;
    let (side, line) = line_key(&h.lines[last])?;
    let start_dl = &h.lines[first];
    let start = match start_dl.origin {
        ' ' if side == LineSide::Old => start_dl.old.map(|n| (n, LineSide::Old)),
        _ => line_key(start_dl).map(|(s, n)| (n, s)),
    }?;
    let start = (start != (line, side)).then_some(start);
    Some(LineAnchor { path: path.to_owned(), line, side, start })
}

/// `first..=last` ordered and clamped to `h`'s lines; `None` for an empty hunk.
fn span(h: &Hunk, first: usize, last: usize) -> Option<(usize, usize)> {
    let end = h.lines.len().checked_sub(1)?;
    let (a, b) = if first <= last { (first, last) } else { (last, first) };
    Some((a.min(end), b.min(end)))
}

/// The text of the new version's lines (added and context) among `first..=last` of `h`, in
/// order: what a suggestion for those lines starts from.
pub fn suggested_lines(h: &Hunk, first: usize, last: usize) -> Vec<&str> {
    let Some((first, last)) = span(h, first, last) else { return Vec::new() };
    h.lines[first..=last].iter().filter(|l| matches!(l.origin, '+' | ' ')).map(|l| l.text.as_str()).collect()
}

/// A suggested-change block offering `lines` in place of the commented ones (none suggests
/// deleting them). Its fence is longer than any run of three or more backticks a line starts
/// with, so no quoted line closes it early.
pub fn suggestion_block(lines: &[&str]) -> String {
    let lines: Vec<&str> = lines.iter().map(|l| l.strip_suffix('\r').unwrap_or(l)).collect();
    let longest = lines.iter().map(|l| l.chars().take_while(|&c| c == '`').count()).filter(|&n| n >= 3).max().unwrap_or(0);
    let fence = "`".repeat((longest + 1).max(3));
    if lines.is_empty() { format!("{fence}suggestion\n{fence}") } else { format!("{fence}suggestion\n{}\n{fence}", lines.join("\n")) }
}

/// The service's name for a side: "LEFT" for the old file, "RIGHT" for the new one.
pub fn side_name(s: LineSide) -> &'static str {
    match s {
        LineSide::Old => "LEFT",
        LineSide::New => "RIGHT",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dl(origin: char, old: Option<u32>, new: Option<u32>, text: &str) -> DiffLine {
        DiffLine { origin, old, new, text: text.into() }
    }

    /// ctx 1/1, - old 2, + new 2, + new 3, ctx 3/4, marker.
    fn hunk() -> Hunk {
        Hunk {
            header: "@@ -1,3 +1,4 @@".into(),
            lines: vec![
                dl(' ', Some(1), Some(1), "a"),
                dl('-', Some(2), None, "b"),
                dl('+', None, Some(2), "B"),
                dl('+', None, Some(3), "C"),
                dl(' ', Some(3), Some(4), "d"),
                dl('>', None, None, "\\ No newline at end of file"),
            ],
        }
    }

    fn anchor(line: u32, side: LineSide, start: Option<(u32, LineSide)>) -> Option<LineAnchor> {
        Some(LineAnchor { path: "f.txt".into(), line, side, start })
    }

    #[test]
    fn line_keys_by_origin() {
        assert_eq!(line_key(&dl('-', Some(4), None, "")), Some((LineSide::Old, 4)));
        assert_eq!(line_key(&dl('+', None, Some(5), "")), Some((LineSide::New, 5)));
        assert_eq!(line_key(&dl(' ', Some(4), Some(6), "")), Some((LineSide::New, 6)));
        for m in ['<', '>', '='] {
            assert_eq!(line_key(&dl(m, None, None, "")), None, "{m}");
        }
    }

    #[test]
    fn single_lines() {
        let h = hunk();
        assert_eq!(line_anchor("f.txt", &h, 2, 2), anchor(2, LineSide::New, None), "an added line");
        assert_eq!(line_anchor("f.txt", &h, 1, 1), anchor(2, LineSide::Old, None), "a removed line, old number");
        assert_eq!(line_anchor("f.txt", &h, 4, 4), anchor(4, LineSide::New, None), "a context line, new number");
    }

    #[test]
    fn ranges() {
        let h = hunk();
        assert_eq!(line_anchor("f.txt", &h, 1, 3), anchor(3, LineSide::New, Some((2, LineSide::Old))), "removed to added");
        assert_eq!(line_anchor("f.txt", &h, 0, 1), anchor(2, LineSide::Old, Some((1, LineSide::Old))), "context start before a LEFT end");
        assert_eq!(line_anchor("f.txt", &h, 0, 2), anchor(2, LineSide::New, Some((1, LineSide::New))));
        assert_eq!(line_anchor("f.txt", &h, 3, 1), line_anchor("f.txt", &h, 1, 3), "reversed ends are ordered");
    }

    #[test]
    fn markers_are_skipped_and_ends_clamped() {
        let h = hunk();
        assert_eq!(line_anchor("f.txt", &h, 4, 5), anchor(4, LineSide::New, None), "the marker at the end is skipped");
        assert_eq!(line_anchor("f.txt", &h, 3, 99), anchor(4, LineSide::New, Some((3, LineSide::New))), "clamped");
        assert_eq!(line_anchor("f.txt", &h, 5, 5), None, "only a marker");
        let empty = Hunk { header: String::new(), lines: Vec::new() };
        assert_eq!(line_anchor("f.txt", &empty, 0, 0), None);
    }

    #[test]
    fn suggestion_lines_and_block() {
        let h = hunk();
        assert_eq!(suggested_lines(&h, 0, 4), ["a", "B", "C", "d"], "removed lines are dropped");
        assert_eq!(suggestion_block(&["x", "y"]), "```suggestion\nx\ny\n```");
        assert_eq!(suggestion_block(&["```rust", "x"]), "````suggestion\n```rust\nx\n````", "a longer fence");
        assert_eq!(suggestion_block(&["`` two", "x\r"]), "```suggestion\n`` two\nx\n```", "trailing CR stripped");
        assert_eq!(suggestion_block(&[]), "```suggestion\n```", "an empty block deletes the lines");
        assert_eq!((side_name(LineSide::Old), side_name(LineSide::New)), ("LEFT", "RIGHT"));
    }
}
