//! Messages and effects of the operation journal: the timeline of HEAD's and the branches'
//! states with Restore (state/journal.rs, state/journal_restore.rs, update/journal.rs,
//! runner/journal.rs, the view in ui/journal/), and its Lost commits tab
//! (state/journal_lost.rs, update/journal_lost.rs, ui/journal/lost*.rs).

use crate::state::journal_lost::JournalTab;
use git2::Oid;
use std::path::PathBuf;
use trellis_core::journal::{LostCommits, Preview, RowKey, Timeline};

#[derive(Clone, Debug, PartialEq)]
pub enum JournalMsg {
    /// Open the journal in the central area (palette, View menu).
    Open,
    /// Back to the graph (Close, Esc).
    Close,
    /// The View menu's check item.
    Toggle,
    /// Read the journal again (Refresh, Try again).
    Refresh,
    /// The journal was read for request `generation`.
    Loaded { generation: u64, result: Result<Box<Timeline>, String> },
    /// A row was clicked.
    Select(RowKey),
    /// The preview of row `key` was read for request `generation`.
    PreviewLoaded { generation: u64, key: RowKey, result: Result<Preview, String> },
    /// Restore… on a row: the confirmation, or why it can't.
    Restore(RowKey),
    /// Create branch here… on a row.
    CreateBranch(RowKey),
    /// The header's States | Lost commits control.
    Tab(JournalTab),
    /// The Lost commits tab.
    Lost(LostMsg),
}

/// The journal's Lost commits tab.
#[derive(Clone, Debug, PartialEq)]
pub enum LostMsg {
    /// The lost commits were found for request `generation`.
    Loaded { generation: u64, result: Result<Box<LostCommits>, String> },
    /// A lost commit was clicked.
    Select(Oid),
    /// The history of lost commit `oid` was read for request `generation`.
    PreviewLoaded { generation: u64, oid: Oid, result: Result<Preview, String> },
    /// Restore… on a lost commit: check it out detached, after a confirmation.
    Restore(Oid),
    /// Create branch here… on a lost commit.
    CreateBranch(Oid),
    /// Apply stash on a dropped stash.
    Apply(Oid),
    /// Put back in stash list on a dropped stash.
    Store(Oid),
}

impl LostMsg {
    /// Nothing is kept for an inactive tab: a load or preview in flight is asked for again when
    /// the tab returns.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Loaded { .. }
            | Self::Select(_)
            | Self::PreviewLoaded { .. }
            | Self::Restore(_)
            | Self::CreateBranch(_)
            | Self::Apply(_)
            | Self::Store(_) => None,
        }
    }
}

impl JournalMsg {
    /// Nothing is kept for an inactive tab: the intents only make sense in the active one, and
    /// a load or preview that was in flight is asked for again when the tab returns.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Open
            | Self::Close
            | Self::Toggle
            | Self::Refresh
            | Self::Loaded { .. }
            | Self::Select(_)
            | Self::PreviewLoaded { .. }
            | Self::Restore(_)
            | Self::CreateBranch(_)
            | Self::Tab(_) => None,
            Self::Lost(m) => m.on_inactive(),
        }
    }
}

/// The journal's reads (runner/journal.rs); scoped to the repository they read.
#[derive(Debug)]
pub enum JournalEffect {
    Load {
        workdir: PathBuf,
        generation: u64,
    },
    /// The preview of row `key`'s commit `oid`, counted against `against` (its subject's tip now).
    Preview {
        workdir: PathBuf,
        key: RowKey,
        oid: Oid,
        against: Option<Oid>,
        generation: u64,
    },
    /// The lost commits (`git fsck`).
    Lost {
        workdir: PathBuf,
        generation: u64,
    },
    /// The history of lost commit `oid`, read from `from` (a dropped stash's base).
    LostPreview {
        workdir: PathBuf,
        oid: Oid,
        from: Oid,
        generation: u64,
    },
}

impl JournalEffect {
    pub fn scope(&self) -> &PathBuf {
        match self {
            Self::Load { workdir, .. } | Self::Preview { workdir, .. } | Self::Lost { workdir, .. } | Self::LostPreview { workdir, .. } => {
                workdir
            }
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Load { .. } => "journal-load",
            Self::Preview { .. } => "journal-preview",
            Self::Lost { .. } => "journal-lost",
            Self::LostPreview { .. } => "journal-lost-preview",
        }
    }
}
