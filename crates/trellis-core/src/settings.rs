//! App preferences persisted to `%APPDATA%\Trellis\settings.json` (or the XDG config dir).

use crate::forge::{Account, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum ThemeChoice {
    Dark,
    Light,
    /// Follow the operating system.
    System,
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum Corner {
    BottomLeft,
    BottomRight,
    TopLeft,
    TopRight,
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum DescriptionMode {
    Always,
    OnHover,
    Never,
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum BranchVisibility {
    All,
    /// The checked-out branch, its upstream, the default branch, and branches with open PRs.
    Smart,
}

/// Where a repository's issues come from (Repo-Specific Preferences > Issue Tracker).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum TrackerChoice {
    /// The code host's own issues (GitHub/GitLab/Bitbucket issues, Azure Boards).
    #[default]
    Auto,
    None,
    /// A connected Jira site, optionally limited to one project key.
    Jira {
        account: Account,
        project: String,
    },
}

/// Which parts of the commit graph are shown (UI Customization > graph toggles).
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct GraphColumns {
    pub refs: bool,
    pub tree: bool,
    pub sha: bool,
    pub message: bool,
    pub description: DescriptionMode,
    pub changes: bool,
    pub author: bool,
    pub date: bool,
}

impl Default for GraphColumns {
    fn default() -> Self {
        Self {
            refs: true,
            tree: true,
            sha: false,
            message: true,
            description: DescriptionMode::Always,
            changes: false,
            author: true,
            date: true,
        }
    }
}

impl GraphColumns {
    pub fn compact() -> Self {
        Self {
            refs: true,
            tree: true,
            sha: false,
            message: true,
            description: DescriptionMode::OnHover,
            changes: false,
            author: false,
            date: false,
        }
    }
}

/// How the diff viewer lays out a file's changes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum DiffLayout {
    /// Changes in one column, old and new numbers side by side.
    #[default]
    Inline,
    /// Old file on the left, new file on the right.
    Split,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Minutes between background fetches; 0 disables auto-fetch.
    pub auto_fetch_minutes: u32,
    pub auto_prune: bool,
    pub default_branch: String,
    pub initial_commits: usize,
    pub reopen_last_repo: bool,
    /// Refresh when files change outside Trellis (Preferences > General).
    pub watch_files: bool,

    // UI customization
    pub theme: ThemeChoice,
    pub notification_location: Corner,
    /// "system" or a locale such as "de_DE".
    pub date_locale: String,
    /// moment.js-style patterns.
    pub date_format: String,
    pub date_word_format: String,
    pub date_time_format: String,
    pub date_verbose_format: String,
    /// Show "3 hours ago" in the graph instead of the Date/Time format.
    pub relative_dates: bool,
    pub toolbar_labels: bool,
    pub spellcheck: bool,
    /// Initials instead of Gravatar / hosting-service avatars.
    pub use_initials: bool,
    pub ghost_refs: bool,
    pub highlight_ref_rows: bool,
    pub branch_visibility: BranchVisibility,
    pub columns: GraphColumns,

    // Diff viewer
    #[serde(default)]
    pub diff_layout: DiffLayout,
    /// Color diff text by language (Preferences > UI Customization).
    pub syntax_highlighting: bool,
    /// Hide whitespace-only changes in diffs (diff header toggle).
    #[serde(default)]
    pub diff_ignore_whitespace: bool,

    /// Connected services. Secrets live in the OS credential store or Git Credential
    /// Manager, never here.
    pub connections: Vec<Connection>,
    /// Issue tracker per repository working directory.
    pub repo_trackers: HashMap<String, TrackerChoice>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_fetch_minutes: 1,
            auto_prune: true,
            default_branch: "main".into(),
            initial_commits: 2000,
            reopen_last_repo: true,
            watch_files: true,
            theme: ThemeChoice::Dark,
            notification_location: Corner::BottomLeft,
            date_locale: "system".into(),
            date_format: "MM/DD/YYYY".into(),
            date_word_format: "MMMM D, YYYY".into(),
            date_time_format: "MM/DD/YYYY @ h:mm A".into(),
            date_verbose_format: "MMMM D, YYYY h:mm A".into(),
            relative_dates: false,
            toolbar_labels: true,
            spellcheck: true,
            use_initials: false,
            ghost_refs: true,
            highlight_ref_rows: true,
            branch_visibility: BranchVisibility::All,
            columns: GraphColumns::default(),
            diff_layout: DiffLayout::Inline,
            syntax_highlighting: true,
            diff_ignore_whitespace: false,
            connections: Vec::new(),
            repo_trackers: HashMap::new(),
        }
    }
}

pub const MIN_INITIAL_COMMITS: usize = 500;

fn config_base() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

pub fn config_dir() -> Option<PathBuf> {
    config_base().map(|b| b.join("Trellis"))
}

