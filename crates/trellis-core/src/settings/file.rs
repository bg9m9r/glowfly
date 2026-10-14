//! Reading and writing `settings.json`, without ever losing a file Trellis can't read.

use super::{Settings, config_dir};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// Something went wrong reading the settings file; shown to the user once.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadProblem {
    /// Where the unreadable file was moved, if it was.
    pub backup: Option<PathBuf>,
    /// Settings that couldn't be read and were reset; `None` when all of them were.
    pub reset: Option<Vec<String>>,
    /// What failed, for the activity log.
    pub detail: String,
}

impl LoadProblem {
    pub fn message(&self) -> String {
        let what = match &self.reset {
            None => "Settings could not be read and were reset to defaults".to_owned(),
            Some(keys) if keys.is_empty() => "The settings file had a problem and was rewritten".to_owned(),
            Some(keys) => format!("Some settings could not be read and were reset: {}", keys.join(", ")),
        };
        let kept = match &self.backup {
            Some(b) => format!(" The original file was kept as {}.", b.display()),
            None => String::new(),
        };
        format!("{what} ({}).{kept}", self.detail)
    }
}

pub fn settings_file() -> Option<PathBuf> {
    config_dir().map(|d| d.join("settings.json"))
}

impl Settings {
    /// The saved settings, or defaults. A file that can't be read fully is moved aside first
    /// (so a later save can't overwrite it) and the problem is returned for the user.
    pub fn load() -> (Settings, Option<LoadProblem>) {
        match settings_file() {
            Some(f) => Settings::load_from(&f),
            None => (Settings::default(), None),
        }
    }

    pub fn load_from(path: &Path) -> (Settings, Option<LoadProblem>) {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Settings::default(), None),
            Err(e) => {
                let detail = format!("could not read {}: {e}", path.display());
                return (Settings::default(), Some(LoadProblem { backup: None, reset: None, detail }));
            }
        };
        let (settings, reset, detail) = match serde_json::from_slice::<Settings>(&bytes) {
            Ok(s) => return (migrate(s), None),
            Err(e) => match lenient(&bytes) {
                Some((s, reset)) => (s, Some(reset), e.to_string()),
                None => (Settings::default(), None, e.to_string()),
            },
        };
        let backup = match crate::persist::move_aside(path) {
            Ok(b) => Some(b),
            Err(e) => {
                // Keep the original where it is; defaults are used but the file isn't lost.
                let detail = format!("{detail}; could not back it up: {e}");
                return (Settings::default(), Some(LoadProblem { backup: None, reset: None, detail }));
            }
        };
        let settings = migrate(settings);
        // Write what could be kept, so the next start doesn't reset it again.
        if reset.is_some() {
            let _ = settings.save_to(path);
        }
        (settings, Some(LoadProblem { backup, reset, detail }))
    }

    pub fn save(&self) -> std::io::Result<()> {
        match settings_file() {
            Some(f) => self.save_to(&f),
            None => Ok(()),
        }
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        crate::persist::atomic_write(path, text.as_bytes())
    }
}

/// Early builds stored a preset name ("Iso", "Long", "Relative") in `date_format`.
fn migrate(mut s: Settings) -> Settings {
    if matches!(s.date_format.as_str(), "Iso" | "Long" | "Relative") {
        s.relative_dates = s.date_format == "Relative";
        s.date_format = Settings::default().date_format;
    }
    s
}

/// Reads a JSON object one setting at a time, so a value this version doesn't understand (an
/// option added by a newer Trellis, a hand edit) resets only that setting. Returns the names
/// of the settings that were reset, or `None` when the file isn't a JSON object at all.
fn lenient(bytes: &[u8]) -> Option<(Settings, Vec<String>)> {
    let Ok(Value::Object(file)) = serde_json::from_slice::<Value>(bytes) else { return None };
    let Ok(Value::Object(mut kept)) = serde_json::to_value(Settings::default()) else { return None };
    let mut reset = Vec::new();
    for (key, value) in file {
        let mut trial: Map<String, Value> = kept.clone();
        trial.insert(key.clone(), value);
        if serde_json::from_value::<Settings>(Value::Object(trial.clone())).is_ok() {
            kept = trial;
        } else {
            reset.push(key);
        }
    }
    let settings = serde_json::from_value(Value::Object(kept)).ok()?;
    Some((settings, reset))
}
