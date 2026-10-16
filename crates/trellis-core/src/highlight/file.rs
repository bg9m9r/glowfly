//! Highlighting a whole file (the blame view's lines): the file as one hunk of unchanged lines,
//! parsed from its first line. It runs on a worker thread, so the background limits apply;
//! beyond them the file stays plain.

use super::{Span, finish, highlight_diff};
use crate::highlight::SyntaxHint;
use crate::repo::{DiffLine, FileDiff, Hunk};

/// Token spans for each of `lines` (raw, tabs kept), or `None` to show them plain: no lines, an
/// unknown language, or too much to parse.
pub fn highlight_file(hint: SyntaxHint<'_>, lines: &[String]) -> Option<Vec<Vec<Span>>> {
    if lines.is_empty() {
        return None;
    }
    let line = |(i, text): (usize, &String)| {
        let n = u32::try_from(i + 1).ok();
        DiffLine { origin: ' ', old: n, new: n, text: text.clone() }
    };
    let hunk = Hunk { header: String::new(), lines: lines.iter().enumerate().map(line).collect() };
    let mut diff = FileDiff { binary: false, hunks: vec![hunk], highlight: None, intraline: Default::default(), whole: None };
    let mut hl = highlight_diff(hint, &diff, None)?;
    if hl.is_pending() {
        diff.highlight = Some(hl);
        hl = finish(&diff, &|| false)?;
    }
    hl.lines.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highlight::TokenKind;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_owned).collect()
    }

    fn kind_at(spans: &[Span], byte: u32) -> Option<TokenKind> {
        spans.iter().find(|s| s.start <= byte && byte < s.end).map(|s| s.kind)
    }

    #[test]
    fn a_rust_file_gets_one_span_list_per_line() {
        let src = lines("// hello\nfn main() {\n    let s = \"x\";\n}\n");
        let spans = highlight_file("src/main.rs".into(), &src).expect("highlighted");
        assert_eq!(spans.len(), src.len());
        assert_eq!(kind_at(&spans[1], 0), Some(TokenKind::Keyword), "{:?}", spans[1]);
        assert_eq!(kind_at(&spans[0], 3), Some(TokenKind::Comment));
    }

    #[test]
    fn a_shebang_script_is_detected_without_an_extension() {
        let src = lines("#!/bin/sh\nif true; then echo hi; fi\n");
        let spans = highlight_file("tools/run".into(), &src).expect("detected from line 1");
        assert!(spans[1].iter().any(|s| s.kind == TokenKind::Keyword), "{:?}", spans[1]);
    }

    #[test]
    fn unknown_or_empty_files_stay_plain() {
        assert!(highlight_file("notes.zzunknown".into(), &lines("just words\n")).is_none());
        assert!(highlight_file("src/main.rs".into(), &[]).is_none());
    }

    #[test]
    fn a_large_file_is_finished_in_the_background_pass() {
        let src: Vec<String> = (0..super::super::MAX_LINES + 10).map(|i| format!("fn f{i}() {{}}")).collect();
        let spans = highlight_file("big.rs".into(), &src).expect("background pass");
        assert_eq!(spans.len(), src.len());
        assert_eq!(kind_at(&spans[src.len() - 1], 0), Some(TokenKind::Keyword));
    }
}
