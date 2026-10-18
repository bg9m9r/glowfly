//! App preferences persisted to `settings.json` in the config folder (see `paths`).

use crate::forge::{Account, Connection};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

mod blame;
mod column_widths;
mod date_fmt;
mod downloads;
mod editor;
mod file;
mod forge_filters;
mod graph_filter;
mod history;
mod language;
mod paths;
mod profiles;
mod profiles_prefs;
pub mod repo_keys;
mod submodules;
mod terminal;
pub mod themes;
mod themes_io;
mod tools;
mod trust;
mod updates;
mod zoom;
pub use blame::{BlameMoves, BlamePrefs};
pub use column_widths::{ColumnWidths, ResizableColumn, drag_width};
pub use date_fmt::{DEFAULT_DATE_FORMAT, DEFAULT_DATE_TIME_FORMAT, DEFAULT_DATE_VERBOSE_FORMAT, DEFAULT_DATE_WORD_FORMAT};
pub use downloads::downloads_dir;
pub use editor::{DEFAULT_DIFF_FONT_SIZE, DIFF_FONT_SIZES, TAB_SIZES};
pub use file::{LoadProblem, settings_file};
pub use forge_filters::RepoForgeFilters;
pub use graph_filter::{FilterOp, GraphFilter};
pub use history::{HistoryPane, HistoryPrefs};
pub use paths::{cache_dir, config_dir, home_dir, log_dir, migrate_old_config_dir, runtime_dir, xdg_data_dir};
pub use profiles::{DEFAULT_PROFILE_NAME, Profile, ProfileId, ProfileProblem};
pub use profiles_prefs::{ProfilePrefs, SetupSwap};
pub use submodules::{SubmodulePrefs, SubmoduleUpdates};
pub use terminal::{CursorStyle, DEFAULT_TERMINAL_FONT_SIZE, ShellChoice, ShellSetting, TERMINAL_FONT_SIZES, TerminalPrefs};
pub use themes::{BuiltinTheme, ColorValue, ThemeCatalog, ThemeFile, ThemeProblem};
pub use tools::{EDITOR_FILE_ARGS, EDITOR_REPO_ARGS, ExternalTools};
pub use trust::RepoTrust;
pub use updates::UpdatePrefs;
pub use zoom::{DEFAULT_ZOOM_PERCENT, ZOOM_STEPS, snap_zoom, zoom_in_from, zoom_out_from};

