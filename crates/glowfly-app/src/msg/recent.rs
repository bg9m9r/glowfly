//! Messages for the recent and favorite repository lists (update/recent.rs).

use super::Inactive;
use glowfly_core::paths::FolderId;
use glowfly_core::settings::RepoColour;
use std::path::PathBuf;

/// The recent and favorite repository lists (update/recent.rs).
pub enum RecentMsg {
    /// Remove this folder from the Recent list (welcome-screen ×, row menu, repository menu ×).
    Remove(PathBuf),
    /// Empty the Recent list (after the confirmation); favorites stay.
    ClearRecent,
    /// Add the folder to favorites (`true`) or remove it (`false`).
    SetFavorite { path: PathBuf, favorite: bool },
    /// Ask which listed folders are gone (startup, window focus).
    CheckFolders,
    /// Result of `Effect::CheckRepoFolders`: what was found at each folder checked.
    FoldersChecked(Vec<FolderCheck>),
    /// Move favorite `from` so it ends up at index `to` (dragging, Move up and Move down).
    MoveFavorite { from: usize, to: usize },
    /// Open the favorite at this index (0 for the first; Open favorite repository 1 to 9).
    OpenFavorite(usize),
    /// Add an unloaded tab for each of these folders not shown by a tab and not missing, then
    /// switch to the first new one (Open all favorites).
    OpenAll(Vec<PathBuf>),
    /// Drop the entries the last check found missing from both lists (after the confirmation).
    RemoveMissing,
    /// Drop this folder from the Recent list and the favorites (a listed folder that is gone).
    Forget(PathBuf),
    /// Give the repository at `path` a colour, or with None take it away (the tab menu's and the
    /// repository list's Color submenus).
    SetColour { path: PathBuf, colour: Option<RepoColour> },
}

/// What `Effect::CheckRepoFolders` found at one listed folder.
#[derive(Clone, Debug, PartialEq)]
pub struct FolderCheck {
    pub path: PathBuf,
    /// A folder is there (false: gone, not a folder, or unreachable).
    pub present: bool,
    /// Its identity, when the volume gives one.
    pub folder: Option<FolderId>,
}

impl RecentMsg {
    /// What this message still needs when its tab is no longer active (`Msg::Scoped`): nothing,
    /// as none of these is ever sent scoped.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Remove(_)
            | Self::ClearRecent
            | Self::SetFavorite { .. }
            | Self::CheckFolders
            | Self::FoldersChecked(_)
            | Self::MoveFavorite { .. }
            | Self::OpenFavorite(_)
            | Self::OpenAll(_)
            | Self::RemoveMissing
            | Self::Forget(_)
            | Self::SetColour { .. } => None,
        }
    }
}
