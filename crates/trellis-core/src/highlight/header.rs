//! The function context in hunk headers: the text git adds after the closing `@@` (the nearest
//! line above the hunk that looks like a definition, e.g. `fn greet(name: &str) {`). It is
//! parsed on its own from a fresh state, never in the context of the hunk or the old text, so a
//! hunk inside a block comment still shows its function line as code.
//!
//! Headers don't count toward `MAX_LINES` or `MAX_BYTES`: there is at most one short line per
//! hunk (git and libgit2 cap the function context at about 80 bytes).

use super::parse::{Classifier, Side, Stop};
use super::{MAX_LINE_BYTES, Span};
use crate::repo::Hunk;
use syntect::parsing::SyntaxReference;

/// The byte offset of the function context in a hunk header (`@@ -a,b +c,d @@ <context>`), or
/// `None` when it has none. Combined headers (`@@@ ... @@@`) work too. Pure.
pub fn context_start(header: &str) -> Option<usize> {
    let n = header.bytes().take_while(|&b| b == b'@').count();
    if n < 2 {
        return None;
    }
    let marker = &header[..n];
    let close = n + header[n..].find(marker)?;
    let mut at = close + n;
    if header[at..].starts_with(' ') {
        at += 1;
    }
    let rest = &header[at..];
    (!rest.trim().is_empty()).then_some(at)
}

/// Spans for each hunk's header context, relative to `context_start` (so a reload can carry
/// them by the context's text when the line numbers change). Empty for a header without context,
/// one longer than `MAX_LINE_BYTES`, or one that fails to parse.
pub(super) fn parse_headers(
    syntax: &SyntaxReference,
    hunks: &[Hunk],
    cx: &mut Classifier,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<Span>>, Stop> {
    let mut out = Vec::with_capacity(hunks.len());
    for hunk in hunks {
        let context = context_start(&hunk.header).map(|at| &hunk.header[at..]).filter(|c| c.len() <= MAX_LINE_BYTES);
        let spans = match context {
            None => Vec::new(),
            Some(text) => match Side::new(syntax).line(text, cx, cancelled) {
                Ok(spans) => spans,
                Err(Stop::Parse) => Vec::new(),
                Err(Stop::Cancelled) => return Err(Stop::Cancelled),
            },
        };
        out.push(spans);
    }
    Ok(out)
}
