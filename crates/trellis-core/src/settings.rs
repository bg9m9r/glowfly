//! App preferences persisted to `settings.json` in the config folder (see `paths`).

use crate::forge::{Account, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::RangeInclusive;

mod file;
mod graph_filter;
mod paths;
mod trust;
pub use file::{LoadProblem, settings_file};
pub use graph_filter::{FilterOp, GraphFilter};
pub use paths::{config_dir, migrate_old_config_dir};
pub use trust::RepoTrust;

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

/// How much of a file the diff viewer shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum DiffMode {
    /// Only the changes, with a few lines of context.
    #[default]
    Hunks,
    /// Every line of the file, changes marked in place.
    WholeFile,
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
    /// Hunks only or the whole file (diff header toggle).
    #[serde(default)]
    pub diff_mode: DiffMode,
    /// Color diff text by language (Preferences > Editor).
    pub syntax_highlighting: bool,
    /// Hide whitespace-only changes in diffs (diff header toggle).
    #[serde(default)]
    pub diff_ignore_whitespace: bool,
    /// Diff text size in points (Preferences > Editor); read through `diff_font_px()`.
    pub diff_font_size: f32,
    /// Columns per tab stop when drawing diff text (Preferences > Editor); read through
    /// `tab_columns()`.
    pub tab_size: u8,
    /// Old/new line numbers in the diff gutters (Preferences > Editor).
    pub diff_line_numbers: bool,
    /// Changed-word fills on paired -/+ lines (Preferences > Editor).
    pub diff_word_highlights: bool,

    /// Push the branch after each successful commit (commit box checkbox).
    pub push_after_commit: bool,
    /// Rename tracked files with git mv so the rename is staged (rename dialog checkbox).
    pub stage_renames: bool,
    /// Push a new tag to a remote right after creating it (New tag dialog checkbox).
    #[serde(default)]
    pub push_new_tags: bool,
    /// Delete… moves files to the OS trash instead of removing them (Preferences > General).
    pub delete_to_trash: bool,

    /// Connected services. Secrets live in the OS credential store or Git Credential
    /// Manager, never here.
    pub connections: Vec<Connection>,
    /// Issue tracker per repository working directory.
    pub repo_trackers: HashMap<String, TrackerChoice>,
    /// Hidden and soloed refs per repository working directory.
    #[serde(default)]
    pub graph_filters: HashMap<String, GraphFilter>,
    /// Trust decisions per repository, keyed by its resolved working directory
    /// (`Snapshot::real_workdir`); a repository with no entry is undecided.
    #[serde(default)]
    pub repo_trust: HashMap<String, RepoTrust>,
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
            diff_mode: DiffMode::Hunks,
            syntax_highlighting: true,
            diff_ignore_whitespace: false,
            diff_font_size: DEFAULT_DIFF_FONT_SIZE,
            tab_size: 4,
            diff_line_numbers: true,
            diff_word_highlights: true,
            push_after_commit: false,
            stage_renames: true,
            push_new_tags: false,
            delete_to_trash: true,
            connections: Vec::new(),
            repo_trackers: HashMap::new(),
            graph_filters: HashMap::new(),
            repo_trust: HashMap::new(),
        }
    }
}

pub const MIN_INITIAL_COMMITS: usize = 500;
/// The diff viewer's text size when nothing else is set.
pub const DEFAULT_DIFF_FONT_SIZE: f32 = 12.5;
/// The diff text sizes Preferences > Editor offers.
pub const DIFF_FONT_SIZES: RangeInclusive<f32> = 8.0..=24.0;
/// The tab sizes Preferences > Editor offers.
pub const TAB_SIZES: RangeInclusive<u8> = 1..=16;

impl Settings {
    pub fn connection(&self, account: &Account) -> Option<&Connection> {
        self.connections.iter().find(|c| &c.account == account)
    }

    pub fn tracker_for(&self, workdir: &std::path::Path) -> TrackerChoice {
        self.repo_trackers.get(&workdir.display().to_string()).cloned().unwrap_or_default()
    }

    pub fn commit_limit(&self) -> usize {
        self.initial_commits.max(MIN_INITIAL_COMMITS)
    }

    /// The diff text size, kept within `DIFF_FONT_SIZES` (the default for a non-number from a
    /// hand-edited file).
    pub fn diff_font_px(&self) -> f32 {
        if !self.diff_font_size.is_finite() {
            return DEFAULT_DIFF_FONT_SIZE;
        }
        self.diff_font_size.clamp(*DIFF_FONT_SIZES.start(), *DIFF_FONT_SIZES.end())
    }

    /// Columns per tab stop, kept within `TAB_SIZES`.
    pub fn tab_columns(&self) -> usize {
        self.tab_size.clamp(*TAB_SIZES.start(), *TAB_SIZES.end()) as usize
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
mod tests;
