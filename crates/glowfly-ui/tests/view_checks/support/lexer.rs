//! A small Rust lexer for the source checks: it splits a file into code, comments and string or
//! char literals, so a `//` inside `"https://…"` is not a comment and a `{` inside a string is not
//! a brace. Shared by glowfly-ui's view checks, the app crate's `source_checks` binary
//! (`hygiene`, `layering`, `i18n_keys`, `dead_pub`) and its `glyph_check`, which include this file.
//!
//! The usual entry points: `code_lines` (comments and inline test modules blanked, literals kept),
//! `bare_lines` (literal text blanked too), `literals` (each string or char literal with its line)
//! and `is_test_file`.

// Each check uses only some of these.
#![allow(dead_code)]

use std::ops::Range;

/// What a piece of the source is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Code,
    /// A `//` or (nested) `/* */` comment, doc comments too.
    Comment,
    /// A string literal: `"…"`, `r#"…"#`, `b"…"`, `br"…"`, `c"…"`.
    Str,
    /// A char literal: `'c'`, `'\n'`, `b'x'`.
    Char,
}

/// One run of the source: its kind and byte range; for a literal, also the range of its text
/// between the quotes (escapes not decoded), and whether it is raw or a byte literal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Piece {
    pub kind: Kind,
    pub span: Range<usize>,
    pub inner: Range<usize>,
    pub raw: bool,
    pub byte: bool,
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80
}

/// The source cut into pieces, in order and covering all of it.
pub fn pieces(src: &str) -> Vec<Piece> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let (mut i, mut code_start) = (0, 0);
    while i < b.len() {
        let after_ident = i > 0 && is_ident(b[i - 1]);
        let at = |k: usize| b.get(i + k).copied();
        let found = match b[i] {
            b'/' if at(1) == Some(b'/') => {
                let end = b[i..].iter().position(|&c| c == b'\n').map_or(b.len(), |k| i + k);
                Some(comment(i, end))
            }
            b'/' if at(1) == Some(b'*') => Some(comment(i, block_comment_end(b, i))),
            b'r' if !after_ident => raw_string(b, i, 1, false),
            b'b' | b'c' if !after_ident && at(1) == Some(b'r') => raw_string(b, i, 2, b[i] == b'b'),
            b'b' | b'c' if !after_ident && at(1) == Some(b'"') => Some(string(b, i, 2, b[i] == b'b')),
            b'b' if !after_ident && at(1) == Some(b'\'') => char_literal(src, i, 2, true),
            b'"' => Some(string(b, i, 1, false)),
            b'\'' => char_literal(src, i, 1, false),
            _ => None,
        };
        match found {
            Some(piece) => {
                if code_start < i {
                    out.push(code(code_start, i));
                }
                i = piece.span.end;
                code_start = i;
                out.push(piece);
            }
            None => i += 1,
        }
    }
    if code_start < b.len() {
        out.push(code(code_start, b.len()));
    }
    out
}

fn code(start: usize, end: usize) -> Piece {
    Piece { kind: Kind::Code, span: start..end, inner: start..end, raw: false, byte: false }
}

fn comment(start: usize, end: usize) -> Piece {
    Piece { kind: Kind::Comment, span: start..end, inner: start..end, raw: false, byte: false }
}

fn block_comment_end(b: &[u8], start: usize) -> usize {
    let (mut i, mut depth) = (start, 0);
    while i + 1 < b.len() {
        match (b[i], b[i + 1]) {
            (b'/', b'*') => {
                depth += 1;
                i += 2;
            }
            (b'*', b'/') => {
                depth -= 1;
                i += 2;
                if depth == 0 {
                    return i;
                }
            }
            _ => i += 1,
        }
    }
    b.len()
}

