//! Messages for the recent and favorite repository lists (update/recent.rs).

use super::Inactive;
use std::path::PathBuf;
use trellis_core::paths::FolderId;

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
            Self::Remove(_) | Self::ClearRecent | Self::SetFavorite { .. } | Self::CheckFolders | Self::FoldersChecked(_) => None,
        }
    }
}
