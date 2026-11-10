//! Application state: plain data, no egui types. Only `update` changes it; views write `ViewState` and send their edits as `Msg::Edit`.

pub mod activity_copy;
pub mod activity_rows;
pub mod am_banner;
mod app_queries;
pub mod askpass;
pub mod auto_stash;
pub mod blame;
pub mod blame_find;
pub mod blocked;
pub mod branch_delete;
pub mod branch_sync;
pub mod branches;
pub mod build_status;
pub mod clone;
pub mod clone_service;
pub mod commit;
pub mod commit_menu;
pub(crate) mod confirm;
pub mod conflict;
pub mod conflict_check;
pub mod conflict_files;
pub mod conflict_picks;
pub mod conflict_strip;
pub mod custom_commands;
pub mod dashboard;
pub(crate) mod dashboard_plan;
pub mod dashboard_rows;
pub mod deep_link;
pub mod dialog_guard;
pub(crate) mod dialog_kind;
pub mod diff_header;
pub mod diff_nav;
pub mod diff_preview;
pub mod error_words;
pub mod files;
mod forge;
pub mod forge_filter_form;
pub mod forge_filters;
pub mod forks;
pub mod git_env;
pub mod gitflow;
pub mod gitflow_base;
pub mod gitflow_resume;
pub mod graph_anchor;
pub mod graph_rows;
pub mod hints;
pub mod history;
pub mod history_index;
pub mod history_text;
pub mod init;
pub mod issue;
pub mod issue_comment;
pub mod issue_form;
pub mod issue_words;
pub mod job_spec;
pub mod job_stop;
pub mod journal;
pub mod journal_lost;
pub mod journal_restore;
pub mod keys_page;
pub(crate) mod known_folders;
pub mod layout;
pub mod lfs;
pub mod lfs_notes;
pub mod line_counts;
pub mod lines;
pub mod load_more;
pub mod logs;
pub mod menu_bar;
pub mod menu_keys;
pub mod menu_model;
mod menu_model_items;
pub mod menu_native;
pub mod modal;
pub mod modal_form;
pub mod notify;
pub mod onboarding;
pub mod onboarding_tour;
pub mod op_actions;
pub mod op_confirm;
#[cfg(any(test, feature = "test-support"))]
pub mod op_fixture;
pub mod op_preview;
pub mod op_steps;
pub mod op_strip;
pub mod palette;
pub mod panel_layout;
pub mod partial;
pub mod partial_confirm;
pub mod patch;
pub mod path_hazards;
pub mod pick;
pub mod prefs;
pub mod prefs_fields;
pub mod prefs_save;
pub mod profile_rules;
pub mod profiles;
pub mod publish;
pub mod pull_request;
pub mod push_pr;
pub mod quit_confirm;
pub mod range_diff;
pub mod rebase;
pub mod rebase_banner;
pub(crate) mod rebase_merges;
pub mod rebase_onto;
pub mod rebase_quick;
pub mod rebase_remaining;
pub mod rebase_steps;
pub mod ref_rows;
pub mod refresh;
pub mod release_notes;
pub mod remote_branch;
pub mod remotes;
pub mod repo_colours;
pub mod repo_group_picker;
pub mod repo_groups;
pub mod repo_identity;
pub mod repo_lists;
pub mod repo_scan;
pub mod repo_sync;
mod running_job;
pub mod search;
pub mod secret_fields;
pub mod section_actions;
pub mod selection;
pub mod selection_span;
pub mod selection_wip;
pub mod session;
pub(crate) mod session_reads;
pub mod session_view;
pub mod settings_file;
pub mod shallow;
pub mod sign_in;
pub mod signatures;
pub mod sparse;
pub mod ssh;
pub mod stack;
pub mod stack_prs;
pub mod stack_push;
pub mod stack_rows;
pub mod stash_confirm;
pub mod stashes;
pub mod status_counts;
pub mod submodules;
pub mod tab_groups;
pub mod tab_losses;
pub mod tabs;
pub mod tag_delete;
pub mod tags;
pub mod terminal;
pub mod theme_names;
pub mod tip;
pub mod tls_help;
pub mod toasts;
pub mod tool_compare;
pub mod tool_git;
pub mod toolbar_tips;
pub mod tools;
pub mod touched;
pub mod trello;
pub mod trust;
pub mod trust_list;
pub mod undo;
pub mod undo_preview;
pub mod updates;
mod view;
pub mod watch;
pub mod window_chrome;
pub mod window_native;
pub mod worktree_branches;
pub mod worktree_manage;
pub mod worktree_move;
pub mod worktrees;

pub use commit::CommitDraft;
pub use forge::{ForgeState, ProjectKeys, Status, tracker_target};
pub use logs::{LOG_LIMIT, LogEntry};
pub use modal::Modal;
pub use prefs::{PrefPage, PrefsState, host_text};
pub use running_job::RunningJob;
pub use selection::Selection;
pub use tabs::Tab;
pub use toasts::{Toast, ToastAction};
pub use view::{ColumnDrag, ViewState, diff_text, file_folds, file_lists, file_tree, frame_times, lent, list_keys, system_a11y, wip_lists};

