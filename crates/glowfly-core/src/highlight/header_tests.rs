//! The function context in hunk headers: where it starts, how it is parsed (standalone, inline
//! or in the background), and how reloads carry it.

use super::context_tests::hunk_at;
use super::tests::{assert_covers, diff, hunk, kinds};
use super::*;
use crate::repo::Hunk;

const GREET: &str = "@@ -10,3 +10,3 @@ fn greet(name: &str) -> String {";

fn with_header(mut h: Hunk, header: &str) -> Hunk {
    h.header = header.into();
    h
}

fn has(spans: &[Span], kind: TokenKind) -> bool {
    kinds(spans).contains(&kind)
}

#[test]
fn context_start_finds_the_text_after_the_marker() {
    let h = "@@ -10,7 +10,8 @@ fn main() {";
    assert_eq!(context_start(h), Some(18));
    assert_eq!(&h[18..], "fn main() {");
    assert_eq!(context_start("@@ -1 +1 @@"), None, "no context");
    assert_eq!(context_start("@@ -1 +1 @@   "), None, "only whitespace");
    assert_eq!(context_start("@@ -1 +1"), None, "no closing marker");
    assert_eq!(context_start("fn x"), None);
    assert_eq!(context_start("@ -1 +1 @ fn x"), None, "a single @ is no marker");
    let combined = "@@@ -1,2 -1,2 +1,3 @@@ fn f()";
    assert_eq!(context_start(combined).map(|at| &combined[at..]), Some("fn f()"));
    let tabbed = "@@ -1 +1 @@ \tint x;";
    assert_eq!(context_start(tabbed).map(|at| &tabbed[at..]), Some("\tint x;"), "only one space is skipped");
}

#[test]
fn header_context_is_highlighted_inline() {
    let d = diff(vec![with_header(hunk(&[(' ', "    let a = 1;"), ('-', "    a"), ('+', "    a + 1")]), GREET)]);
    let hl = highlight_diff("a.rs", &d, None).expect("rust");
    assert!(!hl.is_pending());
    let start = context_start(GREET).unwrap();
    let spans = hl.header(0);
    assert_covers(spans, &GREET[start..]);
    assert!(spans.iter().any(|s| (s.start, s.end, s.kind) == (0, 2, TokenKind::Keyword)), "{spans:?}");
    let greet = GREET[start..].find("greet").unwrap() as u32;
    assert!(spans.iter().any(|s| s.kind == TokenKind::Function && s.start <= greet && s.end >= greet + 5), "{spans:?}");
}

#[test]
fn header_context_is_parsed_on_its_own() {
    // The hunk opens a block comment that never closes: the header is still code.
    let d = diff(vec![with_header(hunk(&[('+', "/*"), ('+', "let x = 1;")]), GREET)]);
    let hl = highlight_diff("a.rs", &d, None).unwrap();
    assert!(has(hl.line(0, 1), TokenKind::Comment), "the lines are a comment");
    assert_eq!(hl.header(0).first().map(|s| s.kind), Some(TokenKind::Keyword), "{:?}", hl.header(0));

    // In context: the old text opens a block comment before the hunk.
    let content = "/*\nx\ny\nz\na\n";
    let h = with_header(hunk_at(3, &[(' ', "z"), ('-', "a"), ('+', "b")]), "@@ -4,2 +4,2 @@ fn foo() {");
    let old = OldText::new(content.as_bytes(), std::slice::from_ref(&h)).expect("fits the blob");
    let d = diff(vec![h]);
    let hl = highlight_in_context("a.rs", &d, None, Some(&Arc::new(old))).unwrap();
    assert!(!hl.is_pending());
    assert!(has(hl.line(0, 0), TokenKind::Comment), "the hunk is inside the comment: {:?}", hl.line(0, 0));
    assert!(hl.header(0).iter().any(|s| (s.start, s.end, s.kind) == (0, 2, TokenKind::Keyword)), "{:?}", hl.header(0));
    assert!(!has(hl.header(0), TokenKind::Comment));
}

#[test]
fn headers_without_context_stay_empty() {
    let lines = [('-', "let a = 1;"), ('+', "let a = 2;")];
    let d = diff(vec![hunk(&lines)]);
    let hl = highlight_diff("a.rs", &d, None).unwrap();
    assert_eq!(hl.header(0), &[]);
    assert_eq!(hl.header(99), &[], "out of range");

    let long = format!("@@ -1 +1 @@ fn x() {{ {} }}", "a".repeat(MAX_LINE_BYTES));
    let hl = highlight_diff("a.rs", &diff(vec![with_header(hunk(&lines), &long)]), None).unwrap();
    assert_eq!(hl.header(0), &[], "too long");
    assert_eq!(highlight_diff("a.unknownext", &diff(vec![with_header(hunk(&lines), GREET)]), None), None);
}

