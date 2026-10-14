use super::*;
use crate::repo::{self, DiffTarget, Hunk};
use crate::testutil::{git, scratch_repo, write};
use std::collections::HashSet;

/// A hunk from (origin, text) pairs, numbered from line 1 on both sides.
fn hunk(lines: &[(char, &str)]) -> Hunk {
    let (mut old, mut new) = (1, 1);
    let lines = lines
        .iter()
        .map(|&(origin, text)| {
            let o = matches!(origin, ' ' | '-').then_some(old);
            let n = matches!(origin, ' ' | '+').then_some(new);
            old += o.is_some() as u32;
            new += n.is_some() as u32;
            DiffLine { origin, old: o, new: n, text: text.into() }
        })
        .collect();
    Hunk { header: "@@ -1 +1 @@".into(), lines }
}

fn diff(hunks: Vec<Hunk>) -> FileDiff {
    FileDiff { binary: false, hunks, highlight: None, intraline: Default::default(), whole: None }
}

fn kinds(spans: &[Span]) -> HashSet<TokenKind> {
    spans.iter().map(|s| s.kind).collect()
}

/// Spans are contiguous and cover the whole line.
fn assert_covers(spans: &[Span], text: &str) {
    if text.is_empty() {
        assert!(spans.is_empty(), "{spans:?}");
        return;
    }
    assert_eq!(spans.first().map(|s| s.start), Some(0), "{text:?}: {spans:?}");
    assert_eq!(spans.last().map(|s| s.end as usize), Some(text.len()), "{text:?}: {spans:?}");
    for w in spans.windows(2) {
        assert_eq!(w[0].end, w[1].start, "{text:?}: {spans:?}");
    }
}

#[test]
fn rust_diff_has_several_kinds() {
    let d = diff(vec![hunk(&[(' ', "fn main() {"), ('-', "    let x = 1; // old"), ('+', "    let s = \"hi\"; // new")])]);
    let hl = highlight_diff("src/main.rs", &d, None).expect("rust is highlighted");
    assert_eq!(hl.syntax, "Rust");
    for (l, dl) in d.hunks[0].lines.iter().enumerate() {
        assert_covers(hl.line(0, l), &dl.text);
    }
    let added = kinds(hl.line(0, 2));
    for k in [TokenKind::String, TokenKind::Comment, TokenKind::Keyword] {
        assert!(added.contains(&k), "added line lacks {k:?}: {:?}", hl.line(0, 2));
    }
    let let_span = hl.line(0, 2).iter().find(|s| s.start == 4).unwrap();
    assert_eq!((let_span.end, let_span.kind), (7, TokenKind::Keyword), "`let` is a keyword");
    let removed = kinds(hl.line(0, 1));
    assert!(removed.contains(&TokenKind::Number) && removed.contains(&TokenKind::Comment), "{:?}", hl.line(0, 1));
    let all: HashSet<_> = (0..3).flat_map(|l| kinds(hl.line(0, l))).collect();
    assert!(all.len() >= 3, "{all:?}");
}

#[test]
fn unknown_extension_is_plain() {
    let d = diff(vec![hunk(&[('+', "hello world")])]);
    assert_eq!(highlight_diff("notes.xyz", &d, None), None);
    assert_eq!(highlight_diff("README.txt", &d, None), None, "Plain Text counts as no language");
    assert_eq!(syntax_name("Makefile", None), Some("Makefile"));
    assert_eq!(syntax_name("build/Makefile", None), Some("Makefile"));
    let shell = syntax_name("run", Some("#!/bin/bash")).expect("shebang picks a shell syntax");
    assert!(shell.contains("Shell") || shell.contains("bash"), "{shell}");
    assert_eq!(syntax_name("run", None), None);
    assert_eq!(syntax_name("a.rs", None), Some("Rust"));
    assert_eq!(syntax_name("x.py", None), Some("Python"));
}

#[test]
fn shebang_on_first_line_picks_the_language() {
    let d = diff(vec![hunk(&[('+', "#!/bin/bash"), ('+', "echo \"hi\" # greet")])]);
    let hl = highlight_diff("scripts/run", &d, None).expect("detected from the first line");
    assert!(kinds(hl.line(0, 1)).contains(&TokenKind::Comment), "{:?}", hl.line(0, 1));
}

#[test]
fn plain_identifier_is_one_plain_span() {
    let d = diff(vec![hunk(&[('+', "foo"), ('+', ""), ('+', "let y = 2;")])]);
    let hl = highlight_diff("a.rs", &d, None).unwrap();
    assert_eq!(hl.line(0, 0), &[Span { start: 0, end: 3, kind: TokenKind::Plain }]);
    assert_eq!(hl.line(0, 1), &[]);
}

#[test]
fn binary_and_oversized_are_plain() {
    let mut d = diff(vec![hunk(&[('+', "let x = 1;")])]);
    assert!(highlight_diff("a.rs", &d, None).is_some());
    d.binary = true;
    assert_eq!(highlight_diff("a.rs", &d, None), None);

    let many: Vec<_> = (0..=MAX_LINES).map(|_| ('+', "let x = 1;")).collect();
    assert_eq!(highlight_diff("a.rs", &diff(vec![hunk(&many)]), None), None);
    let at_limit: Vec<_> = (0..MAX_LINES).map(|_| ('+', "x")).collect();
    assert!(highlight_diff("a.rs", &diff(vec![hunk(&at_limit)]), None).is_some());

    // Total text over the limit, in lines that are each short enough.
    let line = "x".repeat(MAX_BYTES / 50 + 1);
    let bulky: Vec<_> = (0..50).map(|_| ('+', line.as_str())).collect();
    assert_eq!(highlight_diff("a.rs", &diff(vec![hunk(&bulky)]), None), None);

    let long = "x".repeat(MAX_LINE_BYTES + 1);
    assert_eq!(highlight_diff("a.rs", &diff(vec![hunk(&[('+', "let a = 1;"), ('+', &long)])]), None), None);
    assert!(highlight_diff("a.rs", &diff(vec![hunk(&[('+', &long[1..])])]), None).is_some());

    assert_eq!(highlight_diff("a.rs", &diff(Vec::new()), None), None);
}

