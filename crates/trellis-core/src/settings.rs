//! App preferences persisted to `settings.json` in the config folder (see `paths`).

use crate::forge::{Account, Connection};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::ops::RangeInclusive;

mod column_widths;
mod file;
mod graph_filter;
mod paths;
mod profiles;
mod profiles_prefs;
pub mod repo_keys;
mod tools;
mod trust;
pub use column_widths::{ColumnWidths, ResizableColumn, drag_width};
pub use file::{LoadProblem, settings_file};
pub use graph_filter::{FilterOp, GraphFilter};
pub use paths::{cache_dir, config_dir, home_dir, migrate_old_config_dir};
pub use profiles::{DEFAULT_PROFILE_NAME, Profile, ProfileId, ProfileProblem};
pub use profiles_prefs::{ProfilePrefs, SetupSwap};
pub use tools::{EDITOR_FILE_ARGS, EDITOR_REPO_ARGS, ExternalTools};
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
    /// Load more history when the graph is scrolled near the end of a truncated history
    /// (Preferences > General).
    pub load_more_on_scroll: bool,
    pub reopen_last_repo: bool,
    /// Refresh when files change outside Trellis (Preferences > General).
    pub watch_files: bool,
    /// Dry-run merge the current branch against its upstream or default branch after loads and
    /// fetches (Preferences > General).
    pub conflict_check: bool,

    // UI customization
    pub theme: ThemeChoice,
    pub notification_location: Corner,
    /// After a background fetch, toast when the checked-out branch falls (further) behind its upstream (Preferences > UI Customization).
    pub behind_alerts: bool,
    /// While the window is unfocused, also show behind alerts and finished jobs as OS notifications (Preferences > UI Customization).
    pub desktop_notifications: bool,
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
    /// Graph column widths in px (drag a header divider); read through ColumnWidths::get.
    #[serde(default)]
    pub column_widths: ColumnWidths,

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
    /// Wrap long diff lines to the viewer's width (diff header toggle and Preferences > Editor).
    #[serde(default)]
    pub diff_word_wrap: bool,

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
    /// Issue tracker per repository, keyed by its resolved working directory (`repo_keys`).
    pub repo_trackers: HashMap<String, TrackerChoice>,
    /// Hidden and soloed refs per repository, keyed by its resolved working directory (`repo_keys`).
    #[serde(default)]
    pub graph_filters: HashMap<String, GraphFilter>,
    /// Trust decisions per repository, keyed by its resolved working directory
    /// (`Snapshot::real_workdir`, through `repo_keys`); a repository with no entry is undecided.
    #[serde(default)]
    pub repo_trust: HashMap<String, RepoTrust>,
    /// Favorite repositories (welcome screen and repository menu), as working-directory paths in
    /// the order they were added.
    #[serde(default)]
    pub favorite_repos: Vec<String>,
    /// Preferences > External Tools.
    #[serde(default)]
    pub external_tools: ExternalTools,
    /// Named profiles (Preferences > Profile): identities, and the accounts and preferences of
    /// the ones not current; seeded from the global git identity when the page first opens.
    #[serde(default)]
    pub profiles: Vec<Profile>,
    /// The profile whose identity is in the global git config.
    #[serde(default)]
    pub active_profile: Option<ProfileId>,
    /// Leave core.sshCommand unset and let ssh use the running agent (Preferences > SSH).
    #[serde(default)]
    pub use_ssh_agent: bool,
    /// Ask for OpenPGP signing passphrases in Trellis rather than GnuPG's own window
    /// (Preferences > Commit Signing).
    pub gpg_prompts_in_app: bool,
    /// Keyboard shortcuts the user changed (Preferences > Keyboard): a binding's stable id
    /// ("new-tab") -> its keys as text ("Mod+Shift+N"; Mod is Ctrl, or Cmd on macOS). An empty list
    /// leaves the action without a key; ids not listed keep their defaults. The app's keymap
    /// ignores what it can't use.
    #[serde(default)]
    pub keybindings: BTreeMap<String, Vec<String>>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_fetch_minutes: 1,
            auto_prune: true,
            default_branch: "main".into(),
            initial_commits: 2000,
            load_more_on_scroll: true,
            reopen_last_repo: true,
            watch_files: true,
            conflict_check: true,
            theme: ThemeChoice::Dark,
            notification_location: Corner::BottomLeft,
            behind_alerts: true,
            desktop_notifications: false,
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
            column_widths: ColumnWidths::default(),
            diff_layout: DiffLayout::Inline,
            diff_mode: DiffMode::Hunks,
            syntax_highlighting: true,
            diff_ignore_whitespace: false,
            diff_font_size: DEFAULT_DIFF_FONT_SIZE,
            tab_size: 4,
            diff_line_numbers: true,
            diff_word_highlights: true,
            diff_word_wrap: false,
            push_after_commit: false,
            stage_renames: true,
            push_new_tags: false,
            delete_to_trash: true,
            connections: Vec::new(),
            repo_trackers: HashMap::new(),
            graph_filters: HashMap::new(),
            repo_trust: HashMap::new(),
            favorite_repos: Vec::new(),
            external_tools: ExternalTools::default(),
            profiles: Vec::new(),
            active_profile: None,
            use_ssh_agent: false,
            gpg_prompts_in_app: true,
            keybindings: BTreeMap::new(),
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
