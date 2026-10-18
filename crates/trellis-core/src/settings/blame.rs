//! The blame header's toggles: ignoring whitespace and following moved or copied lines.

use serde::{Deserialize, Serialize};

/// Which moved or copied lines blame follows to the commit that first wrote them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlameMoves {
    #[default]
    Off,
    /// Lines moved within the file (`-M`).
    WithinFile,
    /// Also lines moved or copied from other files changed in the same commit (`-M -C`).
    AcrossFiles,
}

impl BlameMoves {
    /// The `git blame` options for this choice.
    pub fn args(self) -> &'static [&'static str] {
        match self {
            BlameMoves::Off => &[],
            BlameMoves::WithinFile => &["-M"],
            BlameMoves::AcrossFiles => &["-M", "-C"],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BlamePrefs {
    /// Ignore whitespace changes (`-w`).
    pub ignore_whitespace: bool,
    pub moves: BlameMoves,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn old_files_load_the_defaults() {
        let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
        assert_eq!(s.blame, BlamePrefs::default());
        assert_eq!(s.blame.moves, BlameMoves::Off);
        assert!(!s.blame.ignore_whitespace);
    }

    #[test]
    fn blame_prefs_round_trip() {
        let on = Settings { blame: BlamePrefs { ignore_whitespace: true, moves: BlameMoves::AcrossFiles }, ..Settings::default() };
        let back: Settings = serde_json::from_str(&serde_json::to_string(&on).unwrap()).unwrap();
        assert_eq!(back.blame, on.blame);
        let partial: BlamePrefs = serde_json::from_str(r#"{"moves": "WithinFile"}"#).unwrap();
        assert_eq!(partial, BlamePrefs { ignore_whitespace: false, moves: BlameMoves::WithinFile });
    }

    #[test]
    fn moves_args() {
        assert!(BlameMoves::Off.args().is_empty());
        assert_eq!(BlameMoves::WithinFile.args(), ["-M"]);
        assert_eq!(BlameMoves::AcrossFiles.args(), ["-M", "-C"]);
    }
}
