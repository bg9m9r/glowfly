//! The network places the user agreed a folder's Git data may point to (Open anyway, saved in
//! `Settings::repo_network_places`), per folder: the opened folder's check
//! (`git_file::opened`) follows a place listed for the very folder it is looking at, and no
//! other folder's (not one above or inside it). Every open of a folder the user chose reads
//! the list this process was last given (`set`), so the background reads of an approved
//! repository open it as its load does, and an unapproved one is still refused.

use crate::paths::same_spelling;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// The approved places of each folder.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Approved {
    folders: Vec<(PathBuf, Vec<String>)>,
}

impl Approved {
    /// The places approved for each folder (as `net_path::place_text` writes them).
    pub fn new(folders: Vec<(PathBuf, Vec<String>)>) -> Self {
        Approved { folders }
    }

    /// The places approved for `folder` itself, matched by spelling (`paths::same_spelling`);
    /// none for any other folder.
    pub fn places_for(&self, folder: &Path) -> &[String] {
        self.folders.iter().find(|(f, _)| same_spelling(f, folder)).map_or(&[], |(_, p)| p.as_slice())
    }
}

static CURRENT: RwLock<Option<Arc<Approved>>> = RwLock::new(None);

/// Makes `approved` the list every later open in this process follows.
pub fn set(approved: Approved) {
    *CURRENT.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(approved));
}

/// The list `set` last gave (none before it).
pub fn current() -> Arc<Approved> {
    CURRENT.read().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_belong_to_their_own_folder_only() {
        let base = crate::paths::temp_dir().join("glowfly-approved-places");
        let (repo, inside, above) = (base.join("app"), base.join("app").join("sub"), base.clone());
        let a = Approved::new(vec![(repo.clone(), vec![r"\\srv\share".to_owned()])]);
        assert_eq!(a.places_for(&repo), [r"\\srv\share".to_owned()]);
        let respelled = PathBuf::from(format!("{}{}", repo.display(), std::path::MAIN_SEPARATOR));
        assert_eq!(a.places_for(&respelled).len(), 1, "a trailing separator is the same folder");
        assert!(a.places_for(&inside).is_empty() && a.places_for(&above).is_empty());
    }
}
