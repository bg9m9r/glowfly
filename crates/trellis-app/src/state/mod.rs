//! Application state: plain data, no egui types. Only `update` changes it, except for
//! transient view state (`ViewState`) and text buffers that views edit in place.

pub mod askpass;
pub mod branches;
pub mod commit;
pub mod conflict;
pub mod diff_nav;
pub mod files;
mod forge;
pub mod history;
pub mod lines;
pub mod palette;
pub mod palette_commands;
pub mod palette_kinds;
pub mod prefs;
pub mod publish;
pub mod rebase;
pub mod selection;
pub mod tabs;
pub mod tags;
pub mod terminal;
pub mod undo;
pub mod watch;

pub use commit::CommitDraft;
pub use forge::{ForgeState, Status};
pub use prefs::{PrefPage, PrefsState, host_text};
pub use tabs::Tab;

use crate::msg::Msg;
use git2::Oid;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use trellis_core::repo::{CommitDetail, DiffTarget, FileDiff, LoadOptions, Snapshot};
use trellis_core::settings::{BranchVisibility, DiffMode, Settings};

/// Background refresh of pull requests and issues (GitHub allows 5,000 API calls an hour).
pub const FORGE_REFRESH_INTERVAL: Duration = Duration::from_secs(300);
pub const TOAST_LIFETIME: Duration = Duration::from_secs(5);
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
    pub then_open: Option<PathBuf>,
    /// Run the push flow when this (commit) job succeeds.
    pub then_push: bool,
    /// The job can't make the undo history wrong (fetch, push, or an undo/redo itself).
    pub keeps_undo: bool,
}

pub struct LogEntry {
    pub ok: bool,
    pub text: String,
}

pub struct Toast {
    pub ok: bool,
    pub text: String,
    pub at: Instant,
}

/// A dialog over the window. `Shortcuts` is the keyboard shortcut sheet, `filter` its filter
/// field.
pub enum Modal {
    NewBranch { start: Option<String>, name: String, checkout: bool },
    NewTag(tags::TagForm),
    Confirm { title: String, body: String, button: String, then: Box<Msg> },
    Clone { url: String, parent: String },
    CreatePr { source: String, target: String, targets: Vec<String>, title: String, description: String, draft: bool, items: String },
    Publish(Box<publish::PublishForm>),
    RenameBranch { old: String, name: String },
    SetUpstream { branch: String, remote: String, name: String },
    NewFile(files::PathForm),
    RenameFile { old: String, form: files::PathForm },
    Shortcuts { filter: String },
}

pub struct Sections {
    pub local: bool,
    pub remote: bool,
    pub prs: bool,
    pub issues: bool,
    pub tags: bool,
    pub stashes: bool,
}

