//! Messages for repository groups (update/repo_groups.rs): making, renaming, colouring,
//! ordering and removing groups, moving favorites into, within and out of them, and adding
//! repositories to a group (its Add repositories picker, a Recent row's Add to group), and
//! resetting every group's colour to the default (and its Undo).

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
    /// Open the Add repositories picker for this group (its placeholder and ⋯).
    AddDialog(usize),
    /// Put the repositories at `paths` at the end of `group`, in that order (the picker's Add,
    /// a Recent row's Add to group): one that isn't a favorite becomes one, one in another group
    /// moves, and one already in the group stays where it is. One settings save.
    AddRepos { group: usize, paths: Vec<PathBuf> },
    /// Give every group the colour the New group color setting would give it, made again top
    /// to bottom (a group menu's Reset all group colors). Toasts with Undo.
    ResetColours,
    /// Put back the colours named groups had (the reset toast's Undo): by name, case ignored; a
    /// group no longer called that is skipped.
    RestoreColours(Vec<(String, Option<RepoColour>)>),
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
            | Self::MoveRepo { .. }
            | Self::AddDialog(_)
            | Self::AddRepos { .. }
            | Self::ResetColours
            | Self::RestoreColours(_) => None,
        }
    }
}
