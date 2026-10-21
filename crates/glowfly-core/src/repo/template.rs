//! The repository's commit message template (`commit.template`), used to prefill an empty
//! commit draft. Read with libgit2: it needs no hooks or credentials.

use git2::{Config, Repository};
use std::path::Path;

/// Templates larger than this are ignored (they are meant to be a few lines of guidance).
pub const MAX_TEMPLATE_BYTES: u64 = 64 * 1024;

/// A commit template split the way the commit box shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct CommitTemplate {
    pub summary: String,
    pub description: String,
    /// The template's lines starting with this are left out of the message (`core.commentChar`).
    pub comment_char: char,
}

impl CommitTemplate {
    /// Splits template text into a summary and a description. None when `text` is blank.
    ///
    /// A first line of plain text becomes the summary, and what follows it (without leading
    /// blank lines) the description. A template that starts with a comment or a blank line
    /// has no summary: all of it (without leading blank lines) is the description.
    pub fn parse(text: &str, comment_char: char) -> Option<Self> {
        let text = text.replace("\r\n", "\n");
        let text = text.trim_end();
        if text.trim().is_empty() {
            return None;
        }
        let drop_blank = |s: &str| s.lines().skip_while(|l| l.trim().is_empty()).collect::<Vec<_>>().join("\n").trim_end().to_owned();
        let (first, rest) = text.split_once('\n').unwrap_or((text, ""));
        let (summary, description) = if first.trim().is_empty() || first.starts_with(comment_char) {
            (String::new(), drop_blank(text))
        } else {
            (first.trim_end().to_owned(), drop_blank(rest))
        };
        Some(Self { summary, description, comment_char })
    }
}

/// The repository's `commit.template`, read with the configured comment char. None when it is
/// unset, missing, unreadable or larger than `MAX_TEMPLATE_BYTES`, as `git commit` ignores it
/// then too. A relative path is resolved against `workdir`, where Glowfly runs git.
pub fn commit_template(repo: &Repository, workdir: &Path) -> Option<CommitTemplate> {
    let cfg = repo.config().ok()?.snapshot().ok()?;
    // get_path expands a leading ~/.
    let path = cfg.get_path("commit.template").ok()?;
    let path = if path.is_relative() { workdir.join(path) } else { path };
    let meta = std::fs::metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > MAX_TEMPLATE_BYTES {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    CommitTemplate::parse(&String::from_utf8_lossy(&bytes), comment_char(&cfg))
}

/// `core.commentChar` (or `core.commentString`) when it is a single character; '#' otherwise,
/// including "auto".
pub(super) fn comment_char(cfg: &Config) -> char {
    ["core.commentChar", "core.commentString"]
        .iter()
        .find_map(|key| {
            let v = cfg.get_string(key).ok()?;
            let mut chars = v.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Some(c),
                _ => None,
            }
        })
        .unwrap_or('#')
}
