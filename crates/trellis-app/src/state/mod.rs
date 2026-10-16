//! Application state: plain data, no egui types. Only `update` changes it, except for
//! transient view state (`ViewState`) and text buffers that views edit in place.

mod app_queries;
pub mod askpass;
pub mod blame;
pub mod branches;
pub mod build_status;
pub mod clone;
pub mod clone_service;
pub mod commit;
pub mod conflict;
pub mod conflict_check;
pub mod dashboard;
pub mod dashboard_plan;
pub mod dashboard_rows;
pub mod deep_link;
pub mod diff_nav;
pub mod files;
mod forge;
pub mod forge_filter_form;
pub mod forge_filters;
pub mod forks;
pub mod git_env;
pub mod gitflow;
pub mod graph_rows;
pub mod history;
pub mod init;
pub mod issue;
pub mod issue_form;
pub mod keys_page;
pub mod known_folders;
pub mod layout;
pub mod lfs;
pub mod lines;
pub mod logs;
pub mod notify;
pub mod onboarding;
pub mod onboarding_tour;
pub mod palette;
pub mod palette_commands;
pub mod palette_kinds;
pub mod palette_onboarding;
pub mod palette_profiles;
pub mod palette_recent;
pub mod palette_tools;
pub mod patch;
pub mod path_hazards;
pub mod pr_details;
pub mod prefs;
pub mod profiles;
pub mod publish;
pub mod pull_request;
pub mod pull_request_actions;
pub mod pull_request_threads;
pub mod push_pr;
pub mod rebase;
pub mod ref_rows;
pub mod refresh;
pub mod remotes;
pub mod repo_lists;
pub mod search;
pub mod selection;
pub mod shallow;
pub mod signatures;
pub mod sparse;
pub mod ssh;
pub mod ssh_service;
pub mod ssh_use;
pub mod stashes;
pub mod submodules;
pub mod tabs;
pub mod tags;
pub mod terminal;
pub mod toasts;
pub mod tool_compare;
pub mod tools;
pub mod trello;
pub mod trust;
pub mod undo;
mod view;
pub mod watch;
pub mod worktree_manage;
pub mod worktrees;

pub use commit::CommitDraft;
pub use forge::{ForgeState, ProjectKeys, Status, tracker_target};
pub use logs::{LOG_LIMIT, LogEntry};
pub use prefs::{PrefPage, PrefsState, host_text};
pub use tabs::Tab;
pub use toasts::{Toast, ToastAction};
pub use view::{ColumnDrag, ViewState};

use crate::msg::Msg;
use git2::Oid;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use trellis_core::repo::{CommitDetail, DiffTarget, FileDiff, Snapshot};
use trellis_core::settings::Settings;

/// Background refresh of pull requests and issues (GitHub allows 5,000 API calls an hour).
pub const FORGE_REFRESH_INTERVAL: Duration = Duration::from_secs(300);
/// Why hunk and line operations are off while the diff hides whitespace changes.
pub const WHITESPACE_BLOCKS_HUNK_OPS: &str =
    "Turn off Ignore whitespace to stage, unstage or discard hunks and lines. The hidden whitespace changes are part of each hunk.";

/// Why Revert Hunk on a commit or comparison diff is refused while whitespace is hidden.
pub const WHITESPACE_BLOCKS_REVERT: &str =
    "Turn off Ignore whitespace to revert hunks. The hidden whitespace changes are part of each hunk.";

/// Why selecting and applying single lines is refused in the whole-file view.
pub const WHOLE_FILE_BLOCKS_LINE_OPS: &str =
    "Line staging isn't available in the whole-file view yet. Switch to Hunks to stage single lines.";

#[derive(Clone, PartialEq, Debug, Default)]
pub enum Selection {
    #[default]
    None,
    Wip,
    Commit(Oid),
    /// Two or more commits (Ctrl- or Shift-click in the graph).
    Commits(selection::MultiSelection),
}

impl Selection {
    /// Whether commit `oid` is selected, alone or as part of a multi-selection.
    pub fn includes(&self, oid: Oid) -> bool {
        match self {
            Selection::Commit(o) => *o == oid,
            Selection::Commits(m) => m.contains(oid),
            _ => false,
        }
    }
}

