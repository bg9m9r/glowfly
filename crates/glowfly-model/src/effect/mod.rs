//! Side effects requested by `update`. The runner executes them off the UI thread and reports
//! back with `Msg`s; the few that need the window (`UiEffect`) are handled by the UI shell.

pub use crate::file_op::{DeleteMode, FileOp, SeenLines, WorkFileOk};
use git2::Oid;
use glowfly_core::askpass::Reply;
use glowfly_core::forge::{Auth, Target};
use glowfly_core::repo::HistoryStart;
use glowfly_core::settings::Settings;
use std::path::PathBuf;

mod account;
pub mod areas;
mod background;
mod blame;
mod branch;
mod clone;
mod commit;
mod conflict;
mod credential;
mod dashboard;
mod diff;
mod files;
mod forge;
mod forge_filters;
mod history;
mod hunk_patch;
mod index;
mod init;
mod issue;
mod job;
mod lfs;
mod line_counts;
mod load;
mod lock;
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
mod sandbox;
mod scope;
mod selection;
mod sparse;
mod stack;
mod stash;
mod submodule;
mod trust;
mod trust_check;
mod ui;
mod updates;
mod worktree;

pub use account::AccountEffect;
use areas::{effect_areas, effect_enum};
pub use background::{BackgroundEffect, Discard};
pub use blame::BlameRequest;
pub use branch::BranchEffect;
pub use clone::CloneEffect;
pub use commit::CommitEffect;
pub use conflict::ConflictEffect;
pub use credential::CredentialEffect;
pub use dashboard::*;
pub use diff::DiffEffect;
pub use files::FilesEffect;
pub use forge::{ForgeCall, ForgeError, ForgeReply, ForgeResult};
pub use forge_filters::{FilterCall, FilterResult};
pub use history::FileTextRequest;
pub use hunk_patch::HunkPatchRequest;
pub use index::{IndexEffect, Shared};
pub use init::InitEffect;
pub use issue::*;
pub use job::JobEffect;
pub use lfs::LfsEffect;
pub use line_counts::{FailedLineCounts, LineCountsEffect};
pub use load::LoadEffect;
pub use lock::LockEffect;
pub use meta::EffectMeta;
pub use meta::SandboxRule;
pub use monitor::{MonitorJob, MonitorOff};
pub use notify::{DesktopNote, NoteKind};
pub use op_preview::OpPreviewJob;
pub use os::OsEffect;
pub use pick::PickJob;
pub use prefs::{ConfigWrite, PrefsEffect};
pub use rebase::{RebaseEffect, RestackJob};
pub use repo::{
    ConflictRead, IndexRequest, KnownRepo, LoadKind, LocalOp, StashHow, StatusRead, UndoOpts, UndoResult, UndoStash, WatchTarget,
};
pub use repos::ReposEffect;
pub use selection::{SelectionEffect, StackRequest};
pub use sparse::SparseEffect;
pub use stack::{StackEffect, StackPushJob};
pub use stash::{PutBackRequest, StashEffect};
pub use submodule::SubmoduleEffect;
pub use trust::TrustEffect;
pub use trust_check::{Accepted, Checked, Seal, Sealed, TrustCheck, WaitingKind};
pub use ui::UiEffect;
pub use updates::{RestartJob, UpdateEffect};
pub use worktree::WorktreeEffect;

// The areas' variants come from the table in areas.rs, each with its own effect type that decides
// its metadata (`EffectMeta`). The flat variants below predate the areas: a new effect goes in an
// area (a new variant of its type, or a new row in areas.rs), never here.
effect_areas! { effect_enum! {
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
    /// The commits that changed `path` (anything under it with `folder`), examining at most `limit`.
    LoadFileHistory {
        workdir: PathBuf,
        path: String,
        start: HistoryStart,
        limit: usize,
        folder: bool,
        generation: u64,
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
