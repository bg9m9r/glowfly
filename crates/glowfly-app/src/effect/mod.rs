//! Side effects requested by `update`. The runner executes them off the UI thread and reports
//! back with `Msg`s; the few that need the window (`UiEffect`) are handled by the UI shell.

pub use crate::file_op::{DeleteMode, FileOp, WorkFileOk};
use git2::Oid;
use glowfly_core::askpass::Reply;
use glowfly_core::forge::{Auth, Target};
use glowfly_core::repo::{DiffTarget, FileDiff, HistoryStart, LoadOptions};
use glowfly_core::settings::Settings;
use glowfly_core::undo::{Action, UndoKind};
use std::path::PathBuf;

mod account;
pub(crate) mod areas;
mod background;
mod blame;
mod clone;
mod conflict;
mod credential;
mod dashboard;
mod forge;
mod forge_filters;
mod history;
mod index;
mod init;
mod issue;
mod lfs;
mod line_counts;
mod meta;
mod monitor;
mod names;
mod notify;
mod op_preview;
mod os;
pub mod partial;
mod pick;
mod prefs;
mod rebase;
mod repo;
mod repos;
mod scope;
mod selection;
mod sparse;
mod stack;
mod stash;
mod submodule;
mod trust;
mod ui;
mod updates;
mod worktree;

pub use account::AccountEffect;
use areas::{effect_areas, effect_enum};
pub use background::{BackgroundEffect, Discard};
pub use blame::BlameRequest;
pub use clone::CloneEffect;
pub use conflict::ConflictEffect;
pub use credential::CredentialEffect;
pub use dashboard::*;
pub use forge::{ForgeCall, ForgeError, ForgeReply, ForgeResult};
pub use forge_filters::{FilterCall, FilterResult};
pub use history::FileTextRequest;
pub use index::{IndexEffect, Shared};
pub use init::InitEffect;
pub use issue::*;
pub use lfs::LfsEffect;
pub use line_counts::{FailedLineCounts, LineCountsEffect};
pub use meta::EffectMeta;
#[cfg(test)]
pub use meta::SandboxRule;
pub use monitor::MonitorJob;
pub use notify::{DesktopNote, NoteKind};
pub use op_preview::OpPreviewJob;
pub use os::OsEffect;
pub use pick::PickJob;
pub use prefs::{ConfigWrite, PrefsEffect};
pub use rebase::{RebaseEffect, RestackJob};
pub use repo::{ConflictRead, IndexRequest, KnownRepo, LoadKind, LocalOp, StatusRead, UndoOpts, UndoResult, WatchTarget};
pub use repos::ReposEffect;
pub use selection::{SelectionEffect, StackRequest};
pub use sparse::SparseEffect;
pub use stack::{StackEffect, StackPushJob};
pub use stash::{PutBackRequest, StashEffect};
pub use submodule::SubmoduleEffect;
pub use trust::TrustEffect;
pub use ui::UiEffect;
pub use updates::{RestartJob, UpdateEffect};
pub use worktree::WorktreeEffect;

