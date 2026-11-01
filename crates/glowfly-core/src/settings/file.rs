//! Reading and writing `settings.json`, without ever losing a file Glowfly can't read.

use super::{Settings, config_dir};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Something went wrong reading the settings file; shown to the user once.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadProblem {
    /// Where the unreadable file was moved, if it was.
    pub backup: Option<PathBuf>,
    /// Settings that couldn't be read and were reset; `None` when all of them were.
    pub reset: Option<Vec<String>>,
    /// What failed, for the activity log.
    pub detail: String,
    /// The file that couldn't be read (or kept a copy of) and is still where it was: defaults
    /// are in use, and saving them would overwrite it, so nothing may be saved until it reads
    /// cleanly (`Settings::reload`). `None` when saving is safe.
    pub locked: Option<PathBuf>,
}

impl LoadProblem {
    pub fn message(&self) -> String {
        let what = match (&self.locked, &self.reset) {
            (Some(_), _) => "Settings could not be read; defaults are used and nothing is saved until the file can be read".to_owned(),
            (None, None) => "Settings could not be read and were reset to defaults".to_owned(),
            (None, Some(keys)) if keys.is_empty() => "The settings file had a problem and was rewritten".to_owned(),
            (None, Some(keys)) => format!("Some settings could not be read and were reset: {}", keys.join(", ")),
        };
        let kept = match &self.backup {
            Some(b) => format!(" The original file was kept as {}.", b.display()),
            None => String::new(),
        };
        format!("{what} ({}).{kept}", self.detail)
    }

    /// The problem of a file that stays where it is, unread: saving must wait.
    fn locked(path: &Path, detail: String) -> Self {
        LoadProblem { backup: None, reset: None, detail, locked: Some(path.to_owned()) }
    }
}

/// How long to wait before each new try at reading a file in the config folder (settings, recent
/// repositories, tabs) after one fails: a virus scanner or sync client holding it (a Windows
/// sharing violation) lets go within moments.
pub const READ_WAITS: [Duration; 3] = [Duration::from_millis(40), Duration::from_millis(120), Duration::from_millis(350)];

pub fn settings_file() -> Option<PathBuf> {
    config_dir().map(|d| d.join("settings.json"))
}

impl Settings {
    /// The saved settings, or defaults. A file that can't be read fully is moved aside first
    /// (so a later save can't overwrite it) and the problem is returned for the user; one that
    /// can't be read at all, or moved, stays put and the problem says saving must wait.
    pub fn load() -> (Settings, Option<LoadProblem>) {
        match settings_file() {
            Some(f) => Settings::load_from(&f),
            None => (Settings::default(), None),
        }
    }

    /// `load` again after it reported a `locked` problem: the file is first copied to `.bak`
    /// once it reads, since the defaults in use since then may be saved over it next.
    pub fn reload() -> (Settings, Option<LoadProblem>) {
        match settings_file() {
            Some(f) => Settings::load_with(&f, &READ_WAITS, |p: &Path| std::fs::read(p), true),
            None => (Settings::default(), None),
        }
    }

    pub fn load_from(path: &Path) -> (Settings, Option<LoadProblem>) {
        Settings::load_with(path, &READ_WAITS, |p: &Path| std::fs::read(p), false)
    }

    /// `load_from`, reading with `read` and trying again after each of `waits` while it fails.
    /// When a try failed (here, or `failed_before` in an earlier load), what was read is copied
    /// to `.bak` before it is returned, so the first save after it can't lose the file.
    pub fn load_with(
        path: &Path,
        waits: &[Duration],
        mut read: impl FnMut(&Path) -> std::io::Result<Vec<u8>>,
        failed_before: bool,
    ) -> (Settings, Option<LoadProblem>) {
        let mut waits = waits.iter();
        let mut failed = failed_before;
        let bytes = loop {
            match read(path) {
                Ok(b) => break b,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Settings::default(), None),
                Err(e) => match waits.next() {
                    Some(wait) => {
                        failed = true;
                        std::thread::sleep(*wait);
                    }
                    None => {
                        let detail = format!("could not read {}: {e}", path.display());
                        return (existing_defaults(), Some(LoadProblem::locked(path, detail)));
                    }
                },
            }
        };
        // A retired theme met before this read isn't this file's.
        let _ = super::theme_choice::take_retired();
        let (settings, reset, detail) = match serde_json::from_slice::<Settings>(&bytes) {
            Ok(s) if !failed => return (migrate(s), None),
            Ok(s) => {
                return match crate::persist::keep_copy(path, &bytes) {
                    Ok(_) => (migrate(s), None),
                    Err(e) => {
                        let detail = format!("read {} only after it failed, and could not back it up: {e}", path.display());
                        (existing_defaults(), Some(LoadProblem::locked(path, detail)))
                    }
                };
            }
            Err(e) => match lenient(&bytes) {
                Some((s, reset)) => (s, Some(reset), e.to_string()),
                None => (existing_defaults(), None, e.to_string()),
            },
        };
        // Moved aside, the original can't be overwritten by any save after this.
        let backup = match crate::persist::move_aside(path) {
            Ok(b) => Some(b),
            Err(e) => {
                // Keep the original where it is; defaults are used but the file isn't lost.
                let detail = format!("{detail}; could not back it up: {e}");
                return (existing_defaults(), Some(LoadProblem::locked(path, detail)));
            }
        };
        let settings = migrate(settings);
        // Write what could be kept, so the next start doesn't reset it again.
        if reset.is_some() {
            let _ = settings.save_to(path);
        }
        (settings, Some(LoadProblem { backup, reset, detail, locked: None }))
    }

    pub fn save(&self) -> std::io::Result<()> {
        match settings_file() {
            Some(f) => self.save_to(&f),
            None => Ok(()),
        }
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        // It names repositories, profiles, emails and trust decisions: for this user only.
        crate::persist::write_file(path, text.as_bytes(), crate::persist::WriteOptions { private: true, ..Default::default() })
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
/// A retired theme (in `theme` or a profile's copy) has already loaded as its successor; the
/// first one read is kept for the toast.
fn migrate(mut s: Settings) -> Settings {
    s.retired_theme = super::theme_choice::take_retired();
    // Terminal colours saved by a built-in's id: the old `dark` / `light` and removed themes'
    // ids take the current id, so the Colors menu ticks the theme they open in.
    if let super::TerminalColours::Theme(id) = &mut s.terminal.colours
        && let Some(b) = super::themes::BuiltinTheme::from_id(id)
    {
        *id = b.id().to_owned();
    }
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

#[cfg(test)]
#[path = "file_tests.rs"]
mod tests;
