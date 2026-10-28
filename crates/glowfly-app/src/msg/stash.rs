//! Messages for renaming a stash, the question before applying one and the Stash dialog
//! (update/stash.rs), and the auto-stash
//! (update/auto_stash.rs), with the records a restart left waiting (update/auto_stash_saved.rs).

use super::Inactive;
use crate::state::stash_confirm::StashAsk;
use git2::Oid;
use glowfly_core::stash_ops::{SavedAutoStash, StashScope};
use std::path::PathBuf;

/// Renaming a stash and the Stash dialog (update/stash.rs, update/stash_push.rs).
pub enum StashMsg {
    /// Open the Rename stash dialog for the stash with this commit, found again in the snapshot.
    RenameDialog { oid: Oid },
    /// Give the stash with this commit this message: the dialog's text, from which `update`
    /// builds the full stash list entry (`stash_ops::edited_message`).
    Rename { oid: Oid, message: String },
    /// Open the Stash dialog on this choice; with `path`, Selected files with that file ticked.
    PushDialog { scope: StashScope, path: Option<String> },
    /// Stash with the dialog's choice and text; `paths` only for Files. `update` re-checks them
    /// against the snapshot. `keep_index`: --keep-index, ignored for Staged.
    Push { scope: StashScope, message: String, paths: Vec<String>, keep_index: bool },
    /// Apply or pop on top of uncommitted changes (state/stash_confirm.rs): read the stash's
    /// files, then ask.
    Ask(StashAsk),
    /// The stash's files for `ask` (runner/stashes.rs), or why they couldn't be read: open the
    /// question.
    PathsRead { ask: StashAsk, paths: Result<Vec<String>, String> },
}

impl StashMsg {
    /// What this message still needs when its tab is no longer active (`Msg::Scoped`): nothing.
    /// The intents never come back scoped, and a question for a tab the user left is dropped.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::RenameDialog { .. }
            | Self::Rename { .. }
            | Self::PushDialog { .. }
            | Self::Push { .. }
            | Self::Ask(_)
            | Self::PathsRead { .. } => None,
        }
    }
}

/// Stash and carry on: the auto-stash prompt's answers, the put-back's result, the toasts'
/// Show stash (update/auto_stash.rs), and the saved records a restart left waiting with their
/// offer's answers (update/auto_stash_saved.rs).
pub enum AutoStashMsg {
    /// The prompt's primary: stash, run the operation, put the changes back.
    Stash,
    /// The hard reset prompt's Discard and reset: today's reset, its stash kept for Undo.
    Discard,
    /// The put-back job ended (runner/stashes.rs): the stash `oid` of `workdir`.
    PutBack {
        workdir: PathBuf,
        oid: Oid,
        outcome: glowfly_core::stash_ops::PutBack,
        output: String,
        /// Boxed: the other variants are small.
        undo: Option<Box<crate::effect::UndoResult>>,
    },
    /// Open STASHES and select the stash with this commit.
    Show(Oid),
    /// A failure toast's Stash and <op>: run the operation again with the auto-stash approved.
    Retry(Box<crate::state::auto_stash::StashRetry>),
    /// The saved records of `workdir` (runner/stashes.rs, only when there are any). Unscoped
    /// like PutBack: they are kept for `workdir` whichever tab is active.
    SavedRead { workdir: PathBuf, saved: Vec<SavedAutoStash> },
    /// The saved stash offer's Put back: the stash `oid` of `workdir` goes back now.
    PutBackSaved { workdir: PathBuf, oid: Oid },
    /// The saved stash offer's Leave in stash list: forget the record, keep the stash.
    LeaveSaved { workdir: PathBuf, oid: Oid },
}

impl AutoStashMsg {
    /// Nothing for an inactive tab: the answers and Show are intents, and PutBack and SavedRead
    /// come back unscoped like RunGit's JobDone.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Stash
            | Self::Discard
            | Self::PutBack { .. }
            | Self::Show(_)
            | Self::Retry(_)
            | Self::SavedRead { .. }
            | Self::PutBackSaved { .. }
            | Self::LeaveSaved { .. } => None,
        }
    }
}