/// A raw string whose `#`s or quote start `prefix` bytes after `start`; none when this is not
/// one (the raw identifier `r#type`, or a plain `r`).
fn raw_string(b: &[u8], start: usize, prefix: usize, byte: bool) -> Option<Piece> {
    let open = start + prefix;
    let hashes = b[open..].iter().take_while(|&&c| c == b'#').count();
    if b.get(open + hashes) != Some(&b'"') {
        return None;
    }
    let text = open + hashes + 1;
    let mut i = text;
    while i < b.len() {
        if b[i] == b'"' && b[i + 1..].iter().take(hashes).filter(|&&c| c == b'#').count() == hashes {
            return Some(Piece { kind: Kind::Str, span: start..i + 1 + hashes, inner: text..i, raw: true, byte });
        }
        i += 1;
    }
    Some(Piece { kind: Kind::Str, span: start..b.len(), inner: text..b.len(), raw: true, byte })
}

/// A string whose text starts `prefix` bytes after `start` (after its opening quote).
fn string(b: &[u8], start: usize, prefix: usize, byte: bool) -> Piece {
    let text = start + prefix;
    let mut i = text;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'"' => return Piece { kind: Kind::Str, span: start..i + 1, inner: text..i, raw: false, byte },
            _ => i += 1,
        }
    }
    Piece { kind: Kind::Str, span: start..b.len(), inner: text..b.len().min(i), raw: false, byte }
}

/// A char literal whose text starts `prefix` bytes after `start`; none for the quote of a
/// lifetime or label (`'a`, `'outer:`).
fn char_literal(src: &str, start: usize, prefix: usize, byte: bool) -> Option<Piece> {
    let b = src.as_bytes();
    let text = start + prefix;
    let close = if b.get(text) == Some(&b'\\') {
        // `'\''`, `'\\'`, `'\n'`, `'\x41'`, `'\u{1F600}'`: the quote that ends it comes after the
        // escaped character.
        (text + 2..(text + 12).min(b.len())).find(|&k| b[k] == b'\'')?
    } else {
        let c = src.get(text..)?.chars().next()?;
        let k = text + c.len_utf8();
        if c == '\n' || c == '\'' || b.get(k) != Some(&b'\'') {
            return None;
        }
        k
    };
    Some(Piece { kind: Kind::Char, span: start..close + 1, inner: text..close, raw: false, byte })
}

/// `src` with every comment, and (unless `keep_literals`) the text of every literal, replaced by
/// spaces, one per character; line breaks stay, so line numbers still match the file.
pub fn blanked(src: &str, keep_literals: bool) -> String {
    let mut out = String::with_capacity(src.len());
    let blank = |out: &mut String, text: &str| out.extend(text.chars().map(|c| if c == '\n' || c == '\r' { c } else { ' ' }));
    for p in pieces(src) {
        match p.kind {
            Kind::Code => out.push_str(&src[p.span]),
            Kind::Comment => blank(&mut out, &src[p.span]),
            Kind::Str | Kind::Char if keep_literals => out.push_str(&src[p.span]),
            Kind::Str | Kind::Char => {
                out.push_str(&src[p.span.start..p.inner.start]);
                blank(&mut out, &src[p.inner.clone()]);
                out.push_str(&src[p.inner.end..p.span.end]);
            }
        }
    }
    out
}

/// The source with comments and inline `#[cfg(test)] mod … { … }` blocks blanked out, line for
/// line, so line numbers still match the file. Literals are kept.
pub fn code_lines(src: &str) -> Vec<String> {
    without_test_modules(&blanked(src, true), &blanked(src, false))
}

/// As `code_lines`, with the text of every literal blanked too (its quotes stay).
pub fn bare_lines(src: &str) -> Vec<String> {
    let bare = blanked(src, false);
    without_test_modules(&bare, &bare)
}