/// The app was called Riffle before. Moves its settings folder (settings, recent repositories,
/// tabs, avatar cache) to the new name, once, if there is nothing under the new name yet.
pub fn migrate_old_config_dir() -> std::io::Result<()> {
    match config_base() {
        Some(base) => move_dir_if_new_missing(&base.join("Riffle"), &base.join("Trellis")),
        None => Ok(()),
    }
}

fn move_dir_if_new_missing(old: &Path, new: &Path) -> std::io::Result<()> {
    if old.is_dir() && !new.exists() { std::fs::rename(old, new) } else { Ok(()) }
}

impl Settings {
    pub fn load() -> Settings {
        config_dir()
            .and_then(|d| std::fs::read_to_string(d.join("settings.json")).ok())
            .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
            .map(|mut s| {
                // Early builds stored a preset name ("Iso", "Long", "Relative") here.
                if matches!(s.date_format.as_str(), "Iso" | "Long" | "Relative") {
                    s.relative_dates = s.date_format == "Relative";
                    s.date_format = Settings::default().date_format;
                }
                s
            })
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(dir) = config_dir() else { return };
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(dir.join("settings.json"), text);
        }
    }

    pub fn connection(&self, account: &Account) -> Option<&Connection> {
        self.connections.iter().find(|c| &c.account == account)
    }

    pub fn tracker_for(&self, workdir: &std::path::Path) -> TrackerChoice {
        self.repo_trackers.get(&workdir.display().to_string()).cloned().unwrap_or_default()
    }

    pub fn commit_limit(&self) -> usize {
        self.initial_commits.max(MIN_INITIAL_COMMITS)
    }

    /// Graph and list dates: the Date/Time format, or relative ("3 hours ago") to `now`.
    pub fn format_time(&self, secs: i64, now: i64, utc_offset: i32) -> String {
        if self.relative_dates {
            return crate::dates::relative(now - secs);
        }
        crate::dates::format(secs, &self.date_time_format, &self.date_locale, utc_offset)
    }

    /// Commit details and tooltips: the verbose format.
    pub fn format_verbose(&self, secs: i64, utc_offset: i32) -> String {
        crate::dates::format(secs, &self.date_verbose_format, &self.date_locale, utc_offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_config_dir_moves_once() {
        let base = std::env::temp_dir().join(format!("trellis-migrate-{}", std::process::id()));
        let (old, new) = (base.join("Riffle"), base.join("Trellis"));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("recent.txt"), "C:/repo").unwrap();
        move_dir_if_new_missing(&old, &new).unwrap();
        assert!(!old.exists());
        assert_eq!(std::fs::read_to_string(new.join("recent.txt")).unwrap(), "C:/repo");

        // An existing new folder is never overwritten.
        std::fs::create_dir_all(&old).unwrap();
        move_dir_if_new_missing(&old, &new).unwrap();
        assert!(old.exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn settings_roundtrip_and_defaults() {
        // Missing fields fall back to defaults so old settings files keep working.
        let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5, "date_format": "Iso"}"#).unwrap_or_default();
        assert_eq!(s.auto_fetch_minutes, 5);
        assert_eq!(s.default_branch, "main");
        assert_eq!(s.columns, GraphColumns::default());
        assert_eq!(Settings { initial_commits: 10, ..Settings::default() }.commit_limit(), MIN_INITIAL_COMMITS);
    }

    #[test]
    fn diff_layout_defaults() {
        // Files written before the diff layout existed open in the inline layout.
        let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
        assert_eq!(s.diff_layout, DiffLayout::Inline);
        let split = Settings { diff_layout: DiffLayout::Split, ..Settings::default() };
        let back: Settings = serde_json::from_str(&serde_json::to_string(&split).unwrap()).unwrap();
        assert_eq!(back.diff_layout, DiffLayout::Split);
    }

    #[test]
    fn syntax_highlighting_defaults_on() {
        // Files written before the setting existed keep highlighting on.
        let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
        assert!(s.syntax_highlighting);
        let off = Settings { syntax_highlighting: false, ..Settings::default() };
        let back: Settings = serde_json::from_str(&serde_json::to_string(&off).unwrap()).unwrap();
        assert!(!back.syntax_highlighting);
    }

    #[test]
    fn ignore_whitespace_defaults_off() {
        // Files written before the toggle existed show every whitespace change.
        let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
        assert!(!s.diff_ignore_whitespace);
        let on = Settings { diff_ignore_whitespace: true, ..Settings::default() };
        let back: Settings = serde_json::from_str(&serde_json::to_string(&on).unwrap()).unwrap();
        assert!(back.diff_ignore_whitespace);
    }

    #[test]
    fn watch_files_defaults_on() {
        // Files written before the setting existed get live updates.
        let s: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
        assert!(s.watch_files);
        let off = Settings { watch_files: false, ..Settings::default() };
        let back: Settings = serde_json::from_str(&serde_json::to_string(&off).unwrap()).unwrap();
        assert!(!back.watch_files);
    }
}