/// Purely visual state that views may change directly.
pub struct ViewState {
    pub show_log: bool,
    pub show_search: bool,
    pub search: String,
    pub search_cursor: usize,
    pub ref_filter: String,
    pub sections: Sections,
    /// Scroll the graph so this row is near the top on the next frame (set by `update`).
    pub scroll_to_row: Option<usize>,
    /// Scroll the graph to this pixel offset on the next frame (keyboard navigation).
    pub scroll_offset: Option<f32>,
    /// Graph scroll offset and viewport height from the last frame.
    pub graph_scroll: (f32, f32),
    /// First diff row visible in the last frame, recorded by the view.
    pub diff_top_row: usize,
    /// Scroll the diff so this row is at the top on the next frame (set by `update`).
    pub diff_scroll_to_row: Option<usize>,
    /// Put this hunk at the top of the next freshly opened diff (its header row, or its change
    /// block in the whole-file view); set when the view mode changes.
    pub diff_open_hunk: Option<usize>,
    /// A file opened by next/previous change, still loading, and the change to land on. Set
    /// and cleared only by `update`.
    pub diff_landing: Option<(DiffTarget, diff_nav::Landing)>,
    /// The change row next/previous change last moved to. Steps continue from it while it is
    /// on screen, because a change near the end can't scroll to the top.
    pub diff_cursor: Option<usize>,
    /// The folds opened in the whole-file view of the open diff, by key. Changed only by
    /// `update`; cleared when a different target's diff arrives or the diff closes; parked with
    /// its tab.
    pub diff_expanded: trellis_core::whole_file::Expanded,
    /// How many diff rows fit in the viewport, recorded by the view.
    pub diff_visible_rows: usize,
    /// Branch tip under the pointer (sidebar or BRANCH/TAG column), for row highlighting.
    pub hover_tip: Option<Oid>,
    pub hover_tip_next: Option<Oid>,
    pub highlight: Option<(Oid, HashSet<Oid>)>,
    pub base_set: Option<HashSet<Oid>>,
    pub was_focused: bool,
    /// A text field had the keyboard when the last frame was drawn. egui takes the focus
    /// away when Esc is pressed before the shell sees the key, so Esc checks this instead.
    pub was_typing: bool,
    /// Wall-clock time (Unix seconds) for this frame, set by the shell; fixed in tests.
    pub now: i64,
    /// UTC offset (seconds) used to display times; the local zone in the app, fixed in tests.
    pub utc_offset: i32,
    /// Fetched avatars waiting for the shell to turn them into textures.
    pub avatars_loaded: Vec<(String, Option<trellis_core::avatars::Image>)>,
    /// Lines selected in the open diff for line-level staging (changed only by `update`).
    pub diff_lines: lines::LineSelection,
    /// Scroll the conflict panes and output to this chunk on the next frame (set by `update`).
    pub conflict_scroll_to: Option<usize>,
    /// Scroll the file history list to this entry on the next frame (set by `update`).
    pub history_scroll_to: Option<usize>,
    /// Focus the graph search field on the next frame (set by update).
    pub focus_search: bool,
    /// Focus the commit summary field on the next frame (set by update, taken by the commit box).
    pub focus_message: bool,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            show_log: false,
            show_search: false,
            search: String::new(),
            search_cursor: 0,
            ref_filter: String::new(),
            sections: Sections { local: true, remote: true, prs: true, issues: true, tags: false, stashes: true },
            scroll_to_row: None,
            scroll_offset: None,
            graph_scroll: (0.0, 0.0),
            diff_top_row: 0,
            diff_scroll_to_row: None,
            diff_open_hunk: None,
            diff_landing: None,
            diff_cursor: None,
            diff_expanded: Default::default(),
            diff_visible_rows: 0,
            hover_tip: None,
            hover_tip_next: None,
            highlight: None,
            base_set: None,
            was_focused: true,
            was_typing: false,
            now: 0,
            utc_offset: 0,
            avatars_loaded: Vec::new(),
            diff_lines: lines::LineSelection::default(),
            conflict_scroll_to: None,
            history_scroll_to: None,
            focus_search: false,
            focus_message: false,
        }
    }
}

pub struct AppState {
    pub settings: Settings,
    /// Settings as last saved, to detect edits made in Preferences.
    pub saved_settings: String,
    pub recent: Vec<PathBuf>,
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
    pub selection: Selection,
    pub detail: Option<CommitDetail>,
    pub diff: Option<(DiffTarget, FileDiff)>,
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
    /// The interactive rebase panel; shown instead of the graph (below a diff, the file
    /// history and a conflict).
    pub rebase: Option<rebase::RebasePanel>,
    /// Incremented for every rebase list request and whenever the panel closes; older
    /// `RebasePlanLoaded` results are ignored.
    pub rebase_gen: u64,
    pub draft: CommitDraft,
    pub job: Option<RunningJob>,
    /// The active tab's undo history.
    pub undo: undo::UndoHistory,
    pub log: Vec<LogEntry>,
    pub toasts: Vec<Toast>,
    pub modal: Option<Modal>,
    pub prefs: Option<PrefsState>,
    pub forge: Option<ForgeState>,
    /// Per-commit (files, added, deleted) for the CHANGES column.
    pub stats: HashMap<Oid, (usize, usize, usize)>,
    pub stats_pending: HashSet<Oid>,
    pub last_fetch: Instant,
    /// The file watcher for the active tab.
    pub watch: watch::WatchState,
    /// Credential prompts from git (global, not per tab).
    pub askpass: askpass::CredentialPrompts,
    /// The command palette, while open (global, not per tab).
    pub palette: Option<palette::Palette>,
    /// Incremented for every palette list request; older `TrackedPathsLoaded` results are
    /// ignored.
    pub palette_gen: u64,
    /// The built-in terminal panel (global, not per tab).
    pub terminal: terminal::TerminalPanel,
    pub view: ViewState,
}

