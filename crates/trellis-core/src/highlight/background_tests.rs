//! The background pass (`finish`), its budget, cancellation, and what reloads of large diffs
//! take over from the diff shown before.

use super::tests::{assert_covers, diff, hunk, kinds};
use super::*;
use crate::repo::Hunk;
use std::cell::Cell;

fn shape(d: &FileDiff) -> Vec<usize> {
    d.hunks.iter().map(|h| h.lines.len()).collect()
}

/// One hunk of added lines.
fn added(lines: &[String]) -> FileDiff {
    let pairs: Vec<_> = lines.iter().map(|l| ('+', l.as_str())).collect();
    diff(vec![hunk(&pairs)])
}

/// `let xN = N;` for N in 0..n.
fn lets(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("let x{i} = {i};")).collect()
}

fn has_keyword(spans: &[Span]) -> bool {
    kinds(spans).contains(&TokenKind::Keyword)
}

/// `d` with its highlight finished in the background.
fn finished(mut d: FileDiff) -> FileDiff {
    d.highlight = highlight_diff("a.rs", &d, None);
    assert!(d.highlight.as_ref().is_some_and(DiffHighlight::is_pending), "over the inline limits");
    d.highlight = finish(&d, &|| false);
    d
}

#[test]
fn pending_diff_is_finished_in_the_background() {
    let mut d = added(&lets(MAX_LINES + 1));
    let hl = highlight_diff("a.rs", &d, None).unwrap();
    assert!(hl.is_pending());
    assert!((0..=MAX_LINES).all(|l| hl.line(0, l).is_empty()));
    d.highlight = Some(hl);

    let done = finish(&d, &|| false).expect("finished");
    assert!(!done.is_pending());
    assert_eq!(done.hunk_lens(), shape(&d));
    for (l, dl) in d.hunks[0].lines.iter().enumerate() {
        assert!(has_keyword(done.line(0, l)), "line {l}: {:?}", done.line(0, l));
        assert_covers(done.line(0, l), &dl.text);
    }
    // Only pending highlights are finished.
    d.highlight = Some(done);
    assert_eq!(finish(&d, &|| false), None);
}

#[test]
fn background_budget() {
    let short = |n: usize| (0..n).map(|_| "x".to_owned()).collect::<Vec<_>>();
    assert_eq!(highlight_diff("a.rs", &added(&short(BACKGROUND_MAX_LINES + 1)), None), None);
    let bulky = vec!["x".repeat(MAX_LINE_BYTES); BACKGROUND_MAX_BYTES / MAX_LINE_BYTES + 1];
    assert_eq!(highlight_diff("a.rs", &added(&bulky), None), None);
    assert!(highlight_diff("a.rs", &added(&short(BACKGROUND_MAX_LINES)), None).is_some_and(|h| h.is_pending()));

    // Long lines don't count: 50 of them (over MAX_BYTES together) plus a few lines of code are
    // highlighted inline, the long lines left plain.
    let mut lines = vec!["y".repeat(MAX_LINE_BYTES + 1); 50];
    lines.extend(lets(10));
    let hl = highlight_diff("a.rs", &added(&lines), None).expect("highlighted");
    assert!(!hl.is_pending());
    assert!((0..50).all(|l| hl.line(0, l).is_empty()));
    assert!((50..60).all(|l| has_keyword(hl.line(0, l))));
}

#[test]
fn finish_leaves_long_lines_plain() {
    // Over the inline limits even without the long line, which doesn't count.
    let mut lines = lets(MAX_LINES + 2);
    lines[100] = format!("let min = [{}];", "1,".repeat(MAX_LINE_BYTES));
    let d = finished(added(&lines));
    let hl = d.highlight.unwrap();
    assert_eq!(hl.line(0, 100), &[]);
    assert!(has_keyword(hl.line(0, 99)) && has_keyword(hl.line(0, 101)));
}

#[test]
fn cancelled_finish_returns_none() {
    let mut d = added(&lets(MAX_LINES + 1));
    d.highlight = highlight_diff("a.rs", &d, None);
    assert_eq!(finish(&d, &|| true), None);
    // Cancelled while parsing, after the first check.
    let checks = Cell::new(0);
    let later = || {
        checks.set(checks.get() + 1);
        checks.get() > 1
    };
    assert_eq!(finish(&d, &later), None);
    assert!(checks.get() > 1);
}

