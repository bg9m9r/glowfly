//! Messages for repository groups (update/repo_groups.rs): making, renaming, colouring,
//! ordering and removing groups, and moving favorites into, within and out of them.

use super::Inactive;
use glowfly_core::settings::RepoColour;
use std::path::PathBuf;

/// Repository groups (update/repo_groups.rs). Groups are named by their index in
/// `Settings::repo_groups`, as the views drew them.
pub enum GroupMsg {
    /// Open the New group dialog; with `repo`, that repository moves into the new group.
    NewDialog { repo: Option<PathBuf> },
    /// Open the Rename group dialog for this group.
    RenameDialog(usize),
    /// Make a group called `name` (the dialog's Create), moving `repo` into it.
    Create { name: String, repo: Option<PathBuf> },
    /// Rename this group (the dialog's Rename).
    Rename { group: usize, name: String },
    /// Give the group a colour, or with None take it away (its Color submenu).
    SetColour { group: usize, colour: Option<RepoColour> },
    /// Move group `from` so it ends up at index `to` (Move up, Move down).
    MoveGroup { from: usize, to: usize },
    /// Remove the group; its repositories stay favorites, after the ungrouped ones.
    Remove(usize),
    /// Put the repository at `path` in `group` (None: no group) at `index` among that section's
    /// rows (dragging, Move to group, Move up and Move down). A repository that isn't a favorite
    /// becomes one.
    MoveRepo { path: PathBuf, group: Option<usize>, index: usize },
}

impl GroupMsg {
    /// What this message still needs when its tab is no longer active (`Msg::Scoped`): nothing,
    /// as none of these is ever sent scoped.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::NewDialog { .. }
            | Self::RenameDialog(_)
            | Self::Create { .. }
            | Self::Rename { .. }
            | Self::SetColour { .. }
            | Self::MoveGroup { .. }
            | Self::Remove(_)
            | Self::MoveRepo { .. } => None,
        }
    }
}