/// A git command sequence that is running.
pub struct RunningJob {
    pub label: String,
    /// The repository the job runs in (its tab), if any.
    pub workdir: Option<PathBuf>,
    /// Background jobs (auto-fetch) don't pop the activity log open or show toasts.
    pub quiet: bool,
    pub clear_commit: bool,
    pub then_open: Option<(PathBuf, trust::Created)>,
    /// Run the push flow when this (commit) job succeeds.
    pub then_push: bool,
    /// Open the create pull request dialog for this branch once this (push) job succeeds.
    pub then_pr: Option<push_pr::PrStart>,
    /// Check the repository again for a pending link's target once this (fetch) job succeeds
    /// (update/deep_link_open.rs).
    pub then_link: bool,
    /// The job can't make the undo history wrong (fetch, push, or an undo/redo itself).
    pub keeps_undo: bool,
}

/// A dialog over the window. `Shortcuts` is the keyboard shortcut sheet, `filter` its filter
/// field. `ConflictReport` is a conflict check the user asked for (branch menus, Check again).
pub enum Modal {
    NewBranch {
        start: Option<String>,
        name: String,
        checkout: bool,
    },
    NewTag(tags::TagForm),
    AnnotateTag(tags::AnnotateForm),
    NewWorktree(worktrees::WorktreeForm),
    RemoveWorktree(worktree_manage::RemoveForm),
    LockWorktree(worktree_manage::LockForm),
    AddSubmodule(submodules::add::AddForm),
    RemoveSubmodule(submodules::remove::RemoveForm),
    Confirm {
        title: String,
        body: String,
        button: String,
        then: Box<Msg>,
    },
    Clone(clone::CloneForm),
    Init(init::InitForm),
    CreatePr {
        source: String,
        target: String,
        targets: Vec<String>,
        title: String,
        description: String,
        draft: bool,
        items: String,
    },
    Publish(Box<publish::PublishForm>),
    RenameBranch {
        old: String,
        name: String,
    },
    RenameStash(stashes::RenameStashForm),
    StashPush(stashes::StashPushForm),
    SetUpstream {
        branch: String,
        remote: String,
        name: String,
    },
    NewFile(files::PathForm),
    RenameFile {
        old: String,
        form: files::PathForm,
    },
    Shortcuts {
        filter: String,
    },
    GitFlowStart(gitflow::StartForm),
    GitFlowFinish(gitflow::FinishForm),
    Remote(remotes::RemoteForm),
    AddFork(Box<forks::ForkPicker>),
    CreateFork(Box<forks::ForkCreate>),
    Trust(trust::TrustPrompt),
    ConflictReport(conflict_check::ConflictReport),
    ForgeFilter(Box<forge_filter_form::FilterForm>),
    /// An invalid link, a linked repository not found, or several copies to choose from.
    DeepLink(deep_link::LinkDialog),
    CreateIssue(Box<issue_form::IssueForm>),
}

/// The most commits "Load more" goes up to.
pub const MAX_COMMITS: usize = 200_000;

