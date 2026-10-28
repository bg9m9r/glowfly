//! Messages and effects of the operation journal: the timeline of HEAD's and the branches'
//! states with Restore (state/journal.rs, state/journal_restore.rs, update/journal.rs,
//! runner/journal.rs, the view in ui/journal/), and its Lost commits tab
//! (state/journal_lost.rs, update/journal_lost.rs, ui/journal/lost*.rs), which also lists the
//! discarded changes Glowfly kept and restores them.

use crate::effect::EffectMeta;
#[cfg(test)]
use crate::effect::SandboxRule;
use crate::state::journal_lost::JournalTab;
use git2::Oid;
use glowfly_core::journal::{LostCommits, Preview, RowKey, Timeline};
use glowfly_core::undo::kept::Restored;
use std::path::{Path, PathBuf};

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
    /// Restore (`copy` false) or Restore as a copy on the kept file `id`.
    RestoreFile { id: Oid, copy: bool },
    /// The kept file `id` (at `path`) was restored, or not.
    FileRestored { id: Oid, path: String, result: Result<Restored, String> },
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
            | Self::Store(_)
            | Self::RestoreFile { .. }
            // The file was written (or not) all the same, and the tab's list is read again
            // when it returns, showing whether it is back.
            | Self::FileRestored { .. } => None,
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

/// The journal's reads (runner/journal.rs), and writing a kept file back; scoped to the
/// repository.
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
    /// Writes the kept file `id` (at `path`) back, or as a copy next to it; never over anything.
    RestoreFile {
        workdir: PathBuf,
        id: Oid,
        path: String,
        copy: bool,
    },
}

impl JournalEffect {
    pub fn workdir(&self) -> &PathBuf {
        match self {
            Self::Load { workdir, .. }
            | Self::Preview { workdir, .. }
            | Self::Lost { workdir, .. }
            | Self::LostPreview { workdir, .. }
            | Self::RestoreFile { workdir, .. } => workdir,
        }
    }
}

impl EffectMeta for JournalEffect {
    fn name(&self) -> &'static str {
        match self {
            Self::Load { .. } => "journal-load",
            Self::Preview { .. } => "journal-preview",
            Self::Lost { .. } => "journal-lost",
            Self::LostPreview { .. } => "journal-lost-preview",
            Self::RestoreFile { .. } => "journal-restore-file",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        Some(self.workdir())
    }

    fn timed(&self) -> bool {
        true
    }

    fn queue(&self) -> Option<&Path> {
        None
    }

    #[cfg(test)]
    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}
