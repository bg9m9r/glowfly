//! Syntax spans for the unchanged lines between hunks, when the hunk view fills them in: the old
//! file parsed from its first line, so a run inside a block comment opened earlier is colored
//! as one.

use super::context::{old_count, starts};
use super::parse::{Classifier, Side};
use super::{BACKGROUND_MAX_BYTES, BACKGROUND_MAX_LINES, MAX_LINE_BYTES, OldText, SYNTAXES, Span};
use crate::repo::Hunk;

/// Per hunk, the spans of each old line between the previous hunk and it (entry 0: the lines
/// above the first hunk), then one more entry for the lines below the last hunk: `hunks.len() +
/// 1` entries, from the old file parsed in order in `syntax` (a syntax name, as
/// `DiffHighlight::syntax`). The last entry is empty (plain) when `old` has no lines after the
/// last hunk (it wasn't built with `OldText::whole`) or the whole file is more than the limits
/// allow, which leaves the runs before the last hunk colored as they were. `None` to show every
/// run plain: no or an unknown syntax, hunks that don't fit `old`, more to parse up to the last
/// hunk than `BACKGROUND_MAX_LINES` or `BACKGROUND_MAX_BYTES`, or a parse error. Lines over
/// `MAX_LINE_BYTES` stay plain and aren't parsed.
pub fn gap_spans(syntax: Option<&str>, old: &OldText, hunks: &[Hunk]) -> Option<Vec<Vec<Vec<Span>>>> {
    let syntax = SYNTAXES.set.find_syntax_by_name(syntax?)?;
    old.gaps(hunks)?;
    let starts = starts(hunks)?;
    let lines = old.lines();
    let end = starts.last().copied().unwrap_or(0).min(lines.len());
    let (count, bytes) = cost(&lines[..end]);
    if count > BACKGROUND_MAX_LINES || bytes > BACKGROUND_MAX_BYTES {
        return None;
    }
    let (rest_count, rest_bytes) = cost(old.rest());
    let with_rest = !old.rest().is_empty() && count + rest_count <= BACKGROUND_MAX_LINES && bytes + rest_bytes <= BACKGROUND_MAX_BYTES;
    let mut side = Side::new(syntax);
    let mut cx = Classifier::new();
    let mut spans = Vec::with_capacity(end);
    let mut parse = |l: &String| -> Option<Vec<Span>> {
        Some(if l.len() <= MAX_LINE_BYTES { side.line(l, &mut cx, &|| false).ok()? } else { Vec::new() })
    };
    for l in &lines[..end] {
        spans.push(parse(l)?);
    }
    let mut below = Vec::new();
    if with_rest {
        for l in old.rest() {
            below.push(parse(l)?);
        }
    }
    let mut out = vec![spans.get(..starts.first().copied().unwrap_or(0))?.to_vec()];
    for h in 1..hunks.len() {
        let from = starts[h - 1] + old_count(&hunks[h - 1]);
        out.push(spans.get(from..starts[h])?.to_vec());
    }
    out.push(hunks.last().and_then(|last| below.get(old_count(last)..)).map(<[_]>::to_vec).unwrap_or_default());
    Some(out)
}

/// The lines and bytes parsing `lines` costs (lines over `MAX_LINE_BYTES` aren't parsed).
fn cost(lines: &[String]) -> (usize, usize) {
    lines.iter().filter(|l| l.len() <= MAX_LINE_BYTES).fold((0, 0), |(n, b), l| (n + 1, b + l.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highlight::TokenKind;
    use crate::highlight::context_tests::hunk_at;

    #[test]
    fn a_gap_inside_a_comment_opened_in_an_earlier_gap_is_comment() {
        let mut text: Vec<String> = vec!["fn a() {}".into(), "/*".into()];
        text.extend((0..20).map(|i| format!("let x{i} = {i};")));
        text.push("*/".into());
        text.push("fn b() {}".into());
        let content = text.join("\n") + "\n";
        // Hunks at line 1 (fn a), line 10 (inside the comment) and line 24 (fn b).
        let hunks = vec![
            hunk_at(0, &[('-', "fn a() {}"), ('+', "fn a2() {}")]),
            hunk_at(9, &[('-', "let x7 = 7;"), ('+', "let x7 = 70;")]),
            hunk_at(23, &[('-', "fn b() {}"), ('+', "fn b2() {}")]),
        ];
        let old = OldText::new(content.as_bytes(), &hunks).expect("fits");
        let spans = gap_spans(Some("Rust"), &old, &hunks).expect("highlighted");
        assert_eq!(spans.len(), 4, "one per hunk, and one for the lines below the last");
        assert!(spans[0].is_empty());
        assert!(spans[3].is_empty(), "no lines after the last hunk");
        // Gap before hunk 2: old lines 11..=23, inside the comment until "*/".
        assert_eq!(spans[2].len(), 13);
        for line in &spans[2][..11] {
            assert!(!line.is_empty() && line.iter().all(|s| s.kind == TokenKind::Comment), "{line:?}");
        }
        assert_eq!(gap_spans(None, &old, &hunks), None);
        assert_eq!(gap_spans(Some("Klingon"), &old, &hunks), None);
    }

    #[test]
    fn the_edges_are_colored_and_the_tail_continues_the_parse() {
        let mut text: Vec<String> = (1..=4).map(|i| format!("let a{i} = {i};")).collect();
        text.push("/* in the last hunk".into());
        text.extend((0..3).map(|i| format!("still comment {i}")));
        text.push("*/ let end = 1;".into());
        text.extend((0..2).map(|i| format!("let tail{i} = {i};")));
        let content = text.join(
            "
",
        ) + "
";
        let hunks = vec![hunk_at(4, &[('-', "/* in the last hunk"), ('+', "/* in the last hunk!"), (' ', "still comment 0")])];
        let old = OldText::whole(content.as_bytes(), &hunks).expect("fits");
        let spans = gap_spans(Some("Rust"), &old, &hunks).expect("highlighted");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].len(), 4, "the lines above a first hunk that starts at line 5");
        assert!(spans[0].iter().all(|l| l.iter().any(|s| s.kind == TokenKind::Keyword)));
        // After the hunk's old lines (2 here): `still comment 1`, `still comment 2`, `*/ ...`, tail.
        assert_eq!(spans[1].len(), 5);
        for line in &spans[1][..2] {
            assert!(!line.is_empty() && line.iter().all(|s| s.kind == TokenKind::Comment), "{line:?}");
        }
        assert!(spans[1][3].iter().any(|s| s.kind == TokenKind::Keyword));
        // Built without the tail: that entry is empty.
        let short = OldText::new(content.as_bytes(), &hunks).expect("fits");
        let spans = gap_spans(Some("Rust"), &short, &hunks).expect("highlighted");
        assert_eq!((spans.len(), spans[0].len(), spans[1].len()), (2, 4, 0));
    }
}
