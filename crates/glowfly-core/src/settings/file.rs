//! Reading and writing `settings.json`, without ever losing a file Glowfly can't read.

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
                return (existing_defaults(), Some(LoadProblem { backup: None, reset: None, detail }));
            }
        };
        let (settings, reset, detail) = match serde_json::from_slice::<Settings>(&bytes) {
            Ok(s) => return (migrate(s), None),
            Err(e) => match lenient(&bytes) {
                Some((s, reset)) => (s, Some(reset), e.to_string()),
                None => (existing_defaults(), None, e.to_string()),
            },
        };
        let backup = match crate::persist::move_aside(path) {
            Ok(b) => Some(b),
            Err(e) => {
                // Keep the original where it is; defaults are used but the file isn't lost.
                let detail = format!("{detail}; could not back it up: {e}");
                return (existing_defaults(), Some(LoadProblem { backup: None, reset: None, detail }));
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

/// The serde default of `welcome_done`: a file without the key was written before the welcome
/// existed, by someone who already uses Glowfly.
pub(super) fn seen() -> bool {
    true
}

/// Defaults for a settings file that existed but couldn't be read: its owner has seen Glowfly
/// before, so the first-run welcome stays away.
fn existing_defaults() -> Settings {
    Settings { welcome_done: true, ..Settings::default() }
}

/// Early builds stored a preset name ("Iso", "Long", "Relative") in `date_format`; builds
/// before the patterns followed the locale saved the old built-in patterns, which mean "unset".
fn migrate(mut s: Settings) -> Settings {
    if matches!(s.date_format.as_str(), "Iso" | "Long" | "Relative") {
        s.relative_dates = s.date_format == "Relative";
        s.date_format = Settings::default().date_format;
    }
    forget_old_date_defaults(&mut s.date_format, &mut s.date_word_format, &mut s.date_time_format, &mut s.date_verbose_format);
    for p in s.profiles.iter_mut().filter_map(|p| p.prefs.as_mut()) {
        forget_old_date_defaults(&mut p.date_format, &mut p.date_word_format, &mut p.date_time_format, &mut p.date_verbose_format);
    }
    s
}

/// Clears each date pattern that still holds the built-in value older builds saved, so it
/// follows the date locale; any other pattern is the user's own and stays.
fn forget_old_date_defaults(date: &mut String, word: &mut String, time: &mut String, verbose: &mut String) {
    let fields = [(date, "MM/DD/YYYY"), (word, "MMMM D, YYYY"), (time, "MM/DD/YYYY @ h:mm A"), (verbose, "MMMM D, YYYY h:mm A")];
    for (field, old) in fields {
        if field.as_str() == old {
            field.clear();
        }
    }
}

/// Reads a JSON object one setting at a time, so a value this version doesn't understand (an
/// option added by a newer Glowfly, a hand edit) resets only that setting. Returns the names
/// of the settings that were reset, or `None` when the file isn't a JSON object at all.
fn lenient(bytes: &[u8]) -> Option<(Settings, Vec<String>)> {
    let Ok(Value::Object(file)) = serde_json::from_slice::<Value>(bytes) else { return None };
    // Seeded as an existing user's: a damaged file that lacks the key was still written before.
    let Ok(Value::Object(mut kept)) = serde_json::to_value(existing_defaults()) else { return None };
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
