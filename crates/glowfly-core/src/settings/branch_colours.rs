//! Branch colours in the settings: the palette preset (Preferences > Appearance > Commit
//! graph > Branch colors, switched with the profile) and the hues pinned to branches, saved per
//! repository under `repo_keys` like the graph filters. A pin is keyed by the branch's colour key
//! (`branch_colour::branch_key`), so a local branch and its remote copies share it.

use super::{Settings, repo_keys};
use crate::branch_colour::{COLOUR_BLIND_SLOTS, Colouring, SLOTS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Which branch hues the graph and every branch colour use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BranchPalette {
    /// The 14 standard hues.
    #[default]
    Standard,
    /// 8 hues that stay apart under red-green and blue-yellow colour blindness.
    ColourBlind,
}

impl BranchPalette {
    /// How many hues the preset has.
    pub fn slots(self) -> u8 {
        match self {
            Self::Standard => SLOTS,
            Self::ColourBlind => COLOUR_BLIND_SLOTS,
        }
    }
}

impl Settings {
    /// The hues pinned in the repository at `real`, by branch key (empty when there are none).
    pub fn branch_pins_for(&self, real: &Path) -> BTreeMap<String, u8> {
        repo_keys::get(&self.branch_pins, real).cloned().unwrap_or_default()
    }

    /// Pins `key` to `slot` in the repository at `real`, or with None lets it pick its own hue
    /// again (the repository's entry goes once it has no pins). Returns whether anything changed.
    pub fn set_branch_pin(&mut self, real: &Path, key: &str, slot: Option<u8>) -> bool {
        let mut pins = self.branch_pins_for(real);
        let changed = match slot {
            Some(s) => pins.insert(key.to_owned(), s) != Some(s),
            None => pins.remove(key).is_some(),
        };
        if changed {
            repo_keys::set(&mut self.branch_pins, real, (!pins.is_empty()).then_some(pins));
        }
        changed
    }

    /// How the repository at `real` colours its branches: the preset's slot count and its pins.
    pub fn colouring_for(&self, real: &Path) -> Colouring {
        Colouring { slots: self.branch_palette.slots(), pins: self.branch_pins_for(real) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn old_files_load_standard_without_pins() {
        let old: Settings = serde_json::from_str(r#"{"auto_fetch_minutes":5}"#).unwrap();
        assert_eq!(old.branch_palette, BranchPalette::Standard);
        assert!(old.branch_pins.is_empty());
        let back: BranchPalette = serde_json::from_str(&serde_json::to_string(&BranchPalette::ColourBlind).unwrap()).unwrap();
        assert_eq!(back, BranchPalette::ColourBlind);
    }

    #[test]
    fn pins_round_trip_through_the_file() {
        let mut s = Settings::default();
        let repo = PathBuf::from("/w/repo");
        assert!(s.set_branch_pin(&repo, "topic", Some(9)));
        s.branch_palette = BranchPalette::ColourBlind;
        let dir = crate::paths::temp_dir().join(format!("glowfly-settings-pins-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("settings.json");
        s.save_to(&file).unwrap();
        let (back, problem) = Settings::load_from(&file);
        assert!(problem.is_none(), "{problem:?}");
        assert_eq!(back.branch_pins_for(&repo), BTreeMap::from([("topic".to_owned(), 9)]));
        assert_eq!(back.branch_palette, BranchPalette::ColourBlind);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn setting_a_pin_reports_changes_and_drops_empty_entries() {
        let mut s = Settings::default();
        let repo = PathBuf::from("/w/repo");
        assert!(s.set_branch_pin(&repo, "topic", Some(3)));
        assert!(!s.set_branch_pin(&repo, "topic", Some(3)), "already pinned there");
        assert!(s.set_branch_pin(&repo, "fix-1", Some(5)));
        assert_eq!(s.branch_pins_for(Path::new("/w/./repo/")).len(), 2, "another spelling finds the entry");
        assert!(s.set_branch_pin(Path::new("/w/repo/"), "topic", None));
        assert!(!s.set_branch_pin(&repo, "topic", None), "nothing to remove");
        assert_eq!(s.branch_pins.len(), 1, "one entry whatever the spelling");
        assert!(s.set_branch_pin(&repo, "fix-1", None));
        assert!(s.branch_pins.is_empty(), "the repository's entry goes with its last pin");
    }

    #[test]
    fn the_colouring_takes_the_preset_and_the_pins() {
        let mut s = Settings::default();
        let repo = PathBuf::from("/w/repo");
        s.set_branch_pin(&repo, "topic", Some(11));
        let c = s.colouring_for(&repo);
        assert_eq!((c.n(), c.pins.get("topic")), (14, Some(&11)));
        s.branch_palette = BranchPalette::ColourBlind;
        let c = s.colouring_for(&repo);
        assert_eq!((c.n(), c.slot_for("topic")), (8, 3), "the pin keeps its number, shown modulo 8");
        assert!(s.colouring_for(Path::new("/w/other")).pins.is_empty());
    }
}
