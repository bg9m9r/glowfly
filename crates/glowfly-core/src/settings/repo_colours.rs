//! Repository colours (Vivid colour): a quiet hue you can give each repository, shown only on
//! the window chrome (its tab, a line along the window's top edge, the repository list). Saved
//! per working folder under `repo_keys`, like the branch pins, as a stable name rather than a
//! slot number, so reordering the hues never recolours a repository and a name this version
//! doesn't know reads as no colour instead of failing the whole settings file.

use super::{Settings, repo_keys};
use std::path::Path;

/// One of the 8 repository hues, in the palette's slot order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RepoColour {
    Rose,
    Ochre,
    Fern,
    Teal,
    Sky,
    Slate,
    Iris,
    Plum,
}

impl RepoColour {
    /// Every hue, in slot order.
    pub const ALL: [RepoColour; 8] = [Self::Rose, Self::Ochre, Self::Fern, Self::Teal, Self::Sky, Self::Slate, Self::Iris, Self::Plum];

    /// The hue's slot in the palette's repository hues.
    pub fn index(self) -> usize {
        self as usize
    }

    /// The name saved in settings.json. These are stored in users' files: never rename them.
    pub fn id(self) -> &'static str {
        match self {
            Self::Rose => "rose",
            Self::Ochre => "ochre",
            Self::Fern => "fern",
            Self::Teal => "teal",
            Self::Sky => "sky",
            Self::Slate => "slate",
            Self::Iris => "iris",
            Self::Plum => "plum",
        }
    }

    /// The hue saved as `id`, or None for a name this version doesn't know.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.id() == id)
    }
}

impl Settings {
    /// The colour given to the repository at `path`, if any.
    pub fn repo_colour_for(&self, path: &Path) -> Option<RepoColour> {
        if self.repo_colours.is_empty() {
            return None;
        }
        repo_keys::get(&self.repo_colours, path).and_then(|s| RepoColour::from_id(s))
    }

    /// Gives the repository at `path` the colour `c`, or with None takes it away (an entry this
    /// version can't read goes too). Returns whether anything changed.
    pub fn set_repo_colour(&mut self, path: &Path, c: Option<RepoColour>) -> bool {
        let wanted = c.map(|c| c.id().to_owned());
        if repo_keys::get(&self.repo_colours, path).cloned() == wanted {
            return false;
        }
        repo_keys::set(&mut self.repo_colours, path, wanted);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn old_files_load_without_colours() {
        let old: Settings = serde_json::from_str(r#"{"auto_fetch_minutes":5}"#).unwrap();
        assert!(old.repo_colours.is_empty());
        assert_eq!(old.repo_colour_for(Path::new("/w/repo")), None);
    }

    #[test]
    fn colours_round_trip_through_the_file() {
        let dir = crate::paths::temp_dir().join(format!("glowfly-settings-colours-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let repo = dir.join("repo");
        let mut s = Settings::default();
        assert!(s.set_repo_colour(&repo, Some(RepoColour::Teal)));
        let file = dir.join("settings.json");
        s.save_to(&file).unwrap();
        let (back, problem) = Settings::load_from(&file);
        assert!(problem.is_none(), "{problem:?}");
        assert_eq!(back.repo_colour_for(&repo), Some(RepoColour::Teal));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unknown_colour_name_keeps_the_file_loading() {
        let dir = crate::paths::temp_dir().join(format!("glowfly-settings-colour-junk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("settings.json");
        std::fs::write(&file, r#"{"auto_fetch_minutes":5,"repo_colours":{"/w/repo":"turquoise"}}"#).unwrap();
        let (mut s, problem) = Settings::load_from(&file);
        assert!(problem.is_none(), "{problem:?}");
        assert_eq!(s.auto_fetch_minutes, 5, "the rest of the file still loads");
        assert_eq!(s.repo_colour_for(Path::new("/w/repo")), None);
        assert!(s.set_repo_colour(Path::new("/w/repo"), None), "None clears the junk entry");
        assert!(s.repo_colours.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn setting_reports_changes_and_keeps_one_entry_per_spelling() {
        let mut s = Settings::default();
        let repo = PathBuf::from("/w/repo");
        assert!(!s.set_repo_colour(&repo, None), "nothing to take away");
        assert!(s.set_repo_colour(&repo, Some(RepoColour::Iris)));
        assert!(!s.set_repo_colour(&repo, Some(RepoColour::Iris)), "already that colour");
        assert_eq!(s.repo_colour_for(Path::new("/w/./repo/")), Some(RepoColour::Iris));
        assert!(s.set_repo_colour(Path::new("/w/repo/"), Some(RepoColour::Fern)));
        assert_eq!(s.repo_colours.len(), 1, "one entry whatever the spelling");
        assert!(s.set_repo_colour(&repo, None));
        assert!(s.repo_colours.is_empty());
    }

    #[test]
    fn the_ids_never_change() {
        let ids: Vec<_> = RepoColour::ALL.iter().map(|c| c.id()).collect();
        assert_eq!(ids, ["rose", "ochre", "fern", "teal", "sky", "slate", "iris", "plum"]);
        for (i, c) in RepoColour::ALL.into_iter().enumerate() {
            assert_eq!(RepoColour::from_id(c.id()), Some(c));
            assert_eq!(c.index(), i);
        }
        assert_eq!(RepoColour::from_id("Rose"), None, "names are case-sensitive");
    }
}
