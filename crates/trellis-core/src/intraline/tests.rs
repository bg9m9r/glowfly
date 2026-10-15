// Expected marks are written as lists of byte ranges, often with one element.
#![allow(clippy::single_range_in_vec_init)]

use super::*;
use crate::repo::{DiffLine, Hunk};

fn pr(old: &str, new: &str) -> Option<PairMarks> {
    pair_ranges(old, new, &mut MAX_CELLS.clone())
}

fn hunk(lines: &[(char, &str)]) -> Hunk {
    let lines = lines.iter().map(|&(origin, text)| DiffLine { origin, old: None, new: None, text: text.into() }).collect();
    Hunk { header: "@@ -1 +1 @@".into(), lines }
}

fn diff(hunks: Vec<Hunk>) -> FileDiff {
    FileDiff { binary: false, hunks, highlight: None, intraline: Intraline::default(), whole: None }
}

#[test]
fn single_word_change() {
    let got = pr("let total = price * quantity;", "let total = price * count;");
    assert_eq!(got, Some((vec![20..28], vec![20..25])));
}

#[test]
fn insertion_at_end_marks_only_new_side() {
    assert_eq!(pr("foo(a)", "foo(a, b)"), Some((vec![], vec![5..8])));
}

#[test]
fn appended_comment() {
    assert_eq!(pr("let x = 1;", "let x = 2; // two"), Some((vec![8..9], vec![8..9, 11..17])));
}

#[test]
fn unicode_boundaries_respected() {
    let (old, new) = ("naïve café ☕", "naïve cafe ☕");
    let (ro, rn) = pr(old, new).unwrap();
    assert_eq!((ro.clone(), rn.clone()), (vec![7..12], vec![7..11]));
    for (text, rs) in [(old, &ro), (new, &rn)] {
        for r in rs {
            assert!(text.is_char_boundary(r.start as usize) && text.is_char_boundary(r.end as usize), "{text:?} {r:?}");
        }
    }
    let (old, new) = ("a → b", "a ⇒ b");
    let (ro, rn) = pr(old, new).unwrap();
    assert_eq!(&old[ro[0].start as usize..ro[0].end as usize], "→");
    assert_eq!(&new[rn[0].start as usize..rn[0].end as usize], "⇒");
    assert_eq!((ro.len(), rn.len()), (1, 1));
}

#[test]
fn dissimilar_lines_have_no_marks() {
    assert_eq!(pr("fix", "fixed build"), None);
    assert_eq!(pr("let a = compute();", "}"), None);
    assert_eq!(pr("same text", "same text"), None);
    assert_eq!(pr("   ", "  "), None);
}

#[test]
fn whitespace_only_change_marks_the_whitespace() {
    assert_eq!(pr("  foo", "    foo"), Some((vec![0..2], vec![0..4])));
    assert_eq!(pr("foo;", "foo;  "), Some((vec![], vec![4..6])));
}

#[test]
fn adjacent_changes_merge_across_one_space() {
    let (old, new) = ("x = a b;", "x = c d;");
    let (ro, rn) = pr(old, new).unwrap();
    assert_eq!((ro.len(), rn.len()), (1, 1));
    assert_eq!(&old[ro[0].start as usize..ro[0].end as usize], "a b");
    assert_eq!(&new[rn[0].start as usize..rn[0].end as usize], "c d");
}

#[test]
fn caps_and_budget() {
    let long = "a".repeat(MAX_LINE_BYTES + 1);
    assert_eq!(pr(&long, "a"), None);
    assert_eq!(pr("a", &long), None);

    let many_old = format!("x {} y", "a ".repeat(MAX_TOKENS));
    let many_new = format!("x {} y", "b ".repeat(MAX_TOKENS));
    assert!(many_old.len() <= MAX_LINE_BYTES);
    assert_eq!(pr(&many_old, &many_new), None);

    let mut budget = 0;
    assert_eq!(pair_ranges("let x = 1;", "let x = 2;", &mut budget), None);
    assert_eq!(budget, 0);
    let mut budget = 100;
    assert!(pair_ranges("let x = 1;", "let x = 2;", &mut budget).is_some());
    assert!(budget < 100);
}

