//! Keeping submodules up to date: the Preferences > General switch and the three-way choice a
//! repository's SUBMODULES menu makes over it (saved per repository in
//! `Settings::repo_submodule_updates`, read through `repo_keys`).

use serde::{Deserialize, Serialize};

/// Preferences > General > Submodules.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SubmodulePrefs {
    /// Run `git submodule update --init --recursive` after a checkout, pull, merge, rebase or
    /// reset succeeds.
    pub keep_updated: bool,
}

/// A repository's choice about updating its submodules after checkouts and pulls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmoduleUpdates {
    /// Whatever Preferences > General says.
    FollowGlobal,
    Always,
    Never,
}

impl SubmoduleUpdates {
    /// The choice a saved override stands for (None follows the global setting).
    pub fn from_override(v: Option<bool>) -> Self {
        match v {
            None => Self::FollowGlobal,
            Some(true) => Self::Always,
            Some(false) => Self::Never,
        }
    }

    /// The override saved for this choice.
    pub fn as_override(self) -> Option<bool> {
        match self {
            Self::FollowGlobal => None,
            Self::Always => Some(true),
            Self::Never => Some(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::Settings;
    use super::super::tests::{json, scratch_settings};
    use super::*;

    #[test]
    fn old_files_keep_submodules_alone() {
        let (dir, file) = scratch_settings("submodules-old", br#"{"auto_fetch_minutes": 5}"#);
        let (s, problem) = Settings::load_from(&file);
        assert!(problem.is_none());
        assert!(!s.submodules.keep_updated);
        assert!(s.repo_submodule_updates.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn submodule_settings_round_trip() {
        let (dir, file) = scratch_settings("submodules-trip", b"{}");
        let mut s = Settings { submodules: SubmodulePrefs { keep_updated: true }, ..Settings::default() };
        s.repo_submodule_updates.insert("/w/repo".into(), false);
        s.save_to(&file).unwrap();
        let (back, problem) = Settings::load_from(&file);
        assert_eq!((json(&back), problem), (json(&s), None));
        assert!(back.submodules.keep_updated);
        assert_eq!(back.repo_submodule_updates.get("/w/repo"), Some(&false));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn overrides_map_to_choices_and_back() {
        for (v, c) in
            [(None, SubmoduleUpdates::FollowGlobal), (Some(true), SubmoduleUpdates::Always), (Some(false), SubmoduleUpdates::Never)]
        {
            assert_eq!(SubmoduleUpdates::from_override(v), c);
            assert_eq!(c.as_override(), v);
        }
    }
}