use glowfly_core::repo::Snapshot;
use glowfly_core::settings::Settings;
use std::{collections::HashMap, path::PathBuf, time::Duration, time::Instant};

/// Background refresh of pull requests and issues (GitHub allows 5,000 API calls an hour).
pub const FORGE_REFRESH_INTERVAL: Duration = Duration::from_secs(300);
/// Why selecting and applying single lines is refused in the whole-file view.
pub const WHOLE_FILE_BLOCKS_LINE_OPS: &str =
    "Line selection isn't available in the whole-file view yet. Switch to Hunks to stage, discard or revert single lines.";

/// The most commits "Load more" goes up to.
pub const MAX_COMMITS: usize = 200_000;

pub struct AppState {
    pub settings: Settings,
    /// The keys in effect: the defaults with `settings.keybindings` applied.
    pub keymap: crate::keymap::Keymap,
    /// The settings as last saved, and whether saving waits for the file to be read.
    pub settings_file: settings_file::SettingsFile,
    pub recent: Vec<PathBuf>,
    /// Recent or favorite repository folders found missing by the last check (`RecentMsg::FoldersChecked`).
    pub missing_repos: Vec<PathBuf>,
    /// A listed folder that failed to open, until the folder check says if it's gone, to offer removing it (update/recent.rs).
    pub missing_offer: Option<PathBuf>,
    /// The Find repositories dialog's scan of the project folders: global, never parked.
    pub repo_scan: repo_scan::RepoScan,
    /// The last fetch or pull across listed repositories, running or finished: global.
    pub repo_sync: Option<repo_sync::RepoSync>,
    /// The identities of listed and loaded folders, as worker threads found them.
    pub known: known_folders::KnownFolders,
    /// Open tabs; the active one's repository data is in the fields below.
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    /// Tab to go back to if opening a repository in a new tab fails.
    pub tab_return: Option<usize>,
    /// Recently closed repositories, most recent last.
    pub closed_tabs: Vec<tabs::ClosedTab>,
    pub snap: Option<Snapshot>,
    /// Incremented for every repository load; stale results are ignored.
    pub load_gen: u64,
    pub loading: bool,
    pub refresh: refresh::RefreshState,
    /// The active tab's per-tab state (its views with their counters, the open diff, the commit detail and draft, the repository
    /// reads, the selection, forge data, undo history, commit search and view state), parked whole with the tab (session.rs).
    pub session: session::RepoSession,
    /// Restacks stopped on a conflict, with the steps still to run once the banner's Continue or
    /// Skip finishes that step; keyed by working folder rather than parked with a tab.
    pub restacks: Vec<stack::PendingRestack>,
    pub job: Option<RunningJob>,
    /// The active job's stop, when it can be stopped (state/job_stop.rs).
    pub job_stop: Option<job_stop::JobStop>,
    /// Fetch full history's size check and a partial clone's missing contents (state/partial.rs).
    pub partial: partial::PartialState,
    /// The active tab's activity log (other tabs keep theirs in `Tab::log`; state/logs.rs).
    pub log: Vec<LogEntry>,
    /// The log files: persistence of the activity log, the panel's tab (state/logs.rs).
    pub logs: logs::LogsState,
    pub toasts: Vec<Toast>,
    /// Last toast id handed out (state/toasts.rs).
    pub toast_seq: u64,
    /// How far behind the checked-out branch was at the end of the auto-fetch, until its reload lands (update/notify.rs).
    pub behind_watch: Option<notify::BehindWatch>,
    pub modal: Option<Modal>,
    pub dialog: dialog_guard::DialogGuard,
    pub prefs: Option<PrefsState>,
    pub last_fetch: Instant,
    /// The file watcher for the active tab.
    pub watch: watch::WatchState,
    /// Credential prompts from git (global, not per tab).
    pub askpass: askpass::CredentialPrompts,
    /// Remotes auto-fetch skips until they are signed in to (global, not per tab).
    pub sign_in: sign_in::SignInNotices,
    /// Submodule updates after jobs and the offers made on open (global, not per tab).
    pub submodules: submodules::batch::SubmoduleSession,
    /// Git LFS: git-lfs installed, banner dismissals (global; the diff's LFS summary: `session.diff`).
    pub lfs: lfs::LfsSession,
    /// The git Glowfly runs and what it can do (global): checked at startup.
    pub git: git_env::GitState,
    /// The theme files in the themes folder (global): read at startup and on Reload.
    pub themes: crate::themes::ThemeState,
    /// The command palette, while open (global, not per tab).
    pub palette: Option<palette::Palette>,
    /// Incremented for every palette list request; older `TrackedPathsLoaded` results are ignored.
    pub palette_gen: u64,
    /// Plain commands recently run from the palette, most recent first (global, per session).
    pub palette_recent: palette::recent::RecentCommands,
    /// The built-in terminal panel (global, not per tab).
    pub terminal: terminal::TerminalPanel,
    /// The running Git Flow Finish job's steps (update/gitflow_finish.rs).
    pub gitflow_finish: Option<gitflow::FinishJob>,
    /// The running remote tag push or delete (update/tags.rs), for its headline.
    pub tag_job: Option<tags::RemoteTagJob>,
    /// The running rename that moves the branch on its remote too (update/branch_sync.rs).
    pub rename_job: Option<branch_sync::RenameJob>,
    pub tag_checkout: Option<tags::TagCheckoutJob>,
    /// The running clean-up after a pull request merge that deletes the local branch, for its headline.
    pub pr_cleanup: Option<pull_request::cleanup::CleanupJob>,
    /// The running Apply patch job (update/patch.rs), for its headline.
    pub patch_apply: Option<patch::PendingApply>,
    /// The glowfly:// link being opened and the scheme registration (global).
    pub deep_link: deep_link::DeepLinkState,
    /// The profile a switch is writing to the global git config (app-wide: the config is global).
    pub profile_switch: Option<glowfly_core::settings::ProfileId>,
    /// The profiles (current or the switch's target) edited while a switch ran: their git writes wait until it ends.
    pub profile_switch_edits: Vec<glowfly_core::settings::ProfileId>,
    /// The repositories' effective identities as git last reported them, by `Snapshot::git_dir` as spelled (global, kept across tabs).
    pub identities: HashMap<PathBuf, glowfly_core::repo_identity::RepoIdentity>,
    /// Writing the profiles' include files and global entries (global).
    pub includes: repo_identity::IncludeSync,
    /// Repository trust prompts asked and repositories just created, this session.
    pub trust: trust::TrustSession,
    /// The commands from the commands.toml files (custom_commands.rs).
    pub custom_commands: custom_commands::CustomCommands,
    /// The first-run welcome and the tour (global, not per tab; nothing saved but the welcome flag).
    pub onboarding: onboarding::Onboarding,
    /// The cross-repository dashboard: global, not parked with a tab.
    pub dashboard: dashboard::Dashboard,
    /// Auto-stashes being made or waiting to be put back, per workdir: global, not parked.
    pub auto_stash: auto_stash::AutoStashes,
    /// Glowfly's own update check and download (global; nothing saved but `settings.updates`).
    pub updates: updates::UpdatesState,
    pub view: lent::Lent<ViewState>,
    pub cues: view::ViewCues,
}

