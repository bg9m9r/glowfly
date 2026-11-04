//! Colours in the settings: the palette preset (Preferences > Accessibility > Colours: standard
//! or colour-blind safe branch, diff and file status colours, one for the whole machine) and the
//! hues pinned to branches, saved per
//! repository under `repo_keys` like the graph filters. A pin is keyed by the branch's colour key
//! (`branch_colour::branch_key`), so a local branch and its remote copies share it. The preset
//! is `Settings::colour_palette`, saved as `branch_palette` before it covered diffs and status
//! (an alias keeps those files loading).

use super::{Settings, repo_keys};
use crate::branch_colour::{COLOUR_BLIND_SLOTS, Colouring, SLOTS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Which colours branches, diffs and file status use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColourPalette {
    /// The 14 standard hues.
    #[default]
    Standard,
    /// Colours that stay apart under red-green and blue-yellow colour blindness: 8 branch hues,
    /// and blue, orange and violet for added, removed and modified (`theme/colour_safe.rs`).
    ColourBlind,
}

impl ColourPalette {
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

    /// How the repository at `real` colours its branches: the preset's slot count and its pins
    /// (no slots near the trunk: those come from the palette, which `update` adds).
    pub fn colouring_for(&self, real: &Path) -> Colouring {
        Colouring { slots: self.colour_palette.slots(), pins: self.branch_pins_for(real), trunk_near: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn old_files_load_standard_without_pins() {
        let old: Settings = serde_json::from_str(r#"{"auto_fetch_minutes":5}"#).unwrap();
        assert_eq!(old.colour_palette, ColourPalette::Standard);
        assert!(old.branch_pins.is_empty());
        let back: ColourPalette = serde_json::from_str(&serde_json::to_string(&ColourPalette::ColourBlind).unwrap()).unwrap();
        assert_eq!(back, ColourPalette::ColourBlind);
    }

    #[test]
    fn files_naming_the_old_field_keep_their_choice() {
        let old: Settings = serde_json::from_str(r#"{"branch_palette":"ColourBlind"}"#).unwrap();
        assert_eq!(old.colour_palette, ColourPalette::ColourBlind);
        let saved = serde_json::to_value(&old).unwrap();
        assert_eq!(saved["colour_palette"], "ColourBlind", "saved under the new name");
        assert!(saved.get("branch_palette").is_none());
    }

    #[test]
    fn pins_round_trip_through_the_file() {
        let mut s = Settings::default();
        let repo = PathBuf::from("/w/repo");
        assert!(s.set_branch_pin(&repo, "topic", Some(9)));
        s.colour_palette = ColourPalette::ColourBlind;
        let dir = crate::paths::temp_dir().join(format!("glowfly-settings-pins-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("settings.json");
        s.save_to(&file).unwrap();
        let (back, problem) = Settings::load_from(&file);
        assert!(problem.is_none(), "{problem:?}");
        assert_eq!(back.branch_pins_for(&repo), BTreeMap::from([("topic".to_owned(), 9)]));
        assert_eq!(back.colour_palette, ColourPalette::ColourBlind);
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
        s.colour_palette = ColourPalette::ColourBlind;
        let c = s.colouring_for(&repo);
        assert_eq!((c.n(), c.slot_for("topic")), (8, 3), "the pin keeps its number, shown modulo 8");
        assert!(s.colouring_for(Path::new("/w/other")).pins.is_empty());
    }
}