// The areas' variants come from the table in areas.rs, each with its own effect type that decides
// its metadata (`EffectMeta`). The flat variants below predate the areas: a new effect goes in an
// area (a new variant of its type, or a new row in areas.rs), never here.
effect_areas! { effect_enum! {
    LoadRepo {
        path: PathBuf,
        opts: LoadOptions,
        generation: u64,
        kind: LoadKind,
        /// The snapshot on screen: when its `refs_key` still matches, only the status is read (`LoadMsg::RepoUnchanged`).
        known: Option<KnownRepo>,
        /// The history on screen: commits a full load takes instead of reading them again, and for `More` the walk's frontier.
        prior: Option<glowfly_core::repo::Prior>,
    },
    LoadStatus {
        workdir: PathBuf,
        generation: u64,
        /// The last load found submodules: read them again even without a .gitmodules.
        submodules: bool,
        /// The file whose working-tree or index diff is open, for `LoadMsg::StatusLoaded::open_stamp`.
        open: Option<String>,
        /// Let git read the status (fsmonitor, untracked cache); false while the repository isn't trusted.
        git_status: bool,
        /// Re-read only these work-tree paths' unstaged rows; None reads everything.
        paths: Option<Vec<String>>,
    },
    /// After an index change: both lists' rows of the changed paths, else the whole status.
    LoadIndexStatus {
        workdir: PathBuf,
        generation: u64,
        submodules: bool,
        open: Option<String>,
        git_status: bool,
        read: IndexRequest,
    },
    LoadDetail {
        workdir: PathBuf,
        oid: Oid,
    },
    LoadDiff {
        workdir: PathBuf,
        target: DiffTarget,
        reload: bool,
        generation: u64,
        /// On a reload, the diff on screen: unchanged hunks keep its highlighting.
        previous: Option<std::sync::Arc<FileDiff>>,
        /// Hide whitespace-only changes (the diff_ignore_whitespace setting when the request was made).
        ignore_whitespace: bool,
        /// Also load the whole file (the diff_mode setting when the request was made).
        whole_file: bool,
    },
    /// Finishes the syntax highlighting `diff` arrived with (pending in its hunks or its whole
    /// file) for the diff shown at `generation` (DiffSlot::shown_gen). Started by
    /// Runner::run with a cancel ticket: a newer HighlightDiff cancels this one.
    HighlightDiff {
        workdir: PathBuf,
        target: DiffTarget,
        generation: u64,
        diff: std::sync::Arc<FileDiff>,
    },
    /// Reads the unchanged lines between `diff`'s hunks for the hunk view, for the diff shown at
    /// `generation` (DiffSlot::shown_gen).
    LoadDiffGaps {
        workdir: PathBuf,
        target: DiffTarget,
        generation: u64,
        diff: std::sync::Arc<FileDiff>,
    },
    /// Reads the sizes and decodes the images of the binary diff shown at `generation`
    /// (DiffSlot::shown_gen), for its header and preview.
    LoadBinarySides {
        workdir: PathBuf,
        target: DiffTarget,
        generation: u64,
    },
    LocalOp {
        workdir: PathBuf,
        op: LocalOp,
    },
    /// Reverts part of a commit or comparison diff (a hunk, some of its lines, or the whole
    /// file) in the working tree only (glowfly_core::repo::revert).
    Revert {
        workdir: PathBuf,
        req: glowfly_core::repo::RevertRequest,
    },
    /// Writes `path` from commit `oid` (or its parent) into the working tree only.
    /// Without `overwrite`, refuses when the file's current contents would be lost.
    RestoreFile {
        workdir: PathBuf,
        oid: Oid,
        path: String,
        overwrite: bool,
    },
    /// Creates, renames, deletes or discards (untracked) a working-tree file; the index is untouched, except for
    /// UnstageDelete, UnstageGone (index only) and a staged rename.
    WorkFile {
        workdir: PathBuf,
        op: FileOp,
    },
    /// The files in the index, for the palette's file finder.
    LoadTrackedPaths {
        workdir: PathBuf,
        generation: u64,
    },
    /// The commits that changed `path` (anything under it with `folder`), examining at most `limit`.
    LoadFileHistory {
        workdir: PathBuf,
        path: String,
        start: HistoryStart,
        limit: usize,
        folder: bool,
        generation: u64,
    },
    /// Runs git commands in order, stopping at the first failure. `write_file` is written first
    /// (used for commit messages). With `record`, the state before and after is compared and
    /// the resulting undoable action reported back; with `journal`, a journal note is written
    /// when the run moved HEAD or the op's branch (runner/journal.rs). Without `prompts`
    /// (background jobs) git fails rather than open a credential prompt.
    RunGit {
        cwd: PathBuf,
        cmds: Vec<Vec<String>>,
        write_file: Option<(PathBuf, String)>,
        record: Option<UndoKind>,
        journal: Option<glowfly_core::journal::Op>,
        prompts: bool,
        /// Run nothing unless this revision still resolves to this commit.
        expect: Option<(String, git2::Oid)>,
        /// Every command only fetches: `Effect::StopGit(StopKey::Job)` can stop it.
        stoppable: bool,
    },
    /// Undoes (or redoes) `action` after checking the repository still matches it. Unscoped like
    /// `RunGit`: the job's result must reach `update` even if its tab is no longer active.
    UndoRedo {
        cwd: PathBuf,
        action: Action,
        redo: bool,
        opts: UndoOpts,
    },
    Forge {
        target: Target,
        cwd: PathBuf,
        auth: Option<Auth>,
        interactive: bool,
        /// The user connected this account in Preferences (lets GCM sign in to a self-hosted server).
        connected: bool,
        call: ForgeCall,
    },
    Stats {
        workdir: PathBuf,
        oids: Vec<Oid>,
    },
    /// Which of `oids` (newest first) changed a path matching `terms`, for search request
    /// `request`; reported in chunks as SearchMsg::PathsSearched. Handled by Runner::run's
    /// path-search worker: a newer request replaces one waiting and stops one running.
    SearchPaths {
        workdir: PathBuf,
        request: u64,
        terms: Vec<glowfly_core::path_query::PathTerm>,
        oids: Vec<Oid>,
    },
    /// Checks commit `oid`'s signature (in `format`) with git, for the details panel's badge.
    VerifySignature {
        workdir: PathBuf,
        oid: Oid,
        format: glowfly_core::signing::SigningFormat,
        generation: u64,
    },
    /// Downloads (or reads from the disk cache) avatars one after another.
    FetchAvatars(Vec<String>),
    SaveSettings(Box<Settings>),
    SaveRecent(Vec<PathBuf>),
    /// Which of these repository folders are gone (not a directory or unreachable), and the
    /// identity of each that is there; reported as `RecentMsg::FoldersChecked`.
    CheckRepoFolders(Vec<PathBuf>),
    /// The common dir of each restored tab's folder, without loading it; reported as
    /// `TabsMsg::GroupsRead`.
    ReadTabGroups(Vec<PathBuf>),
    /// The open tabs (repositories), their labels (parallel to `paths`) and which one is
    /// active, for the next start.
    SaveTabs {
        paths: Vec<PathBuf>,
        labels: Vec<Option<String>>,
        active: usize,
    },
    /// Watch this repository for outside changes, replacing any previous watch; None stops watching. Handled by
    /// `Runner::run` itself (the watcher outlives a worker thread).
    Watch(Option<WatchTarget>),
    /// Check the watched tree for changes the watcher may have missed (Windows poll); `Runner::run`.
    SweepWatch {
        workdir: PathBuf,
    },
    /// Answers credential prompt `id`. Handled by `Runner::run` itself, which owns the listener the helper is waiting on.
    /// The answer's `Debug` output is redacted.
    AnswerPrompt {
        id: u64,
        reply: Reply,
    },
    /// Stops the running git command registered under this key, if any. Handled by `Runner::run` itself, which holds the stops.
    StopGit(partial::StopKey),
} }
