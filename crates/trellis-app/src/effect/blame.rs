//! The blame view's request (`Effect::LoadBlame`).

use git2::Oid;
use std::path::PathBuf;

/// Blame `path` of `workdir` at HEAD (`at: None`) or as of commit `at`.
#[derive(Debug)]
pub struct BlameRequest {
    pub workdir: PathBuf,
    pub path: String,
    pub at: Option<Oid>,
    /// Highlight the lines too (Preferences > Editor syntax highlighting).
    pub highlight: bool,
    pub generation: u64,
}
