//! App preferences persisted to `settings.json` in the config folder (see `paths`).

use crate::forge::{Account, Connection};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

mod blame;
mod branch_colours;
mod builtin_themes;
mod column_widths;
mod date_fmt;
mod downloads;
mod editor;
mod file;
mod file_lists;
mod forge_filters;
mod graph_columns;
mod graph_filter;
mod history;
mod language;
mod panel_sizes;
mod paths;
mod profile_rules;
mod profiles;
mod profiles_prefs;
mod repo_colours;
pub mod repo_keys;
mod submodules;
mod terminal;
pub mod terminal_schemes;
mod theme_choice;
pub mod themes;
mod themes_io;
mod tools;
mod tracker;
mod trust;
mod ui_choices;
mod updates;
mod zoom;
pub use crate::dates::DateKind;
pub use blame::{BlameMoves, BlamePrefs};
pub use branch_colours::BranchPalette;
pub use column_widths::{ColumnWidths, ResizableColumn, drag_width};
pub use downloads::downloads_dir;
pub use editor::{DEFAULT_DIFF_FONT_SIZE, DIFF_FONT_SIZES, TAB_SIZES};
pub use file::{LoadProblem, settings_file};
pub use file_lists::FileListMode;
pub use forge_filters::RepoForgeFilters;
pub use graph_columns::GraphColumns;
pub use graph_filter::{FilterOp, GraphFilter};
pub use history::{HistoryPane, HistoryPrefs};
pub use panel_sizes::{PanelSizes, SizedPanel};
pub use paths::{cache_dir, config_dir, home_dir, log_dir, runtime_dir, xdg_data_dir};
pub use profile_rules::{RuleProblem, profile_for_git_dir, rule_for_git_dir, rule_matches, shown_folder};
pub use profiles::{DEFAULT_PROFILE_NAME, FolderRule, Profile, ProfileId, ProfileProblem};
pub use profiles_prefs::{ProfilePrefs, SetupSwap};
pub use repo_colours::RepoColour;
pub use submodules::{SubmodulePrefs, SubmoduleUpdates};
pub use terminal::{
    CursorStyle, DEFAULT_TERMINAL_FONT_SIZE, ShellChoice, ShellSetting, TERMINAL_FONT_SIZES, TerminalColours, TerminalPrefs,
};
pub use theme_choice::ThemeChoice;
pub use themes::{BuiltinTheme, ColorValue, ThemeCatalog, ThemeFile, ThemeProblem};
pub use tools::{EDITOR_FILE_ARGS, EDITOR_REPO_ARGS, ExternalTools};
pub use tracker::TrackerChoice;
pub use trust::{RepoTrust, SavedTrust};
pub use ui_choices::{BranchVisibility, Corner, DescriptionMode, DiffLayout, DiffMode};
pub use updates::UpdatePrefs;
pub use zoom::{DEFAULT_ZOOM_PERCENT, ZOOM_STEPS, snap_zoom, zoom_in_from, zoom_out_from};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Minutes between background fetches; 0 disables auto-fetch.
    pub auto_fetch_minutes: u32,
    pub auto_prune: bool,
    pub default_branch: String,
    pub initial_commits: usize,
    /// Load more history when the graph is scrolled near the end of a truncated history (Preferences > General).
    pub load_more_on_scroll: bool,
    /// Index large repositories' whole history for search (Preferences > General).
    pub index_history: bool,
    pub reopen_last_repo: bool,
    /// Refresh when files change outside Glowfly (Preferences > General).
    pub watch_files: bool,
    /// Dry-run merge the current branch against its upstream or default branch after loads and
    /// fetches (Preferences > General).
    pub conflict_check: bool,
    /// Update submodules after checkouts and pulls (Preferences > General).
    #[serde(default)]
    pub submodules: SubmodulePrefs,
    /// "system" or a code from i18n::Lang (Preferences > General).
    pub language: String,

    // Appearance
    pub theme: ThemeChoice,
    pub notification_location: Corner,
    /// After a background fetch, toast when the checked-out branch falls (further) behind its upstream (Preferences > Appearance).
    pub behind_alerts: bool,
    /// While the window is unfocused, also show behind alerts and finished jobs as OS notifications (Preferences > Appearance).
    pub desktop_notifications: bool,
    /// "system" or a locale such as "de_DE".
    pub date_locale: String,
    /// moment.js-style patterns; blank follows the date locale (`dates::default_pattern`).
    /// Date: file history and blame rows; Long date (`date_word_format`): created-on lines;
    /// Date and time (`date_time_format`): the graph; Full date and time
    /// (`date_verbose_format`): details and hovers.
    pub date_format: String,
    pub date_word_format: String,
    pub date_time_format: String,
    pub date_verbose_format: String,
    /// Show "3 hours ago" in the graph instead of the Date and time format.
    pub relative_dates: bool,
    pub toolbar_labels: bool,
    /// Tooltips and popups appear without fading or sliding in (egui's animations off).
    pub reduce_motion: bool,
    /// Window zoom in percent (keys, palette, Preferences > Appearance); read through
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
    /// Look up author avatars online (opt-in); was `use_initials`, which no file opts into now.
    pub avatar_lookups: bool,
    /// The quick-branch (New branch) button on the checked-out commit (Preferences > Appearance).
    pub ghost_refs: bool,
    /// Trace a branch on hover: light up its commits while the pointer is on its graph label.
    pub highlight_ref_rows: bool,
    /// Show a WIP row in the graph for each other worktree with uncommitted changes (Preferences > Appearance).
    pub worktree_wips: bool,
    /// Keep tabs of one repository's worktrees side by side (Preferences > Appearance).
    pub group_worktree_tabs: bool,
    pub branch_visibility: BranchVisibility,
    /// The branch hues: standard or colour-blind friendly (Preferences > Appearance).
    pub branch_palette: BranchPalette,
    pub columns: GraphColumns,
    /// Graph column widths in px (drag a header divider); read through ColumnWidths::get.
    #[serde(default)]
    pub column_widths: ColumnWidths,
    /// Changed files in one list or grouped in folders (list header toggle, Preferences > UI Customization).
    #[serde(default)]
    pub file_list_mode: FileListMode,

    // Diff viewer
    #[serde(default)]
    pub diff_layout: DiffLayout,
    /// Hunks only or the whole file (diff header toggle).
    #[serde(default)]
    pub diff_mode: DiffMode,
    /// Color diff text by language (Preferences > Diff viewer).
    pub syntax_highlighting: bool,
    /// Hide whitespace-only changes in diffs (diff header toggle).
    #[serde(default)]
    pub diff_ignore_whitespace: bool,
    /// Diff text size in points (Preferences > Diff viewer); read through `diff_font_px()`.
    pub diff_font_size: f32,
    /// Columns per tab stop when drawing diff text (Preferences > Diff viewer); read through `tab_columns()`.
    pub tab_size: u8,
    /// Old/new line numbers in the diff gutters (Preferences > Diff viewer).
    pub diff_line_numbers: bool,
    /// Changed-word fills on paired -/+ lines (Preferences > Diff viewer).
    pub diff_word_highlights: bool,
    /// Wrap long diff lines to the viewer's width (diff header toggle and Preferences > Diff viewer).
    #[serde(default)]
    pub diff_word_wrap: bool,
    /// The blame header's whitespace and move toggles.
    #[serde(default)]
    pub blame: BlamePrefs,
    /// The file history's Diff / File choice (header toggle and Preferences > Diff viewer).
    #[serde(default)]
    pub history: HistoryPrefs,

    /// Push the branch after each successful commit (commit box checkbox).
    pub push_after_commit: bool,
    /// Rename tracked files with git mv so the rename is staged (rename dialog checkbox).
    pub stage_renames: bool,
    /// Push a new tag to a remote right after creating it (New tag dialog checkbox).
    #[serde(default)]
    pub push_new_tags: bool,
    /// The one-time detached-HEAD notice after a tag checkout has been shown.
    #[serde(default)]
    pub detached_head_warned: bool,
    /// Delete… moves files to the OS trash instead of removing them (Preferences > General).
    pub delete_to_trash: bool,
    /// Stash uncommitted changes before checkout, pull, merge, rebase, cherry-pick and revert without asking (Preferences > General).
    pub auto_stash: bool,

    /// Connected services. Secrets live in the OS credential store or Git Credential
    /// Manager, never here.
    pub connections: Vec<Connection>,
    /// Issue tracker per repository, keyed by its resolved working directory (`repo_keys`).
    pub repo_trackers: HashMap<String, TrackerChoice>,
    /// Hidden and soloed refs per repository, keyed by its resolved working directory (`repo_keys`).
    #[serde(default)]
    pub graph_filters: HashMap<String, GraphFilter>,
    /// Hues pinned to branches (by branch key) per repository, keyed by `repo_keys`.
    #[serde(default)]
    pub branch_pins: HashMap<String, BTreeMap<String, u8>>,
    /// Repository colours (Vivid colour) by `repo_keys`, as `RepoColour::id` names; unknown names read as none.
    #[serde(default)]
    pub repo_colours: HashMap<String, String>,
    /// Saved pull request and issue filters and the ones shown, per repository, keyed by its
    /// resolved working directory (`repo_keys`).
    #[serde(default)]
    pub repo_forge_filters: HashMap<String, RepoForgeFilters>,
    /// Trust decisions per repository, keyed by its resolved working directory
    /// (`Snapshot::real_workdir`, through `repo_keys`); a repository with no entry is undecided.
    #[serde(default)]
    pub repo_trust: HashMap<String, RepoTrust>,
    /// What each Trusted decision was made on (`trust::TrustPrint`), keyed like `repo_trust`;
    /// none for decisions saved before prints were kept.
    #[serde(default)]
    pub repo_trust_prints: HashMap<String, crate::trust::TrustPrint>,
    /// Repositories the slow-status note was shown for, keyed by `repo_keys`.
    #[serde(default)]
    pub status_tip_shown: HashMap<String, bool>,
    /// Per-repository override of `submodules.keep_updated`, keyed by `repo_keys`; no entry follows it.
    #[serde(default)]
    pub repo_submodule_updates: HashMap<String, bool>,
    /// Repositories (by `repo_keys`) whose branch names like `2024-cleanup` name issue 2024 (This repository > Issue tracker).
    #[serde(default)]
    pub repo_year_branches: HashMap<String, bool>,
    /// Favorite repositories (welcome screen and repository menu), as working-directory paths in
    /// the order they were added.
    #[serde(default)]
    pub favorite_repos: Vec<String>,
    /// Folders the Find repositories dialog looks for repositories in (`paths::encode`d), in
    /// the order they were added.
    #[serde(default)]
    pub project_folders: Vec<String>,
    /// Preferences > External tools.
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
    /// Key pairs outside ~/.ssh that Preferences > SSH lists (Use existing).
    #[serde(default)]
    pub ssh_extra_keys: Vec<PathBuf>,
    /// Ask for OpenPGP signing passphrases in Glowfly rather than GnuPG's own window
    /// (Preferences > Commit Signing).
    pub gpg_prompts_in_app: bool,
    /// Save HTTPS sign-ins typed in the credential prompt with git's credential helper
    /// (Preferences > General).
    pub remember_https_credentials: bool,
    /// Add git commands and their timings to the activity log and write details to the error log (Preferences > General).
    #[serde(default)]
    pub extended_logging: bool,
    /// Show the last, average and slowest frame times in a corner of the window
    /// (Preferences > General > Diagnostics).
    #[serde(default)]
    pub show_frame_times: bool,
    /// Keyboard shortcuts the user changed (Preferences > Keyboard): a binding's stable id
    /// ("new-tab") -> its keys as text ("Mod+Shift+N"; Mod is Ctrl, or Cmd on macOS). An empty list
    /// leaves the action without a key; ids not listed keep their defaults. The app's keymap
    /// ignores what it can't use.
    #[serde(default)]
    pub keybindings: BTreeMap<String, Vec<String>>,
    /// The git program Glowfly runs (Preferences > Git): an absolute path, or None for git from
    /// PATH.
    pub git_program: Option<String>,
    /// The git version ("2.34.1") whose too-old banner was hidden with Don't show again. Another
    /// version shows the banner again, and it never hides the banner for a git that can't run.
    #[serde(default)]
    pub git_banner_hidden_for: Option<String>,
    /// The first-run welcome was finished or closed. Files written before it existed lack it and
    /// read as true, so only a new install shows the welcome.
    #[serde(default = "file::seen")]
    pub welcome_done: bool,
    /// The daily update check and the release notes last shown (Preferences > General > Updates).
    #[serde(default)]
    pub updates: UpdatePrefs,
    /// Preferences > Terminal, and the panel's remembered state.
    #[serde(default)]
    pub terminal: TerminalPrefs,
    /// The sizes the left, details and logs drawers were dragged to (the app clamps them).
    #[serde(default)]
    pub panel_sizes: PanelSizes,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_fetch_minutes: 1,
            auto_prune: true,
            default_branch: "main".into(),
            initial_commits: 2000,
            load_more_on_scroll: true,
            index_history: true,
            reopen_last_repo: true,
            watch_files: true,
            conflict_check: true,
            submodules: SubmodulePrefs::default(),
            language: crate::i18n::SYSTEM.into(),
            theme: ThemeChoice::Graphite,
            notification_location: Corner::BottomLeft,
            behind_alerts: true,
            desktop_notifications: false,
            date_locale: "system".into(),
            date_format: String::new(),
            date_word_format: String::new(),
            date_time_format: String::new(),
            date_verbose_format: String::new(),
            relative_dates: false,
            toolbar_labels: true,
            reduce_motion: false,
            zoom_percent: DEFAULT_ZOOM_PERCENT,
            show_left_panel: true,
            show_details_panel: true,
            show_toolbar: true,
            show_menu_bar: true,
            spellcheck: true,
            avatar_lookups: false,
            ghost_refs: true,
            highlight_ref_rows: true,
            worktree_wips: true,
            group_worktree_tabs: true,
            branch_visibility: BranchVisibility::All,
            branch_palette: BranchPalette::Standard,
            columns: GraphColumns::default(),
            column_widths: ColumnWidths::default(),
            file_list_mode: FileListMode::List,
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
            detached_head_warned: false,
            delete_to_trash: true,
            auto_stash: false,
            connections: Vec::new(),
            repo_trackers: HashMap::new(),
            graph_filters: HashMap::new(),
            branch_pins: HashMap::new(),
            repo_colours: HashMap::new(),
            repo_forge_filters: HashMap::new(),
            repo_trust: HashMap::new(),
            repo_trust_prints: HashMap::new(),
            status_tip_shown: HashMap::new(),
            repo_submodule_updates: HashMap::new(),
            repo_year_branches: HashMap::new(),
            favorite_repos: Vec::new(),
            project_folders: Vec::new(),
            external_tools: ExternalTools::default(),
            profiles: Vec::new(),
            active_profile: None,
            use_ssh_agent: false,
            ssh_extra_keys: Vec::new(),
            gpg_prompts_in_app: true,
            remember_https_credentials: true,
            extended_logging: false,
            show_frame_times: false,
            keybindings: BTreeMap::new(),
            git_program: None,
            git_banner_hidden_for: None,
            welcome_done: false,
            updates: UpdatePrefs::default(),
            terminal: TerminalPrefs::default(),
            panel_sizes: PanelSizes::default(),
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
mod date_migrate_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod updates_tests;
#[cfg(test)]
mod welcome_tests;