#[test]
fn intraline_pairs_like_split_rows() {
    let d = diff(vec![
        hunk(&[('-', "let a = 1;"), ('-', "let b = 2;"), ('+', "let a = 9;")]),
        hunk(&[('+', "x = 1"), ('-', "x = 2")]),
        hunk(&[
            (' ', "ctx"),
            ('-', "let v = 1;"),
            ('>', "\\ No newline at end of file"),
            ('+', "let v = 2;"),
            ('<', "\\ No newline at end of file"),
        ]),
    ]);
    let il = intraline(&d);
    assert_eq!(il.hunk_lens(), d.hunks.iter().map(|h| h.lines.len()).collect::<Vec<_>>());
    assert_eq!(il.line(0, 0), &[8..9]);
    assert!(il.line(0, 1).is_empty());
    assert_eq!(il.line(0, 2), &[8..9]);
    assert!(il.line(1, 0).is_empty() && il.line(1, 1).is_empty());
    let marked: Vec<usize> = (0..5).filter(|&l| !il.line(2, l).is_empty()).collect();
    assert_eq!(marked, [1, 3]);

    assert_eq!(intraline(&diff(Vec::new())), Intraline::default());
    let mut binary = d.clone();
    binary.binary = true;
    assert_eq!(intraline(&binary), Intraline::default());
    assert!(il.line(9, 0).is_empty() && il.line(0, 99).is_empty());
    assert!(Intraline::default().line(0, 0).is_empty());
    assert_eq!(il.pairs(), &crate::line_pairs::pair_lines(&d));
}

fn sim(old: &str, new: &str) -> Option<f32> {
    similarity(old, new, &mut MAX_CELLS.clone())
}

#[test]
fn similarity_agrees_with_pair_ranges() {
    let cases = [
        ("let total = price * quantity;", "let total = price * count;"),
        ("let x = 1;", "let x = 2; // two"),
        ("foo(a)", "foo(a, b)"),
        ("fix", "fixed build"),
        ("let a = compute();", "}"),
        ("  foo", "    foo"),
        ("foo;", "foo;  "),
        ("   ", "  "),
        ("host = \"localhost\"", "host = \"0.0.0.0\""),
        ("port = 8080", "# listen on all interfaces"),
        ("ab cd", "ab ef"),
        ("ab cd ef", "ab gh ij"),
        ("abc", "abd"),
        ("x", ""),
    ];
    for (o, n) in cases {
        assert_eq!(sim(o, n).is_some_and(|s| s >= MIN_SIMILARITY), pr(o, n).is_some(), "{o:?} / {n:?}: {:?}", sim(o, n));
    }
}

#[test]
fn similarity_edge_cases() {
    let mut budget = 0;
    assert_eq!(similarity("let x = 1;", "let x = 1;", &mut budget), Some(1.0), "identical lines cost nothing");
    assert_eq!(sim("", ""), None);
    assert_eq!(sim("   ", "   "), None);
    assert_eq!(sim("   ", "\t"), None);
    let long = "a ".repeat(MAX_LINE_BYTES);
    assert_eq!(sim(&long, "a"), None);
    assert_eq!(sim("a", &long), None);
    let mut budget = 0;
    assert_eq!(similarity("let x = 1;", "let x = 2;", &mut budget), None);
    let s = sim("let x = 1;", "let x = 2;").expect("scored");
    assert!((s - 12.0 / 14.0).abs() < 1e-6, "{s}");
}

#[test]
fn marks_follow_similarity_pairs() {
    let d = diff(vec![hunk(&[
        (' ', "[server]"),
        ('-', "host = \"localhost\""),
        ('-', "port = 8080"),
        ('+', "# listen on all interfaces"),
        ('+', "host = \"0.0.0.0\""),
        ('+', "port = 9090"),
        (' ', "timeout = 30"),
    ])]);
    let il = intraline(&d);
    let marked: Vec<usize> = (0..7).filter(|&l| !il.line(0, l).is_empty()).collect();
    assert_eq!(marked, [1, 2, 4, 5]);
    assert_eq!(il.pairs().hunk(0), [(1, 4), (2, 5)]);
    let host = &d.hunks[0].lines[4].text;
    assert_eq!(il.line(0, 4).iter().map(|r| &host[r.start as usize..r.end as usize]).collect::<Vec<_>>(), ["0.0.0.0"]);
}
