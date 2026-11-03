//! App preferences persisted to `settings.json` in the config folder (see `paths`).

use crate::forge::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

mod a11y;
mod blame;
mod builtin_themes;
mod colour_palette;
mod column_widths;
pub mod commit_limit;
mod date_fmt;
mod defaults;
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
mod repo_groups;
pub mod repo_keys;
mod submodules;
mod terminal;
pub mod terminal_schemes;
mod theme_choice;
mod theme_pair;
pub mod themes;
mod themes_io;
mod tools;
mod tracker;
mod trust;
mod ui_choices;
mod updates;
mod zoom;
pub use crate::dates::DateKind;
pub use a11y::{A11yPrefs, FocusStyle, ToastTime, TooltipDelay};
pub use blame::{BlameMoves, BlamePrefs};
pub use colour_palette::ColourPalette;
pub use column_widths::{ColumnWidths, ResizableColumn, drag_width};
pub use downloads::downloads_dir;
pub use editor::{DEFAULT_DIFF_FONT_SIZE, DIFF_FONT_SIZES, TAB_SIZES};
pub use file::{LoadProblem, READ_WAITS, settings_file};
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
pub use repo_groups::{
    GROUP_ORDER, GroupColourDefault, MAX_GROUP_NAME, RepoGroup, clean_name as clean_group_name, default_colours, new_group_colour,
};
pub use submodules::{SubmodulePrefs, SubmoduleUpdates};
pub use terminal::{
    CursorStyle, DEFAULT_TERMINAL_FONT_SIZE, ShellChoice, ShellSetting, TERMINAL_FONT_SIZES, TerminalColours, TerminalPrefs,
};
pub use theme_choice::ThemeChoice;
pub use themes::{BuiltinTheme, ColorValue, SystemPair, ThemeCatalog, ThemeFile, ThemeProblem};
pub use tools::{EDITOR_FILE_ARGS, EDITOR_REPO_ARGS, ExternalTools};
pub use tracker::TrackerChoice;
pub use trust::{RepoTrust, SavedTrust};
pub use ui_choices::{BranchVisibility, Corner, Density, DescriptionMode, DiffLayout, DiffMode};
pub use updates::{NewsChip, UpdatePrefs, WhatsNew};
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
    /// Dry-run merge the current branch against its upstream or default branch after loads and fetches (Preferences > General).
    pub conflict_check: bool,
    /// Update submodules after checkouts and pulls (Preferences > General).
    pub submodules: SubmodulePrefs,
    /// "system" or a code from i18n::Lang (Preferences > General).
    pub language: String,

    // Appearance
    pub theme: ThemeChoice,
    /// The theme High contrast replaced, which turning it off brings back (Preferences > Accessibility; per profile, like `theme`).
    pub theme_before_contrast: Option<ThemeChoice>,
    pub system_pair: SystemPair,
    /// A retired theme the file named (as shown, and its successor), for the one-time toast.
    #[serde(skip)]
    pub retired_theme: Option<(String, BuiltinTheme)>,
    pub notification_location: Corner,
    /// After a background fetch, toast when the checked-out branch falls (further) behind its upstream (Preferences > Appearance).
    pub behind_alerts: bool,
    /// While the window is unfocused, also show behind alerts and finished jobs as OS notifications (Preferences > Appearance).
    pub desktop_notifications: bool,
    /// A short toast after each push and pull says what it did (Preferences > General > Notifications).
    pub confirm_push_pull: bool,
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
    /// Show ages ("3 h", "Yesterday") in the graph and "3 hours ago" in lists instead of the
    /// Date and time format; on for new settings files.
    pub relative_dates: bool,
    pub toolbar_labels: bool,
    /// Tooltips and popups appear without fading or sliding in (Preferences > Accessibility).
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
    /// Windows: the system's title bar and frame instead of the drawn ones, from the next start (Preferences > UI).
    pub system_title_bar: bool,
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
    #[serde(alias = "branch_palette")]
    pub colour_palette: ColourPalette,
    pub columns: GraphColumns,
    /// Graph column widths in px (drag a header divider); read through ColumnWidths::get.
    pub column_widths: ColumnWidths,
    /// The history table's row height (Preferences > Appearance).
    pub graph_density: Density,
    /// Changed files in one list or grouped in folders (list header toggle, Preferences > UI Customization).
    #[serde(default)]
    pub file_list_mode: FileListMode,

    // Diff viewer
    pub diff_layout: DiffLayout,
    /// Hunks only or the whole file (diff header toggle).
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
    pub blame: BlamePrefs,
    /// The file history's Diff / File choice (header toggle and Preferences > Diff viewer).
    pub history: HistoryPrefs,

    /// Push the branch after each successful commit (commit box checkbox).
    pub push_after_commit: bool,
    /// Rename tracked files with git mv so the rename is staged (rename dialog checkbox).
    pub stage_renames: bool,
    /// Push a new tag to a remote right after creating it (New tag dialog checkbox).
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
    pub graph_filters: HashMap<String, GraphFilter>,
    /// Hues pinned to branches (by branch key) per repository, keyed by `repo_keys`.
    pub branch_pins: HashMap<String, BTreeMap<String, u8>>,
    /// Repository colours (Vivid colour) by `repo_keys`, as `RepoColour::id` names; unknown names read as none.
    pub repo_colours: HashMap<String, String>,
    /// Saved pull request and issue filters and the ones shown, per repository, keyed by `repo_keys`.
    pub repo_forge_filters: HashMap<String, RepoForgeFilters>,
    /// Trust decisions per repository, keyed by `repo_keys`; a repository with no entry is undecided.
    pub repo_trust: HashMap<String, RepoTrust>,
    /// What each Trusted decision was made on (`trust::TrustPrint`), keyed like `repo_trust`; none for older decisions.
    pub repo_trust_prints: HashMap<String, crate::trust::TrustPrint>,
    /// Repositories the slow-status note was shown for, keyed by `repo_keys`.
    pub status_tip_shown: HashMap<String, bool>,
    /// Per-repository override of `submodules.keep_updated`, keyed by `repo_keys`; no entry follows it.
    pub repo_submodule_updates: HashMap<String, bool>,
    /// Repositories (by `repo_keys`) whose branch names like `2024-cleanup` name issue 2024 (This repository > Issue tracker).
    pub repo_year_branches: HashMap<String, bool>,
    /// The merge method last used in each repository's pull request panel (`MergeMethod::api_name`), by `repo_keys`.
    pub repo_merge_methods: HashMap<String, String>,
    /// Favorite repositories (welcome screen and repository menu), as working-directory paths in the order added.
    pub favorite_repos: Vec<String>,
    /// Named groups of favorites with their colours (`repo_groups`), in the order shown.
    pub repo_groups: Vec<RepoGroup>,
    /// The colour new groups take, and Reset all group colors gives (Preferences > UI Customization).
    pub new_group_colour: GroupColourDefault,
    /// Folders the Find repositories dialog looks for repositories in (`paths::encode`d), in the order they were added.
    pub project_folders: Vec<String>,
    /// Preferences > External tools.
    #[serde(default)]
    pub external_tools: ExternalTools,
    /// Named profiles (Preferences > Profile): identities, and the accounts and preferences of
    /// the ones not current; seeded from the global git identity when the page first opens.
    pub profiles: Vec<Profile>,
    /// The profile whose identity is in the global git config.
    #[serde(default)]
    pub active_profile: Option<ProfileId>,
    /// Leave core.sshCommand unset and let ssh use the running agent (Preferences > SSH).
    #[serde(default)]
    pub use_ssh_agent: bool,
    /// Key pairs outside ~/.ssh that Preferences > SSH lists (Use existing).
    pub ssh_extra_keys: Vec<PathBuf>,
    /// Ask for OpenPGP signing passphrases in Glowfly rather than GnuPG's own window (Preferences > Commit Signing).
    pub gpg_prompts_in_app: bool,
    /// Save HTTPS sign-ins typed in the credential prompt with git's credential helper (Preferences > General).
    pub remember_https_credentials: bool,
    /// Add git commands and their timings to the activity log and write details to the error log (Preferences > General).
    #[serde(default)]
    pub extended_logging: bool,
    /// Show the last, average and slowest frame times in a corner of the window (Preferences > General > Diagnostics).
    #[serde(default)]
    pub show_frame_times: bool,
    /// Keyboard shortcuts the user changed (Preferences > Keyboard): a binding's stable id
    /// ("new-tab") -> its keys as text ("Mod+Shift+N"; Mod is Ctrl, or Cmd on macOS). An empty list
    /// leaves the action without a key; ids not listed keep their defaults. The app's keymap
    /// ignores what it can't use.
    pub keybindings: BTreeMap<String, Vec<String>>,
    /// The git program Glowfly runs (Preferences > Git): an absolute path, or None for git from PATH.
    pub git_program: Option<String>,
    /// The git version ("2.34.1") whose too-old banner was hidden with Don't show again. Another
    /// version shows the banner again, and it never hides the banner for a git that can't run.
    #[serde(default)]
    pub git_banner_hidden_for: Option<String>,
    /// The first-run welcome was finished or closed. Files written before it existed lack it and
    /// read as true, so only a new install shows the welcome.
    #[serde(default = "file::seen")]
    pub welcome_done: bool,
    /// Tips while I work (Preferences > General); the tips retired and ignored once, by `HintId::id`.
    pub hints: bool,
    pub hints_seen: BTreeSet<String>,
    pub hints_ignored: BTreeSet<String>,
    /// The daily update check and the release notes last shown (Preferences > General > Updates).
    pub updates: UpdatePrefs,
    /// Preferences > Terminal, and the panel's remembered state.
    pub terminal: TerminalPrefs,
    /// The sizes the left, details and logs drawers were dragged to (the app clamps them).
    pub panel_sizes: PanelSizes,
    /// Preferences > Accessibility's own settings, flat in the file.
    #[serde(flatten)]
    pub a11y: A11yPrefs,
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
