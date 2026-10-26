//! The operation journal: every state HEAD and the local branches have been in, newest first,
//! from git's reflogs (`reflog.rs`) plus Glowfly's own records written before an operation that
//! rewrites history (`notes.rs`), merged into rows by `timeline.rs`. `events.rs` reads what a
//! reflog message says happened, `preview.rs` what restoring a state would move, and
//! `restore.rs` holds Restore's command lines. `reach.rs` marks the rows whose commit nothing
//! keeps any more, and `lost.rs` finds the commits no ref keeps at all (`git fsck`). Reads use
//! libgit2 (fsck runs through gitcli); notes are written only by the app's runner.

use crate::commit_text;
use crate::error::Result;
use crate::undo::HeadPos;
use git2::{Oid, Repository};
use std::collections::HashMap;
use std::path::Path;

pub mod events;
pub mod lost;
pub mod notes;
pub mod preview;
mod reach;
mod reflog;
pub mod restore;
mod timeline;

pub use events::Event;
pub use lost::{LostCommit, LostCommits, LostKind};
pub use notes::{Note, NotedRef, Op};
pub use preview::{CommitLine, Count, Preview, preview};

/// The newest entries read from each reflog.
pub const MAX_PER_REF: usize = 1000;
/// The most rows a timeline holds.
pub const MAX_ROWS: usize = 2000;

/// Whose state a row is: HEAD's own (a checkout, a detached HEAD), or a local branch's.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Subject {
    Head,
    Branch(String),
}

/// One state in the journal.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub subject: Subject,
    pub oid: Oid,
    /// The commit before (`None`: created, or a note's own row).
    pub prev: Option<Oid>,
    /// Unix seconds.
    pub at: i64,
    pub event: Event,
    /// Git's reflog message (empty for a note's own row).
    pub message: String,
    pub note: Option<Note>,
    /// The commit's summary; `None` when the commit is no longer in the repository.
    pub summary: Option<String>,
    /// The newest row of its subject, and the subject still points at `oid`.
    pub current: bool,
    /// No ref, worktree HEAD or stash entry reaches `oid`; false when unknown or gone.
    pub unreachable: bool,
}

/// What identifies a row across reloads.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RowKey {
    pub subject: Subject,
    pub oid: Oid,
    pub at: i64,
}

impl Row {
    pub fn key(&self) -> RowKey {
        RowKey { subject: self.subject.clone(), oid: self.oid, at: self.at }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Timeline {
    pub rows: Vec<Row>,
    /// Cut at `MAX_PER_REF` per reflog or `MAX_ROWS` overall.
    pub capped: bool,
    pub head: Option<HeadPos>,
    /// Each local branch and its tip.
    pub tips: Vec<(String, Oid)>,
}

/// The journal of the repository at `workdir`.
pub fn load(workdir: &Path) -> Result<Timeline> {
    let repo = Repository::open(workdir)?;
    let read = reflog::read(&repo)?;
    let notes = notes::read(workdir);
    let (mut rows, capped) = timeline::build(read.entries, notes, read.head.1, &read.tips);
    let mut summaries: HashMap<Oid, Option<String>> = HashMap::new();
    for row in &mut rows {
        let summary =
            summaries.entry(row.oid).or_insert_with(|| repo.find_commit(row.oid).ok().map(|c| commit_text::summary(&c).into_owned()));
        row.summary = summary.clone();
    }
    let marks = reach::unreachable(&repo, rows.iter().filter(|r| r.summary.is_some()).map(|r| r.oid));
    for row in &mut rows {
        row.unreachable = marks.contains(&row.oid);
    }
    Ok(Timeline { rows, capped: capped || read.capped, head: read.head.0, tips: read.tips })
}

#[cfg(test)]
mod events_tests;
#[cfg(test)]
mod lost_tests;
#[cfg(test)]
mod notes_tests;
#[cfg(test)]
mod preview_tests;
#[cfg(test)]
mod reach_tests;
#[cfg(test)]
mod reflog_tests;
#[cfg(test)]
mod timeline_tests;
