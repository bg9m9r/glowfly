//! Syntax spans for the unchanged lines between hunks, when the hunk view fills them in: the old
//! file parsed from its first line, so a run inside a block comment opened earlier is colored
//! as one.

use super::context::{old_count, starts};
use super::parse::{Classifier, Side};
use super::{BACKGROUND_MAX_BYTES, BACKGROUND_MAX_LINES, MAX_LINE_BYTES, OldText, SYNTAXES, Span};
use crate::repo::Hunk;

/// Per hunk, the spans of each old line between the previous hunk and it (none before the first
/// hunk), from the old file parsed in order in `syntax` (a syntax name, as
/// `DiffHighlight::syntax`). `None` to show them plain: no or an unknown syntax, hunks that don't
/// fit `old`, more to parse than `BACKGROUND_MAX_LINES` or `BACKGROUND_MAX_BYTES`, or a parse
/// error. Lines over `MAX_LINE_BYTES` stay plain and aren't parsed.
pub fn gap_spans(syntax: Option<&str>, old: &OldText, hunks: &[Hunk]) -> Option<Vec<Vec<Vec<Span>>>> {
    let syntax = SYNTAXES.set.find_syntax_by_name(syntax?)?;
    old.gaps(hunks)?;
    let starts = starts(hunks)?;
    let lines = old.lines();
    let end = starts.last().copied().unwrap_or(0).min(lines.len());
    let parsed = lines[..end].iter().filter(|l| l.len() <= MAX_LINE_BYTES);
    let (count, bytes) = parsed.fold((0, 0), |(n, b), l| (n + 1, b + l.len()));
    if count > BACKGROUND_MAX_LINES || bytes > BACKGROUND_MAX_BYTES {
        return None;
    }
    let mut side = Side::new(syntax);
    let mut cx = Classifier::new();
    let mut spans = Vec::with_capacity(end);
    for l in &lines[..end] {
        spans.push(if l.len() <= MAX_LINE_BYTES { side.line(l, &mut cx, &|| false).ok()? } else { Vec::new() });
    }
    let mut out = vec![Vec::new()];
    for h in 1..hunks.len() {
        let from = starts[h - 1] + old_count(&hunks[h - 1]);
        out.push(spans.get(from..starts[h])?.to_vec());
    }
    Some(out)
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
        assert_eq!(spans.len(), 3);
        assert!(spans[0].is_empty());
        // Gap before hunk 2: old lines 11..=23, inside the comment until "*/".
        assert_eq!(spans[2].len(), 13);
        for line in &spans[2][..11] {
            assert!(!line.is_empty() && line.iter().all(|s| s.kind == TokenKind::Comment), "{line:?}");
        }
        assert_eq!(gap_spans(None, &old, &hunks), None);
        assert_eq!(gap_spans(Some("Klingon"), &old, &hunks), None);
    }
}