/// 300 hunks of `let` lines against a 3,000-line file (over `MAX_LINES`), each header saying
/// `context(i)`, plus the file's text for an in-context parse.
fn big(context: impl Fn(usize) -> &'static str) -> (FileDiff, Arc<OldText>) {
    let content: String = (1..=3000).map(|k| format!("let v{k} = {k};\n")).collect();
    let line = |k: usize| format!("let v{k} = {k};");
    let hunks: Vec<Hunk> = (0..300)
        .map(|i| {
            let s = i * 10;
            let lines = [
                (' ', line(s + 1)),
                (' ', line(s + 2)),
                (' ', line(s + 3)),
                ('-', line(s + 4)),
                ('+', format!("let v{} = 0;", s + 4)),
                (' ', line(s + 5)),
                (' ', line(s + 6)),
                (' ', line(s + 7)),
            ];
            let pairs: Vec<_> = lines.iter().map(|(o, t)| (*o, t.as_str())).collect();
            with_header(hunk_at(s, &pairs), &format!("@@ -{0},7 +{0},7 @@ {1}", s + 1, context(i)))
        })
        .collect();
    let old = OldText::new(content.as_bytes(), &hunks).expect("fits the blob");
    (diff(hunks), Arc::new(old))
}

#[test]
fn pending_headers_are_finished_in_the_background() {
    let (mut d, old) = big(|_| "fn main() {");
    for old in [None, Some(&old)] {
        let hl = highlight_in_context("a.rs", &d, None, old).expect("highlighted");
        assert!(hl.is_pending());
        assert!((0..d.hunks.len()).all(|h| hl.header(h).is_empty()), "plain while pending");
        d.highlight = Some(hl);
        assert_eq!(finish(&d, &|| true), None, "cancelled");
        let done = finish(&d, &|| false).expect("finished");
        assert_eq!(done.old_text.is_some(), old.is_some());
        for h in 0..d.hunks.len() {
            assert!(has(done.header(h), TokenKind::Keyword), "hunk {h}: {:?}", done.header(h));
        }
    }
}

#[test]
fn pending_reload_carries_header_spans_by_context_text() {
    let lines = [(' ', "let a = 1;"), ('-', "let b = 1;"), ('+', "let b = 2;")];
    let mut before = diff(vec![with_header(hunk(&lines), "@@ -1,2 +1,2 @@ fn main() {")]);
    before.highlight = highlight_diff("a.rs", &before, None);
    let prev = before.highlight.as_ref().unwrap();
    assert!(!prev.is_pending() && has(prev.header(0), TokenKind::Keyword));

    let (mut d, _) = big(|i| if i == 7 { "fn other() {" } else { "fn main() {" });
    let hl = highlight_diff("a.rs", &d, Some(&before)).unwrap();
    assert!(hl.is_pending());
    for h in (0..d.hunks.len()).filter(|&h| h != 7) {
        assert_eq!(hl.header(h), prev.header(0), "hunk {h} carries its header's spans");
    }
    assert_eq!(hl.header(7), &[], "new context text waits for the background pass");
    d.highlight = Some(hl);
    let done = finish(&d, &|| false).unwrap();
    assert!(has(done.header(7), TokenKind::Function), "{:?}", done.header(7));
}

#[test]
fn unchanged_reload_keeps_header_spans() {
    let content = "fn foo() {\nx\ny\nz\na\n";
    let h = with_header(hunk_at(3, &[(' ', "z"), ('-', "a"), ('+', "b")]), "@@ -4,2 +4,2 @@ fn foo() {");
    let old = Arc::new(OldText::new(content.as_bytes(), std::slice::from_ref(&h)).unwrap());
    let mut d = diff(vec![h]);
    let hl = highlight_in_context("a.rs", &d, None, Some(&old)).unwrap();
    assert!(!hl.header(0).is_empty());
    d.highlight = Some(hl.clone());
    let again = highlight_in_context("a.rs", &d, Some(&d), Some(&old)).unwrap();
    assert_eq!(again.headers, hl.headers);
    assert_eq!(again, hl);
}