impl AppState {
    pub fn new(settings: Settings, recent: Vec<PathBuf>, now: Instant) -> Self {
        let saved_settings = serde_json::to_string(&settings).unwrap_or_default();
        let watch = watch::WatchState { enabled: settings.watch_files, ..Default::default() };
        Self {
            settings,
            saved_settings,
            recent,
            tabs: vec![Tab::empty()],
            active_tab: 0,
            tab_return: None,
            closed_tabs: Vec::new(),
            snap: None,
            load_gen: 0,
            loading: false,
            selection: Selection::None,
            detail: None,
            diff: None,
            diff_gen: 0,
            diff_shown_gen: 0,
            conflict: None,
            conflict_gen: 0,
            history: None,
            history_gen: 0,
            rebase: None,
            rebase_gen: 0,
            draft: CommitDraft::default(),
            job: None,
            undo: undo::UndoHistory::default(),
            log: Vec::new(),
            toasts: Vec::new(),
            modal: None,
            prefs: None,
            forge: None,
            stats: HashMap::new(),
            stats_pending: HashSet::new(),
            last_fetch: now,
            watch,
            askpass: Default::default(),
            palette: None,
            palette_gen: 0,
            terminal: Default::default(),
            view: ViewState::default(),
        }
    }

    /// The generation of the diff on screen when its hunk indexes are current, i.e. no reload
    /// or newer diff is on its way. Hunk operations must carry it.
    pub fn hunk_ops_gen(&self) -> Option<u64> {
        (self.diff.is_some() && self.diff_shown_gen == self.diff_gen).then_some(self.diff_gen)
    }

    /// Why hunk operations on the open diff (stage/unstage/discard on a working-tree diff,
    /// revert on a commit or comparison diff) are refused, apart from a pending reload: they
    /// count hunks of the unfiltered diff, so with whitespace hidden the shown hunks don't
    /// match. Every change to the setting reopens the diff (a new `diff_gen`), so a diff loaded
    /// under the other setting never has `hunk_ops_gen` set.
    pub fn hunk_ops_blocked(&self) -> Option<&'static str> {
        let (target, _) = self.diff.as_ref()?;
        let why = if target.is_working_tree() { WHITESPACE_BLOCKS_HUNK_OPS } else { WHITESPACE_BLOCKS_REVERT };
        self.settings.diff_ignore_whitespace.then_some(why)
    }

    /// Why selecting and applying single lines is refused: whatever blocks hunk operations, or
    /// the whole-file view.
    pub fn line_ops_blocked(&self) -> Option<&'static str> {
        self.hunk_ops_blocked().or((self.settings.diff_mode == DiffMode::WholeFile).then_some(WHOLE_FILE_BLOCKS_LINE_OPS))
    }

    pub fn workdir(&self) -> Option<PathBuf> {
        self.snap.as_ref().map(|s| s.workdir.clone())
    }

    /// Working directory for commands that don't need a repository (git config, sign-in).
    pub fn any_dir(&self) -> PathBuf {
        self.workdir().unwrap_or_else(std::env::temp_dir)
    }

    /// How to load the repository at `workdir`, with the hidden and soloed refs saved for it.
    pub fn load_options(&self, workdir: &Path) -> LoadOptions {
        let smart = self.settings.branch_visibility == BranchVisibility::Smart;
        // Smart visibility also keeps branches that have open pull requests.
        let extra_tips = self.forge.as_ref().map(|f| f.prs.iter().map(|p| p.source.clone()).collect()).unwrap_or_default();
        let f = self.settings.graph_filter_for(workdir);
        LoadOptions { max_commits: self.settings.commit_limit(), smart, extra_tips, hidden: f.hidden, solo: f.solo }
    }

    pub fn log(&mut self, ok: bool, text: impl Into<String>) {
        self.log.push(LogEntry { ok, text: text.into() });
        if !ok {
            self.view.show_log = true;
        }
    }

    pub fn toast(&mut self, ok: bool, text: impl Into<String>, now: Instant) {
        self.toasts.push(Toast { ok, text: text.into(), at: now });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }
}
