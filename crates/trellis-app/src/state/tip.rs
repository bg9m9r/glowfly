//! The rich tooltip's content, as plain data: a title, a weak subline (a shortcut or a state
//! line), a body sentence, an inset preview of commits, files or lines, and a footer. State
//! rules build it and tests read it; `ui/widgets/tooltip.rs` draws it.

use crate::keymap::BindingId;
use trellis_core::repo::Snapshot;

/// Rows a preview shows before "and N more".
pub const PREVIEW_ROWS: usize = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct Tip {
    /// The action's name.
    pub title: String,
    pub sub: Option<TipSub>,
    /// One sentence; on a disabled button, why it can't be used.
    pub body: Option<String>,
    pub preview: Option<TipPreview>,
    pub footer: Option<String>,
}

/// The line under the title.
#[derive(Clone, Debug, PartialEq)]
pub enum TipSub {
    /// The binding's first key, drawn as key caps (left out when it has none).
    Keys(BindingId),
    /// A short state line ("main is 2 ahead, 3 behind origin/main").
    State(String),
}

/// An inset list of what the action would touch.
#[derive(Clone, Debug, PartialEq)]
pub struct TipPreview {
    /// Sentence case, with the count ("Sends 2 commits").
    pub heading: String,
    pub rows: Vec<TipRow>,
    /// How many more there are than `rows` shows ("and N more").
    pub more: usize,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TipRow {
    /// A 7-character short SHA and the summary.
    Commit {
        sha: String,
        summary: String,
    },
    /// A file's status letter and path.
    File {
        status: char,
        path: String,
    },
    Line(String),
}

impl Tip {
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), sub: None, body: None, preview: None, footer: None }
    }

    pub fn keys(mut self, id: BindingId) -> Self {
        self.sub = Some(TipSub::Keys(id));
        self
    }

    pub fn state(mut self, line: impl Into<String>) -> Self {
        self.sub = Some(TipSub::State(line.into()));
        self
    }

    /// `state` when there is a line.
    pub fn state_opt(self, line: Option<String>) -> Self {
        match line {
            Some(l) => self.state(l),
            None => self,
        }
    }

    pub fn body(mut self, text: impl Into<String>) -> Self {
        self.body = Some(text.into());
        self
    }

    pub fn preview(mut self, p: Option<TipPreview>) -> Self {
        self.preview = p;
        self
    }

    pub fn footer(mut self, text: impl Into<String>) -> Self {
        self.footer = Some(text.into());
        self
    }
}

impl TipPreview {
    pub fn new(heading: impl Into<String>, rows: Vec<TipRow>) -> Self {
        Self { heading: heading.into(), rows, more: 0, note: None }
    }

    pub fn note(mut self, note: Option<String>) -> Self {
        self.note = note;
        self
    }
}

/// The first 7 characters of `oid`.
pub fn short_sha(oid: git2::Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// Commit rows for `oids` from the loaded history; one missing from it shows its short SHA as a
/// line.
pub fn commit_rows(snap: &Snapshot, oids: &[git2::Oid]) -> Vec<TipRow> {
    oids.iter()
        .map(|o| match snap.row_of.get(o) {
            Some(&row) => TipRow::Commit { sha: short_sha(*o), summary: snap.commits[row].summary.clone() },
            None => TipRow::Line(short_sha(*o)),
        })
        .collect()
}
