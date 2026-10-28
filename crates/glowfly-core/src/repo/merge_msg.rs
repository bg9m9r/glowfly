//! The message git prepared for a stopped merge, cherry-pick or revert (`<git dir>/MERGE_MSG`), cleaned the way
//! `git commit` cleans it, to prefill the commit draft. Read with libgit2 (`Repository::message`),
//! whose git dir is a linked worktree's own, so each worktree reads its own file.

use super::template::{MAX_TEMPLATE_BYTES, comment_char};
use git2::Repository;

/// MERGE_MSG without its comment lines and everything from a scissors line on. None when the
/// file is missing, larger than `MAX_TEMPLATE_BYTES` or blank once cleaned.
pub fn merge_message(repo: &Repository) -> Option<String> {
    let text = repo.message().ok()?;
    if text.len() as u64 > MAX_TEMPLATE_BYTES {
        return None;
    }
    let comment = repo.config().ok().and_then(|mut c| c.snapshot().ok()).map(|c| comment_char(&c)).unwrap_or('#');
    clean(&text, comment)
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