impl AppState {
    pub fn new(settings: Settings, recent: Vec<PathBuf>, now: Instant) -> Self {
        let settings_file = settings_file::SettingsFile::new(serde_json::to_string(&settings).unwrap_or_default());
        let watch = watch::WatchState { enabled: settings.watch_files, ..Default::default() };
        let terminal = terminal::TerminalPanel::restored(&settings.terminal);
        let (keymap, key_problems) = crate::keymap::Keymap::from_overrides(&settings.keybindings);
        let mut st = Self {
            settings,
            keymap,
            settings_file,
            recent,
            missing_repos: Vec::new(),
            missing_offer: None,
            repo_scan: Default::default(),
            repo_sync: None,
            known: Default::default(),
            tabs: vec![Tab::empty()],
            active_tab: 0,
            tab_return: None,
            closed_tabs: Vec::new(),
            snap: None,
            load_gen: 0,
            loading: false,
            refresh: Default::default(),
            session: Default::default(),
            restacks: Vec::new(),
            job: None,
            job_stop: None,
            partial: Default::default(),
            log: Vec::new(),
            logs: Default::default(),
            toasts: Vec::new(),
            toast_seq: 0,
            behind_watch: None,
            modal: None,
            dialog: Default::default(),
            prefs: None,
            last_fetch: now,
            watch,
            askpass: Default::default(),
            sign_in: Default::default(),
            submodules: Default::default(),
            lfs: Default::default(),
            git: Default::default(),
            themes: Default::default(),
            palette: None,
            palette_gen: 0,
            palette_recent: Default::default(),
            terminal,
            gitflow_finish: None,
            tag_job: None,
            rename_job: None,
            tag_checkout: None,
            pr_cleanup: None,
            patch_apply: None,
            deep_link: Default::default(),
            profile_switch: None,
            profile_switch_edits: Vec::new(),
            identities: HashMap::new(),
            includes: Default::default(),
            trust: Default::default(),
            custom_commands: Default::default(),
            onboarding: Default::default(),
            dashboard: Default::default(),
            auto_stash: Default::default(),
            updates: Default::default(),
            view: Default::default(),
            cues: Default::default(),
        };
        for p in key_problems {
            tracing::warn!("{p}");
            st.log(false, p);
        }
        st
    }
}
