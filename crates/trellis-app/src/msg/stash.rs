//! Messages for renaming a stash (update/stash.rs).

use super::Inactive;
use git2::Oid;

/// Renaming a stash from the STASHES row menu or the palette (update/stash.rs).
pub enum StashMsg {
    /// Open the Rename stash dialog for the stash with this commit, found again in the snapshot.
    RenameDialog { oid: Oid },
    /// Give the stash with this commit this message: the dialog's text, from which `update`
    /// builds the full stash list entry (`stash_ops::edited_message`).
    Rename { oid: Oid, message: String },
}

impl StashMsg {
    /// What this message still needs when its tab is no longer active (`Msg::Scoped`): nothing,
    /// as both are user intents and never come back scoped.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::RenameDialog { .. } | Self::Rename { .. } => None,
        }
    }
}
