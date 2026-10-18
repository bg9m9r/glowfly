//! The open blame view: the file and commit asked for, the blame once it has loaded, the steps
//! back, the commits skipped, and where the view is (its top line, the cursor line, find and
//! go to line).

use super::blame_find::BlameFind;
use git2::Oid;
use trellis_core::highlight::Span;
use trellis_core::repo::Blame;

/// How many "Blame before this commit" steps Back remembers; the oldest goes first.
pub const BACK_LIMIT: usize = 50;

/// A loaded blame, its syntax spans (one list per line; `None` to paint plain) and how new
/// each commit is (`Blame::ages`).
#[derive(Debug)]
pub struct BlameData {
    pub blame: Blame,
    pub spans: Option<Vec<Vec<Span>>>,
    pub ages: Vec<f32>,
}

impl BlameData {
    pub fn new(blame: Blame, spans: Option<Vec<Vec<Span>>>) -> Self {
        let ages = blame.ages();
        Self { blame, spans, ages }
    }

    /// The spans of line `line`; empty when plain or out of range.
    pub fn spans(&self, line: usize) -> &[Span] {
        self.spans.as_ref().and_then(|s| s.get(line)).map_or(&[], Vec::as_slice)
    }

    /// How new commit `commit` is: 0 for the oldest, 1 for the newest.
    pub fn age(&self, commit: usize) -> f32 {
        self.ages.get(commit).copied().unwrap_or(1.0)
    }
}

/// A blame Back returns to.
#[derive(Clone, Debug, PartialEq)]
pub struct BlameStep {
    pub path: String,
    pub at: Option<Oid>,
    pub skipped: Vec<Oid>,
    pub top_line: usize,
    pub cursor: Option<usize>,
}

/// Where the view scrolls to (0-based line).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlameLanding {
    pub line: usize,
    /// Make it the cursor and centre it; otherwise make it the top line.
    pub center: bool,
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
    /// The blames "Blame before this commit" came from, newest last.
    pub back: Vec<BlameStep>,
    /// Commits blamed past (`--ignore-rev`).
    pub skipped: Vec<Oid>,
    /// Where to scroll once the pending load arrives (set by update).
    pub land: Option<BlameLanding>,
    /// Where to scroll on the next frame (taken by the view).
    pub scroll_to: Option<BlameLanding>,
    /// The line find, go to line, the run keys and stepping back went to.
    pub cursor: Option<usize>,
    /// The first line in view (written by the view each frame).
    pub top_line: usize,
    pub find: BlameFind,
    /// The Line field's text.
    pub goto: String,
    /// Put the keyboard in the find field on the next frame (taken by the view).
    pub focus_find: bool,
}

impl BlameView {
    pub fn loading(path: String, at: Option<Oid>) -> Self {
        Self {
            path,
            at,
            loading: true,
            error: None,
            data: None,
            back: Vec::new(),
            skipped: Vec::new(),
            land: None,
            scroll_to: None,
            cursor: None,
            top_line: 0,
            find: BlameFind::default(),
            goto: String::new(),
            focus_find: false,
        }
    }

    /// A view that shows `why` instead of a blame.
    pub fn failed(path: String, at: Option<Oid>, why: &str) -> Self {
        Self { loading: false, error: Some(why.to_owned()), ..Self::loading(path, at) }
    }

    /// The loaded blame's line count (0 while loading).
    pub fn line_count(&self) -> usize {
        self.data.as_ref().map_or(0, |d| d.blame.lines.len())
    }

    /// Goes to line `line`: makes it the cursor and centres it.
    pub fn land_on(&mut self, line: usize) {
        self.cursor = Some(line);
        self.scroll_to = Some(BlameLanding { line, center: true });
    }
}