#[test]
fn sides_highlight_separately() {
    // The removed line opens a string that only the old side sees.
    let d = diff(vec![hunk(&[
        (' ', "fn f() {"),
        ('-', "let a = \""),
        ('-', "x\";"),
        ('+', "y // z"),
        (' ', "}"),
        ('=', "\\ No newline at end of file"),
        ('<', "\\ No newline at end of file"),
        ('>', "\\ No newline at end of file"),
    ])]);
    let hl = highlight_diff("a.rs", &d, None).unwrap();
    assert_eq!(hl.line(0, 2).first().map(|s| s.kind), Some(TokenKind::String), "old side is inside the string");
    assert!(kinds(hl.line(0, 3)).contains(&TokenKind::Comment), "new side never saw the string: {:?}", hl.line(0, 3));
    assert!(!kinds(hl.line(0, 3)).contains(&TokenKind::String));
    for l in 5..8 {
        assert_eq!(hl.line(0, l), &[], "EOF marker {l} is left plain");
    }
    // Each hunk starts fresh: an unterminated string in hunk 0 doesn't leak into hunk 1.
    let d = diff(vec![hunk(&[('+', "let a = \"")]), hunk(&[('+', "b // c")])]);
    let hl = highlight_diff("a.rs", &d, None).unwrap();
    assert!(kinds(hl.line(1, 0)).contains(&TokenKind::Comment), "{:?}", hl.line(1, 0));
}

#[test]
fn line_out_of_range_is_empty() {
    assert_eq!(DiffHighlight::default().line(3, 9), &[]);
    let hl = highlight_diff("a.rs", &diff(vec![hunk(&[('+', "let x = 1;")])]), None).unwrap();
    assert_eq!(hl.line(0, 1), &[]);
    assert_eq!(hl.line(1, 0), &[]);
}

#[test]
fn highlights_a_loaded_diff() {
    let dir = scratch_repo("highlight-load");
    write(&dir, "lib.rs", "pub fn one() -> u32 { 1 }\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "one"]);
    write(&dir, "lib.rs", "pub fn one() -> u32 { 2 } // two\n");
    let d = repo::load_diff(&dir, &DiffTarget::Unstaged("lib.rs".into())).unwrap();
    assert!(d.highlight.is_none(), "load_diff does git only");
    let hl = highlight_diff("lib.rs", &d, None).expect("rust is highlighted");
    assert_eq!(hl.hunk_lens(), d.hunks.iter().map(|h| h.lines.len()).collect::<Vec<_>>());
    let (h, l) = d.hunks.iter().enumerate().find_map(|(h, hk)| hk.lines.iter().position(|l| l.origin == '+').map(|l| (h, l))).unwrap();
    let added = kinds(hl.line(h, l));
    assert!(added.contains(&TokenKind::Number) && added.contains(&TokenKind::Comment), "{:?}", hl.line(h, l));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deleted_script_detected_from_its_old_first_line() {
    let mut d = diff(vec![hunk(&[('-', "#!/usr/bin/env python3"), ('-', "print('hi')")])]);
    for (i, l) in d.hunks[0].lines.iter_mut().enumerate() {
        l.old = Some(i as u32 + 1);
        l.new = None;
    }
    assert_eq!(highlight_diff("scripts/run", &d, None).map(|h| h.syntax), Some("Python".to_owned()));
}

#[test]
fn reload_reuses_unchanged_hunks() {
    let first = hunk(&[('+', "let a = 1;")]);
    let second = hunk(&[('+', "let b = \"two\";")]);
    let mut before = diff(vec![first, second]);
    before.highlight = highlight_diff("a.rs", &before, None);
    let mut marked = before.highlight.clone().unwrap();
    // Mark the spans so reuse is visible: a fresh parse would never produce this.
    marked.lines[1][0] = vec![Span { start: 0, end: 1, kind: TokenKind::Attribute }];
    before.highlight = Some(marked);

    // After a reload the second hunk is unchanged (only its line numbers moved) and the first
    // is gone: the second keeps the old spans, a new hunk is parsed.
    let mut moved = hunk(&[('+', "let b = \"two\";")]);
    moved.lines[0].new = Some(40);
    let after = diff(vec![moved, hunk(&[('+', "fn c() {}")])]);
    let hl = highlight_diff("a.rs", &after, Some(&before)).unwrap();
    assert_eq!(hl.line(0, 0), [Span { start: 0, end: 1, kind: TokenKind::Attribute }]);
    assert!(hl.line(1, 0).iter().any(|s| s.kind == TokenKind::Keyword));

    // Spans from another language are never reused.
    let other = highlight_diff("a.py", &after, Some(&before)).unwrap();
    assert_ne!(other.line(0, 0), [Span { start: 0, end: 1, kind: TokenKind::Attribute }]);
}