/// The lines of `text`, with inline `#[cfg(test)] mod … { … }` blocks emptied; braces are
/// counted in `bare` (the same text with literals blanked), so a brace in a string doesn't count.
fn without_test_modules(text: &str, bare: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let bare: Vec<&str> = bare.lines().collect();
    let mut out = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        let next = bare.get(i + 1).map(|l| l.trim()).unwrap_or("");
        if bare[i].trim() == "#[cfg(test)]" && next.starts_with("mod ") && next.ends_with('{') {
            out.push(String::new());
            i += 1;
            let mut depth = 0i64;
            while i < lines.len() {
                depth += bare[i].matches('{').count() as i64 - bare[i].matches('}').count() as i64;
                out.push(String::new());
                i += 1;
                if depth <= 0 {
                    break;
                }
            }
            continue;
        }
        out.push(lines[i].to_owned());
        i += 1;
    }
    out
}

/// A string or char literal outside comments: the 1-based line it starts on, its text between
/// the quotes (escapes not decoded) and its piece.
pub struct Literal<'a> {
    pub line: usize,
    pub text: &'a str,
    pub piece: Piece,
}

/// Every string and char literal of `src`, in order, comments skipped (test code is not).
pub fn literals(src: &str) -> Vec<Literal<'_>> {
    let mut line = 1;
    let mut at = 0;
    let mut out = Vec::new();
    for p in pieces(src).into_iter().filter(|p| matches!(p.kind, Kind::Str | Kind::Char)) {
        line += src[at..p.span.start].matches('\n').count();
        at = p.span.start;
        out.push(Literal { line, text: &src[p.inner.clone()], piece: p });
    }
    out
}

/// Whether a source path (`/`-separated) is test code by its name: a component (without `.rs`)
/// that is `tests`, ends with `_tests` or starts with `snapshot_tests`. A name that merely starts
/// with `tests_` is not.
pub fn is_test_file(rel: &str) -> bool {
    rel.split('/')
        .map(|c| c.strip_suffix(".rs").unwrap_or(c))
        .any(|c| c == "tests" || c.ends_with("_tests") || c.starts_with("snapshot_tests"))
}

#[test]
fn comments_end_where_the_language_says() {
    let src = "let u = \"https://x\"; std::fs::read(p); // gone\n/* a /* nested */ \"no\" */ b(); let c = '\"'; d();";
    let lines = code_lines(src);
    assert_eq!(lines[0].trim_end(), "let u = \"https://x\"; std::fs::read(p);");
    assert_eq!(lines[1].trim(), "b(); let c = '\"'; d();");
    assert_eq!(bare_lines(src)[0].trim_end(), "let u = \"         \"; std::fs::read(p);");
}

#[test]
fn literals_of_every_shape() {
    let src = "f(r#\"a \"b\" c\"#, b\"x\\\"y\", br\"z\", 'q', b'\\'', '\\u{e9}');\nfn g<'a>(x: &'a str) { 'outer: loop { break 'outer; } }\n\
               let r#type = \"t\";\n\"two\nlines\" \"after\"";
    let found: Vec<(usize, &str, Kind)> = literals(src).iter().map(|l| (l.line, l.text, l.piece.kind)).collect();
    assert_eq!(
        found,
        [
            (1, "a \"b\" c", Kind::Str),
            (1, "x\\\"y", Kind::Str),
            (1, "z", Kind::Str),
            (1, "q", Kind::Char),
            (1, "\\'", Kind::Char),
            (1, "\\u{e9}", Kind::Char),
            (3, "t", Kind::Str),
            (4, "two\nlines", Kind::Str),
            (5, "after", Kind::Str),
        ]
    );
}

#[test]
fn test_modules_are_blanked_by_their_real_braces() {
    let src = "fn a() {}\n#[cfg(test)]\nmod tests {\n    const S: &str = \"}\";\n    fn b() {}\n}\nfn c() {}\n";
    let lines = code_lines(src);
    assert_eq!(lines.len(), 7);
    assert!(lines[1..6].iter().all(|l| l.is_empty()), "{lines:?}");
    assert_eq!(lines[6], "fn c() {}");
    assert!(is_test_file("update/tabs_tests.rs") && is_test_file("ui/snapshot_tests_clone.rs") && is_test_file("update/tests/x.rs"));
    assert!(!is_test_file("update/tabs.rs") && !is_test_file("tests_like.rs"));
}