pub struct AppState {
    pub settings: Settings,
    /// The keys in effect: the defaults with `settings.keybindings` applied.
    pub keymap: crate::keymap::Keymap,
    /// Settings as last saved, to detect edits made in Preferences.
    pub saved_settings: String,
    pub recent: Vec<PathBuf>,
    /// Recent or favorite repository folders found missing by the last check
    /// (`RecentMsg::FoldersChecked`).
    pub missing_repos: Vec<PathBuf>,
    /// The identities of listed and loaded folders, as worker threads found them.
    pub known: known_folders::KnownFolders,
    /// Open tabs; the active one's repository data is in the fields below.
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    /// Tab to go back to if opening a repository in a new tab fails.
    pub tab_return: Option<usize>,
    /// Recently closed repositories, most recent last.
    pub closed_tabs: Vec<PathBuf>,
    pub snap: Option<Snapshot>,
    /// Incremented for every repository load; stale results are ignored.
    pub load_gen: u64,
    pub loading: bool,
    pub refresh: refresh::RefreshState,
    pub selection: Selection,
    pub detail: Option<CommitDetail>,
    /// Shared with effects that read it (a reload, a highlight) instead of copied for them.
    pub diff: Option<(DiffTarget, std::sync::Arc<FileDiff>)>,
    /// Incremented for every diff request and whenever the diff on screen goes stale (closed,
    /// replaced, or changed by a hunk operation); results of older requests are ignored.
    pub diff_gen: u64,
    /// The request that produced the diff on screen. Hunk operations only run while this is
    /// `diff_gen`: otherwise a reload is on its way and the hunk indexes may no longer match.
    pub diff_shown_gen: u64,
    /// The conflict being resolved; shown instead of the graph or a diff.
    pub conflict: Option<conflict::ConflictView>,
    /// Incremented for every conflict load request and whenever the view closes; older
    /// `ConflictLoaded` results are ignored.
    pub conflict_gen: u64,
    /// The open file history; shown instead of a diff or the graph (below a conflict).
    pub history: Option<history::FileHistoryView>,
    /// Incremented for every history request and whenever the view closes; older
    /// `FileHistoryLoaded` results are ignored.
    pub history_gen: u64,
    /// The open blame view; shown above the file history, a diff and the panels, below a conflict.
    pub blame: Option<blame::BlameView>,
    /// Incremented for every blame request and whenever the view closes (stale `Loaded` results are dropped).
    pub blame_gen: u64,
    /// The open pull request panel; shown instead of a diff or the graph (below a conflict and
    /// the file history).
    pub pull_request: Option<pull_request::PrView>,
    /// Incremented for every file-list request and whenever the panel closes.
    pub pull_request_gen: u64,
    /// Incremented for every request of the panel's page and whenever the panel closes.
    pub pull_request_page_gen: u64,
    /// The open issue panel; in the pull request panel's place (the two never show together).
    pub issue: Option<issue::IssueView>,
    /// Incremented for every request of the panel's details and whenever the panel closes.
    pub issue_gen: u64,
    /// The interactive rebase panel; shown instead of the graph (below a diff, the file history and a conflict).
    pub rebase: Option<rebase::RebasePanel>,
    /// Incremented for every rebase list request and whenever the panel closes; older
    /// `RebasePlanLoaded` results are ignored.
    pub rebase_gen: u64,
    /// The Git Flow panel is open; shown in place of the graph (below the rebase panel). Parked
    /// with the tab.
    pub gitflow_panel: bool,
    pub draft: CommitDraft,
    pub job: Option<RunningJob>,
    /// The active tab's undo history.
    pub undo: undo::UndoHistory,
    pub log: Vec<LogEntry>,
    /// The log files: persistence of the activity log, the panel's tab (state/logs.rs).
    pub logs: logs::LogsState,
    pub toasts: Vec<Toast>,
    /// Last toast id handed out (state/toasts.rs).
    pub toast_seq: u64,
    /// How far behind the checked-out branch was when the active tab's auto-fetch ended, until
    /// the reload after it lands (update/notify.rs).
    pub behind_watch: Option<notify::BehindWatch>,
    pub modal: Option<Modal>,
    pub prefs: Option<PrefsState>,
    pub forge: Option<ForgeState>,
    /// Per-commit (files, added, deleted) for the CHANGES column.
    pub stats: HashMap<Oid, (usize, usize, usize)>,
    pub stats_pending: HashSet<Oid>,
    /// Signature checks of the active tab's commits (parked with the tab).
    pub signatures: signatures::Signatures,
    /// Predicted merge conflicts of the checked-out branch (parked with the tab).
    pub conflict_checks: conflict_check::ConflictChecks,
    /// The active tab's commit search; parked with the tab.
    pub search: search::CommitSearch,
    pub last_fetch: Instant,
    /// The file watcher for the active tab.
    pub watch: watch::WatchState,
    /// Credential prompts from git (global, not per tab).
    pub askpass: askpass::CredentialPrompts,
    /// Whether git-lfs is installed (global, not per tab): checked at startup and whenever Preferences > Git LFS reads its status.
    pub lfs_install: lfs::LfsInstall,
    /// The git Trellis runs and what it can do (global): checked at startup.
    pub git: git_env::GitState,
    /// The theme files in the themes folder (global): read at startup and on Reload.
    pub themes: crate::themes::ThemeState,
    /// The command palette, while open (global, not per tab).
    pub palette: Option<palette::Palette>,
    /// Incremented for every palette list request; older `TrackedPathsLoaded` results are ignored.
    pub palette_gen: u64,
    /// Plain commands recently run from the palette, most recent first (global, per session).
    pub palette_recent: palette_recent::RecentCommands,
    /// The built-in terminal panel (global, not per tab).
    pub terminal: terminal::TerminalPanel,
    /// The running Git Flow Finish job's steps (update/gitflow_finish.rs).
    pub gitflow_finish: Option<gitflow::FinishJob>,
    /// The running remote tag push or delete (update/tags.rs), for its headline.
    pub tag_job: Option<tags::RemoteTagJob>,
    /// The running Apply patch job (update/patch.rs), for its headline.
    pub patch_apply: Option<patch::PendingApply>,
    /// The trellis:// link being opened and the scheme registration (global).
    pub deep_link: deep_link::DeepLinkState,
    /// The profile a switch is writing to the global git config (app-wide: the config is
    /// global).
    pub profile_switch: Option<trellis_core::settings::ProfileId>,
    /// The profiles (the current one or the switch's target) whose fields were edited while a
    /// switch ran: their git writes wait until it ends.
    pub profile_switch_edits: Vec<trellis_core::settings::ProfileId>,
    /// Repository trust prompts asked and repositories just created, this session.
    pub trust: trust::TrustSession,
    /// The first-run welcome and the tour (global, not per tab; nothing saved but the welcome
    /// flag).
    pub onboarding: onboarding::Onboarding,
    /// The cross-repository dashboard: global, not parked with a tab.
    pub dashboard: dashboard::Dashboard,
    pub view: ViewState,
}

