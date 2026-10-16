//! Messages for renaming a stash and the Stash dialog (update/stash.rs).

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
    /// against the snapshot.
    Push { scope: StashScope, message: String, paths: Vec<String> },
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