/// The theme in use (Preferences > UI Customization > Theme). Old files' "Dark", "Light" and
/// "System" load unchanged; a theme file is saved as `{"Custom": "ocean"}`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ThemeChoice {
    Dark,
    Light,
    /// Follow the operating system.
    System,
    HighContrastDark,
    HighContrastLight,
    Sand,
    /// A theme file in the themes folder, by file stem ("ocean" for themes/ocean.json).
    Custom(String),
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
    /// A connected Trello account, optionally limited to one board (empty: all boards).
    Trello {
        account: Account,
        #[serde(default)]
        board: String,
        #[serde(default)]
        board_name: String,
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
    /// Update submodules after checkouts and pulls (Preferences > General).
    #[serde(default)]
    pub submodules: SubmodulePrefs,
    /// "system" or a code from i18n::Lang (Preferences > General).
    pub language: String,

    // UI customization
    pub theme: ThemeChoice,
    pub notification_location: Corner,
    /// After a background fetch, toast when the checked-out branch falls (further) behind its upstream (Preferences > UI Customization).
    pub behind_alerts: bool,
    /// While the window is unfocused, also show behind alerts and finished jobs as OS notifications (Preferences > UI Customization).
    pub desktop_notifications: bool,
    /// "system" or a locale such as "de_DE".
    pub date_locale: String,
    /// moment.js-style patterns (see `date_fmt`). Date: file history and blame rows; Date word:
    /// created-on lines (issue panel, history footer).
    pub date_format: String,
    pub date_word_format: String,
    pub date_time_format: String,
    pub date_verbose_format: String,
    /// Show "3 hours ago" in the graph instead of the Date/Time format.
    pub relative_dates: bool,
    pub toolbar_labels: bool,
    /// Window zoom in percent (keys, palette, Preferences > UI Customization); read through
    /// `zoom_level()`/`zoom_factor()`.
    pub zoom_percent: u16,
    /// Draw the left panel of branches, remotes and other refs.
    pub show_left_panel: bool,
    /// Draw the right panel with WIP, commit and multi-commit details.
    pub show_details_panel: bool,
    /// Draw the toolbar at the top of the window.
    pub show_toolbar: bool,
    /// Draw the menu bar (File, Edit, View, Window, Help) at the top of the window.
    pub show_menu_bar: bool,
    pub spellcheck: bool,
    /// Initials instead of Gravatar / hosting-service avatars.
    pub use_initials: bool,
    pub ghost_refs: bool,
    pub highlight_ref_rows: bool,
    /// Show a WIP row in the graph for each other worktree with uncommitted changes
    /// (Preferences > UI Customization).
    pub worktree_wips: bool,
    /// Keep tabs of one repository's worktrees side by side (Preferences > UI Customization).
    pub group_worktree_tabs: bool,
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
    /// The blame header's whitespace and move toggles.
    #[serde(default)]
    pub blame: BlamePrefs,
    /// The file history's Diff / File choice (header toggle and Preferences > Editor).
    #[serde(default)]
    pub history: HistoryPrefs,

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
    /// Saved pull request and issue filters and the ones shown, per repository, keyed by its
    /// resolved working directory (`repo_keys`).
    #[serde(default)]
    pub repo_forge_filters: HashMap<String, RepoForgeFilters>,
    /// Trust decisions per repository, keyed by its resolved working directory
    /// (`Snapshot::real_workdir`, through `repo_keys`); a repository with no entry is undecided.
    #[serde(default)]
    pub repo_trust: HashMap<String, RepoTrust>,
    /// Repositories the slow-status note was shown for, keyed by `repo_keys`.
    #[serde(default)]
    pub status_tip_shown: HashMap<String, bool>,
    /// Per-repository override of `submodules.keep_updated`, keyed by `repo_keys`; no entry follows it.
    #[serde(default)]
    pub repo_submodule_updates: HashMap<String, bool>,
    /// Favorite repositories (welcome screen and repository menu), as working-directory paths in
    /// the order they were added.
    #[serde(default)]
    pub favorite_repos: Vec<String>,
    /// Folders the Find repositories dialog looks for repositories in (`paths::encode`d), in
    /// the order they were added.
    #[serde(default)]
    pub project_folders: Vec<String>,
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
    /// Save HTTPS sign-ins typed in the credential prompt with git's credential helper
    /// (Preferences > General).
    pub remember_https_credentials: bool,
    /// Add git commands and their timings to the activity log and write details to the error log (Preferences > General).
    #[serde(default)]
    pub extended_logging: bool,
    /// Keyboard shortcuts the user changed (Preferences > Keyboard): a binding's stable id
    /// ("new-tab") -> its keys as text ("Mod+Shift+N"; Mod is Ctrl, or Cmd on macOS). An empty list
    /// leaves the action without a key; ids not listed keep their defaults. The app's keymap
    /// ignores what it can't use.
    #[serde(default)]
    pub keybindings: BTreeMap<String, Vec<String>>,
    /// The git program Trellis runs (Preferences > Git): an absolute path, or None for git from
    /// PATH.
    pub git_program: Option<String>,
    /// The first-run welcome was finished or closed. Files written before it existed lack it and
    /// read as true, so only a new install shows the welcome.
    #[serde(default = "file::seen")]
    pub welcome_done: bool,
    /// The daily update check and the release notes last shown (Preferences > General > Updates).
    #[serde(default)]
    pub updates: UpdatePrefs,
    /// Preferences > In-App Terminal, and the panel's remembered state.
    #[serde(default)]
    pub terminal: TerminalPrefs,
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
            submodules: SubmodulePrefs::default(),
            language: crate::i18n::SYSTEM.into(),
            theme: ThemeChoice::Dark,
            notification_location: Corner::BottomLeft,
            behind_alerts: true,
            desktop_notifications: false,
            date_locale: "system".into(),
            date_format: DEFAULT_DATE_FORMAT.into(),
            date_word_format: DEFAULT_DATE_WORD_FORMAT.into(),
            date_time_format: DEFAULT_DATE_TIME_FORMAT.into(),
            date_verbose_format: DEFAULT_DATE_VERBOSE_FORMAT.into(),
            relative_dates: false,
            toolbar_labels: true,
            zoom_percent: DEFAULT_ZOOM_PERCENT,
            show_left_panel: true,
            show_details_panel: true,
            show_toolbar: true,
            show_menu_bar: true,
            spellcheck: true,
            use_initials: false,
            ghost_refs: true,
            highlight_ref_rows: true,
            worktree_wips: true,
            group_worktree_tabs: true,
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
            blame: BlamePrefs::default(),
            history: HistoryPrefs::default(),
            push_after_commit: false,
            stage_renames: true,
            push_new_tags: false,
            delete_to_trash: true,
            connections: Vec::new(),
            repo_trackers: HashMap::new(),
            graph_filters: HashMap::new(),
            repo_forge_filters: HashMap::new(),
            repo_trust: HashMap::new(),
            status_tip_shown: HashMap::new(),
            repo_submodule_updates: HashMap::new(),
            favorite_repos: Vec::new(),
            project_folders: Vec::new(),
            external_tools: ExternalTools::default(),
            profiles: Vec::new(),
            active_profile: None,
            use_ssh_agent: false,
            gpg_prompts_in_app: true,
            remember_https_credentials: true,
            extended_logging: false,
            keybindings: BTreeMap::new(),
            git_program: None,
            welcome_done: false,
            updates: UpdatePrefs::default(),
            terminal: TerminalPrefs::default(),
        }
    }
}

pub const MIN_INITIAL_COMMITS: usize = 500;

impl Settings {
    pub fn connection(&self, account: &Account) -> Option<&Connection> {
        self.connections.iter().find(|c| &c.account == account)
    }

    pub fn commit_limit(&self) -> usize {
        self.initial_commits.max(MIN_INITIAL_COMMITS)
    }
}

#[cfg(test)]
mod date_fmt_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod updates_tests;
#[cfg(test)]
mod welcome_tests;
