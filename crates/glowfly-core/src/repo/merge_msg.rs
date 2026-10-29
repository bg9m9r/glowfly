//! The message git prepared for a stopped merge, cherry-pick or revert (`<git dir>/MERGE_MSG`), cleaned the way
//! `git commit` cleans it, to prefill the commit draft. Read from libgit2's git dir
//! (`Repository::path`), a linked worktree's own, so each worktree reads its own file. git
//! writes a cherry-pick's or revert's message in `i18n.commitEncoding`, so it is decoded from
//! that (`commit_text`'s rules: valid UTF-8 under a single-byte label reads as UTF-8), never
//! through libgit2's `Repository::message`, which panics on bytes that aren't UTF-8.

use super::template::{MAX_TEMPLATE_BYTES, comment_char};
use crate::commit_encoding::MessageEncoding;
use git2::Repository;

/// MERGE_MSG without its comment lines and everything from a scissors line on, decoded from
/// `encoding`. None when the file is missing, larger than `MAX_TEMPLATE_BYTES` or blank once
/// cleaned.
pub fn merge_message(repo: &Repository, encoding: MessageEncoding) -> Option<String> {
    let file = repo.path().join("MERGE_MSG");
    if std::fs::metadata(&file).ok()?.len() > MAX_TEMPLATE_BYTES {
        return None;
    }
    let bytes = std::fs::read(&file).ok()?;
    let comment = repo.config().ok().and_then(|mut c| c.snapshot().ok()).map(|c| comment_char(&c)).unwrap_or('#');
    clean(&encoding.decode(&bytes), comment)
}

/// `text` with CRLF as LF, cut at a scissors line, without lines starting with `comment` and
/// without trailing whitespace; None when nothing is left.
pub(super) fn clean(text: &str, comment: char) -> Option<String> {
    let scissors = format!("{comment} ------------------------ >8");
    let text = text.replace("\r\n", "\n");
    let kept: Vec<&str> =
        text.lines().take_while(|l| !l.starts_with(&scissors)).filter(|l| !l.starts_with(comment)).map(str::trim_end).collect();
    let out = kept.join("\n").trim_end().to_owned();
    (!out.trim().is_empty()).then_some(out)
}