impl AppState {
    pub fn new(settings: Settings, recent: Vec<PathBuf>, now: Instant) -> Self {
        let saved_settings = serde_json::to_string(&settings).unwrap_or_default();
        let watch = watch::WatchState { enabled: settings.watch_files, ..Default::default() };
        let (keymap, key_problems) = crate::keymap::Keymap::from_overrides(&settings.keybindings);
        let mut st = Self {
            settings,
            keymap,
            saved_settings,
            recent,
            missing_repos: Vec::new(),
            known: Default::default(),
            tabs: vec![Tab::empty()],
            active_tab: 0,
            tab_return: None,
            closed_tabs: Vec::new(),
            snap: None,
            load_gen: 0,
            loading: false,
            refresh: Default::default(),
            selection: Selection::None,
            detail: None,
            diff: None,
            diff_gen: 0,
            diff_shown_gen: 0,
            conflict: None,
            conflict_gen: 0,
            history: None,
            history_gen: 0,
            blame: None,
            blame_gen: 0,
            pull_request: None,
            pull_request_gen: 0,
            pull_request_page_gen: 0,
            issue: None,
            issue_gen: 0,
            rebase: None,
            rebase_gen: 0,
            gitflow_panel: false,
            draft: CommitDraft::default(),
            job: None,
            undo: undo::UndoHistory::default(),
            log: Vec::new(),
            logs: Default::default(),
            toasts: Vec::new(),
            toast_seq: 0,
            behind_watch: None,
            modal: None,
            prefs: None,
            forge: None,
            stats: HashMap::new(),
            stats_pending: HashSet::new(),
            signatures: Default::default(),
            conflict_checks: Default::default(),
            search: Default::default(),
            last_fetch: now,
            watch,
            askpass: Default::default(),
            lfs_install: Default::default(),
            git: Default::default(),
            themes: Default::default(),
            palette: None,
            palette_gen: 0,
            palette_recent: Default::default(),
            terminal: Default::default(),
            gitflow_finish: None,
            tag_job: None,
            patch_apply: None,
            deep_link: Default::default(),
            profile_switch: None,
            profile_switch_edits: Vec::new(),
            trust: Default::default(),
            onboarding: Default::default(),
            dashboard: Default::default(),
            view: ViewState::default(),
        };
        for p in key_problems {
            tracing::warn!("{p}");
            st.log(false, p);
        }
        st
    }
}
