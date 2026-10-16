//! The open blame view: the file and commit asked for, and the blame once it has loaded.

use git2::Oid;
use trellis_core::highlight::Span;
use trellis_core::repo::Blame;

/// A loaded blame and its syntax spans (one list per line; `None` to paint plain).
#[derive(Debug)]
pub struct BlameData {
    pub blame: Blame,
    pub spans: Option<Vec<Vec<Span>>>,
}

impl BlameData {
    /// The spans of line `line`; empty when plain or out of range.
    pub fn spans(&self, line: usize) -> &[Span] {
        self.spans.as_ref().and_then(|s| s.get(line)).map_or(&[], Vec::as_slice)
    }
}

#[derive(Debug)]
pub struct BlameView {
    /// Repo-relative path, as opened.
    pub path: String,
    /// The commit it was opened at; `None` for HEAD.
    pub at: Option<Oid>,
    /// A request is running.
    pub loading: bool,
    pub error: Option<String>,
    pub data: Option<BlameData>,
}

impl BlameView {
    pub fn loading(path: String, at: Option<Oid>) -> Self {
        Self { path, at, loading: true, error: None, data: None }
    }

    /// A view that shows `why` instead of a blame.
    pub fn failed(path: String, at: Option<Oid>, why: &str) -> Self {
        Self { path, at, loading: false, error: Some(why.to_owned()), data: None }
    }
}