/// Hunk `i`: three context lines, one changed line, three context lines.
fn small_hunk(i: usize) -> Hunk {
    let c = |k: usize| format!("let c{i}_{k} = {k};");
    let (old, new) = (format!("let v{i} = {i};"), format!("let v{i} = {i} + 1;"));
    let lines = [(' ', c(0)), (' ', c(1)), (' ', c(2)), ('-', old), ('+', new), (' ', c(3)), (' ', c(4)), (' ', c(5))];
    let pairs: Vec<_> = lines.iter().map(|(o, t)| (*o, t.as_str())).collect();
    hunk(&pairs)
}

#[test]
fn reload_of_big_diff_reuses_unchanged_hunks_inline() {
    let mut before = finished(diff((0..300).map(small_hunk).collect()));
    let mut marked = before.highlight.clone().unwrap();
    // Mark the spans so reuse is visible: a fresh parse would never produce this.
    marked.lines[5][0] = vec![Span { start: 0, end: 1, kind: TokenKind::Attribute }];
    before.highlight = Some(marked);

    // Hunk 0 was staged: the others come back with shifted line numbers.
    let mut hunks: Vec<Hunk> = (1..300).map(small_hunk).collect();
    for l in hunks.iter_mut().flat_map(|h| &mut h.lines) {
        l.new = l.new.map(|n| n - 1);
    }
    let after = diff(hunks);
    let hl = highlight_diff("a.rs", &after, Some(&before)).expect("highlighted");
    assert!(!hl.is_pending(), "nothing left to parse: no background pass");
    let prev = before.highlight.as_ref().unwrap();
    for h in 0..299 {
        assert_eq!(hl.lines[h], prev.lines[h + 1], "hunk {h}");
    }
    assert_eq!(hl.line(4, 0), [Span { start: 0, end: 1, kind: TokenKind::Attribute }]);
}

#[test]
fn pending_reload_carries_spans_by_text() {
    let mut lines: Vec<(char, String)> = vec![(' ', "fn main() {".into())];
    lines.extend(lets(MAX_LINES).into_iter().map(|l| ('+', l)));
    lines.push(('+', "let z = \"q\";".into()));
    let to_diff = |lines: &[(char, String)]| {
        let pairs: Vec<_> = lines.iter().map(|(o, t)| (*o, t.as_str())).collect();
        diff(vec![hunk(&pairs)])
    };
    let before = finished(to_diff(&lines));
    let prev = before.highlight.as_ref().unwrap();

    lines[6].1 = "let x5 = 500;".into();
    // A removed line with the text of an added one: the side is part of the key.
    lines.push(('-', "let z = \"q\";".into()));
    let after = to_diff(&lines);
    let hl = highlight_diff("a.rs", &after, Some(&before)).expect("highlighted");
    assert!(hl.is_pending());
    assert_eq!(hl.hunk_lens(), shape(&after));
    for l in (0..lines.len() - 1).filter(|&l| l != 6) {
        assert!(!prev.line(0, l).is_empty(), "line {l}");
        assert_eq!(hl.line(0, l), prev.line(0, l), "line {l} keeps its spans");
    }
    assert_eq!(hl.line(0, 6), &[], "new text has nothing to carry");
    assert_eq!(hl.line(0, lines.len() - 1), &[], "a removed line never takes an added line's spans");
}

#[test]
fn pending_previous_hunks_are_not_reused_exactly() {
    let d = added(&lets(MAX_LINES + 1));
    let mut before = finished(d.clone());
    let hl = before.highlight.as_mut().unwrap();
    hl.lines[0][3] = vec![Span { start: 0, end: 1, kind: TokenKind::Attribute }];
    hl.pending = vec![true];
    let prev = hl.clone();

    let again = highlight_diff("a.rs", &d, Some(&before)).expect("highlighted");
    assert!(again.is_pending(), "a provisional hunk is parsed again, not reused");
    for l in 0..=MAX_LINES {
        assert_eq!(again.line(0, l), prev.line(0, l), "line {l} carries its spans by text");
    }
}
