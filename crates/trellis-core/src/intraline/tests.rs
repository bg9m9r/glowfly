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
    FileDiff { binary: false, hunks, highlight: None, intraline: Intraline::default() }
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
}
