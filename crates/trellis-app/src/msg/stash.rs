//! Messages for renaming a stash and the Stash dialog (update/stash.rs), and the auto-stash
//! (update/auto_stash.rs).

use super::Inactive;
use git2::Oid;
use trellis_core::stash_ops::StashScope;

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
}

impl StashMsg {
    /// What this message still needs when its tab is no longer active (`Msg::Scoped`): nothing,
    /// as all are user intents and never come back scoped.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::RenameDialog { .. } | Self::Rename { .. } | Self::PushDialog { .. } | Self::Push { .. } => None,
        }
    }
}

/// Stash and carry on: the auto-stash prompt's answers, the put-back's result and the toasts'
/// Show stash (update/auto_stash.rs).
pub enum AutoStashMsg {
    /// The prompt's primary: stash, run the operation, put the changes back.
    Stash,
    /// The hard reset prompt's Discard and reset: today's reset, its stash kept for Undo.
    Discard,
    /// The put-back job ended (runner/stashes.rs): the stash `oid` of `workdir`.
    PutBack {
        workdir: std::path::PathBuf,
        oid: Oid,
        outcome: trellis_core::stash_ops::PutBack,
        output: String,
        /// Boxed: the other variants are small.
        undo: Option<Box<crate::effect::UndoResult>>,
    },
    /// Open STASHES and select the stash with this commit.
    Show(Oid),
    /// A failure toast's Stash and <op>: run the operation again with the auto-stash approved.
    Retry(Box<crate::state::auto_stash::StashRetry>),
}

impl AutoStashMsg {
    /// Nothing for an inactive tab: the answers and Show are intents, and PutBack comes back
    /// unscoped like RunGit's JobDone.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Stash | Self::Discard | Self::PutBack { .. } | Self::Show(_) | Self::Retry(_) => None,
        }
    }
}
