//! The blame view's request (`Effect::LoadBlame`).

use git2::Oid;
use std::path::PathBuf;
use trellis_core::repo::BlameOptions;

/// Blame `path` of `workdir` at HEAD (`at: None`) or as of commit `at`.
#[derive(Debug)]
pub struct BlameRequest {
    pub workdir: PathBuf,
    pub path: String,
    pub at: Option<Oid>,
    /// Whitespace, moves and skipped commits (the header's toggles and skipped chip).
    pub opts: BlameOptions,
    /// Highlight the lines too (Preferences > Editor syntax highlighting).
    pub highlight: bool,
    pub generation: u64,
}
