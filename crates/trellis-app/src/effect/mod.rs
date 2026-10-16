//! Side effects requested by `update`. The runner executes them off the UI thread and reports
//! back with `Msg`s; the few that need the window (`UiEffect`) are handled by the UI shell.

pub use crate::file_op::{DeleteMode, FileOp, WorkFileOk};
use crate::msg::{FilePurpose, FolderPurpose};
use git2::Oid;
use std::path::PathBuf;
use trellis_core::askpass::Reply;
use trellis_core::forge::{Account, Auth, NewRepo, Target};
use trellis_core::rebase::{RebaseBase, StartRequest};
use trellis_core::repo::{DiffTarget, FileDiff, HistoryStart, Hunk, LoadOptions};
use trellis_core::settings::Settings;
use trellis_core::undo::{Action, UndoKind};

mod blame;
mod clone;
mod forge;
mod init;
mod names;
mod notify;
mod repo;

pub use blame::BlameRequest;
pub use clone::CloneEffect;
pub use forge::{ForgeCall, ForgeError, ForgeReply, ForgeResult};
pub use init::InitEffect;
pub use notify::{DesktopNote, NoteKind};
pub use repo::{KnownRepo, LocalOp, UndoResult, WatchTarget};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LoadKind {
    Open,
    Refresh,
}

#[derive(Debug)]
pub enum Effect {
    LoadRepo {
        path: PathBuf,
        opts: LoadOptions,
        generation: u64,
        kind: LoadKind,
        /// The snapshot on screen: when its `refs_key` still matches, only the status is read
        /// (`LoadMsg::RepoUnchanged`).
        known: Option<KnownRepo>,
    },
    LoadStatus {
        workdir: PathBuf,
        generation: u64,
        /// The last load found submodules: read them again even without a .gitmodules.
        submodules: bool,
        /// The file whose working-tree or index diff is open, for `LoadMsg::StatusLoaded::open_stamp`.
        open: Option<String>,
        /// Let git read the status (fsmonitor, untracked cache); false while the repository
        /// isn't trusted.
        git_status: bool,
    },
    LoadDetail {
        workdir: PathBuf,
        oid: Oid,
    },
    /// The files that differ between two commits (older `from`, newer `to`).
    LoadCompare {
        workdir: PathBuf,
        from: Oid,
        to: Oid,
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
    /// file) for the diff shown at `generation` (AppState::diff_shown_gen). Started by
    /// Runner::run with a cancel ticket: a newer HighlightDiff cancels this one.
    HighlightDiff {
        workdir: PathBuf,
        target: DiffTarget,
        generation: u64,
        diff: std::sync::Arc<FileDiff>,
    },
    LoadConflict {
        workdir: PathBuf,
        path: String,
        generation: u64,
    },
    LocalOp {
        workdir: PathBuf,
        op: LocalOp,
    },
    /// Applies the inverse of hunk `index` of `target` (a commit or comparison diff) to the
    /// working tree only; refused if git's hunk is no longer `expected`.
    RevertHunk {
        workdir: PathBuf,
        target: DiffTarget,
        index: usize,
        expected: Hunk,
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
    /// Reads the submodule at `path` (repo-relative) again, with the commits an update would
    /// leave behind, before confirming the update.
    CheckSubmodule {
        workdir: PathBuf,
        path: String,
    },
    /// Checks the folder of a worktree about to be created (missing, or an empty folder) before
    /// `git worktree add` runs. Unscoped: the dialog waits for the answer even if tabs change.
    CheckWorktreeFolder(crate::worktree::NewWorktree),
    /// Whether the Clone dialog's parent folder `parent` exists, checked after `delay` so typing
    /// doesn't wait on the disk. Reported as `CloneMsg::ParentChecked`.
    CheckCloneParent {
        parent: String,
        delay: std::time::Duration,
    },
    /// Whether git-lfs is installed (`git lfs version`). Unscoped: the answer is global.
    DetectLfs {
        cwd: PathBuf,
    },
    /// Runs `git worktree remove` then, if asked, `git branch -d`. Unscoped like RunGit: it ends
    /// the active job.
    RemoveWorktree(crate::worktree::RemoveRequest),
    /// Renames the stash list entry stash@{index} (only if it still is commit `oid`) to
    /// `message`, keeping its position. Unscoped like RunGit: it ends the active job.
    RenameStash {
        cwd: PathBuf,
        index: usize,
        oid: Oid,
        message: String,
    },
    /// Git LFS status for the preferences page: installed version, set up or not, tracked patterns.
    LoadLfs {
        workdir: PathBuf,
        generation: u64,
    },
    /// Sparse checkout state for the preferences page.
    LoadSparse {
        workdir: PathBuf,
        generation: u64,
    },
    /// Runs `git sparse-checkout` set, reapply or disable. Unscoped like RunGit: it ends the
    /// active job.
    SparseCheckout {
        cwd: PathBuf,
        command: trellis_core::sparse_ops::SparseCommand,
    },
    /// The commits that changed `path`, examining at most `limit` commits.
    LoadFileHistory {
        workdir: PathBuf,
        path: String,
        start: HistoryStart,
        limit: usize,
        generation: u64,
    },
    /// Blames one file (`git blame`), reported as `BlameMsg::Loaded`.
    LoadBlame(BlameRequest),
    /// Runs git commands in order, stopping at the first failure. `write_file` is written first
    /// (used for commit messages). With `record`, the state before and after is compared and
    /// the resulting undoable action reported back. Without `prompts` (background jobs) git
    /// fails rather than open a credential prompt.
    RunGit {
        cwd: PathBuf,
        cmds: Vec<Vec<String>>,
        write_file: Option<(PathBuf, String)>,
        record: Option<UndoKind>,
        prompts: bool,
        /// Run nothing unless this revision still resolves to this commit.
        expect: Option<(String, git2::Oid)>,
    },
    /// The commits an interactive rebase from `base` would replay.
    LoadRebasePlan {
        workdir: PathBuf,
        base: RebaseBase,
        generation: u64,
    },
    /// Runs an interactive rebase with a prepared todo list. Unscoped like `RunGit`: the job's
    /// result must reach `update` even if its tab is no longer active.
    InteractiveRebase {
        cwd: PathBuf,
        request: StartRequest,
    },
    /// Undoes (or redoes) `action` after checking the repository still matches it. Unscoped like
    /// `RunGit`: the job's result must reach `update` even if its tab is no longer active.
    UndoRedo {
        cwd: PathBuf,
        action: Action,
        redo: bool,
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
    /// Where the signed-in user can create repositories (their login, then organizations).
    RepoOwners {
        account: Account,
        cwd: PathBuf,
    },
    /// Creates a repository for `workdir`; the result carries what is needed to push to it.
    CreateRepo {
        account: Account,
        cwd: PathBuf,
        user: String,
        repo: NewRepo,
        branch: String,
    },
    Connect {
        account: Account,
        user: Option<String>,
        token: Option<String>,
        cwd: PathBuf,
    },
    SaveToken {
        account: Account,
        user: Option<String>,
        token: String,
    },
    DeleteToken(Account),
    ForgetCredentialManager {
        accounts: Vec<Account>,
        cwd: PathBuf,
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
        terms: Vec<trellis_core::path_query::PathTerm>,
        oids: Vec<Oid>,
    },
    /// Checks commit `oid`'s signature (in `format`) with git, for the details panel's badge.
    VerifySignature {
        workdir: PathBuf,
        oid: Oid,
        format: trellis_core::signing::SigningFormat,
        generation: u64,
    },
    /// Downloads (or reads from the disk cache) avatars one after another.
    FetchAvatars(Vec<String>),
    ReadGitConfig {
        cwd: PathBuf,
        has_repo: bool,
    },
    SetGitConfig {
        cwd: PathBuf,
        global: bool,
        key: String,
        value: Option<String>,
    },
    SaveSettings(Box<Settings>),
    SaveRecent(Vec<PathBuf>),
    /// Which of these repository folders are gone (not a directory or unreachable), and the
    /// identity of each that is there; reported as `RecentMsg::FoldersChecked`.
    CheckRepoFolders(Vec<PathBuf>),
    /// The open tabs (repositories) and which one is active, for the next start.
    SaveTabs {
        paths: Vec<PathBuf>,
        active: usize,
    },
    OpenUrl(String),
    OpenTerminal(PathBuf),
    OpenFolder(PathBuf),
    /// Opens the working-tree file `path` (repo-relative) of `workdir` in the default program.
    /// Unless `confirmed`, a file the system would run instead of show is reported back
    /// (`FilesMsg::OpenNeedsConfirm`) rather than opened.
    OpenPath {
        workdir: PathBuf,
        path: String,
        confirmed: bool,
    },
    /// Shows an absolute path selected in the file manager.
    RevealPath(PathBuf),
    PickFolder {
        title: String,
        purpose: FolderPurpose,
    },
    PickFile {
        title: String,
        purpose: FilePurpose,
    },
    ListSigningKeys {
        format: trellis_core::signing::SigningFormat,
        program: String,
    },
    GenerateGpgKey {
        program: String,
        name: String,
        email: String,
        passphrase: String,
    },
    ExportPublicKey {
        format: trellis_core::signing::SigningFormat,
        program: String,
        key: String,
    },
    /// Watch this repository for outside changes, replacing any previous watch; None stops
    /// watching. Handled by `Runner::run` itself (the watcher outlives a worker thread).
    Watch(Option<WatchTarget>),
    /// Answers credential prompt `id`. Handled by `Runner::run` itself, which owns the
    /// listener the helper is waiting on. The answer's `Debug` output is redacted.
    AnswerPrompt {
        id: u64,
        reply: Reply,
    },
    /// Listing and generating SSH keys (ssh.rs); unscoped, outside any repository.
    Ssh(crate::ssh::SshEffect),
    /// Writing a profile's identity to the global git config (profile.rs); unscoped.
    Profile(crate::profile::ProfileEffect),
    /// The built-in terminal panel's shells; handled by Runner::run, never a worker.
    Terminal(crate::terminal::TerminalEffect),
    /// Patch dialogs and saving commits as a patch (patch.rs); unscoped, each carries its repository.
    Patch(crate::patch::PatchEffect),
    /// Launching the external editor or a custom terminal, and their program pickers (tools.rs); unscoped.
    Tool(crate::tools::ToolEffect),
    /// Finding git and its version, and adding a folder to safe.directory (git_env.rs); unscoped.
    GitEnv(crate::git_env::GitEnvEffect),
    /// Dry-runs a merge with git merge-tree (conflict_check.rs); reads only.
    CheckConflicts(crate::conflict_check::CheckJob),
    /// Fork lists and forking for the REMOTE section (remote.rs); unscoped.
    Remote(crate::remote::RemoteEffect),
    /// A sparse clone and listing the user's repositories for the Clone dialog; unscoped.
    Clone(CloneEffect),
    /// The Init dialog's folder checks and the init job; unscoped.
    Init(InitEffect),
    /// A desktop notification (runner/notify.rs); unscoped.
    Notify(DesktopNote),
    Ui(UiEffect),
}

impl Effect {
    /// The repository whose tab a result belongs to: results for a tab that is no longer
    /// active are dropped (the tab reloads when it is selected again).
    pub fn scope(&self) -> Option<&PathBuf> {
        match self {
            Effect::LoadStatus { workdir, .. }
            | Effect::LoadDetail { workdir, .. }
            | Effect::LoadCompare { workdir, .. }
            | Effect::LoadDiff { workdir, .. }
            | Effect::HighlightDiff { workdir, .. }
            | Effect::LoadConflict { workdir, .. }
            | Effect::LoadFileHistory { workdir, .. }
            | Effect::LoadTrackedPaths { workdir, .. }
            | Effect::CheckSubmodule { workdir, .. }
            | Effect::LoadLfs { workdir, .. }
            | Effect::LoadSparse { workdir, .. }
            | Effect::LoadRebasePlan { workdir, .. }
            | Effect::LocalOp { workdir, .. }
            | Effect::RevertHunk { workdir, .. }
            | Effect::RestoreFile { workdir, .. }
            | Effect::WorkFile { workdir, .. }
            | Effect::Stats { workdir, .. }
            | Effect::SearchPaths { workdir, .. }
            | Effect::VerifySignature { workdir, .. } => Some(workdir),
            Effect::Forge { cwd, .. } => Some(cwd),
            Effect::CheckConflicts(j) => Some(&j.workdir),
            Effect::LoadBlame(r) => Some(&r.workdir),
            // Unscoped: global answers, jobs whose result must reach `update` whichever tab is
            // active, and work outside any repository.
            Effect::LoadRepo { .. }
            | Effect::CheckWorktreeFolder(_)
            | Effect::CheckCloneParent { .. }
            | Effect::DetectLfs { .. }
            | Effect::RemoveWorktree(_)
            | Effect::RenameStash { .. }
            | Effect::SparseCheckout { .. }
            | Effect::RunGit { .. }
            | Effect::UndoRedo { .. }
            | Effect::InteractiveRebase { .. }
            | Effect::RepoOwners { .. }
            | Effect::CreateRepo { .. }
            | Effect::Connect { .. }
            | Effect::SaveToken { .. }
            | Effect::DeleteToken(_)
            | Effect::ForgetCredentialManager { .. }
            | Effect::FetchAvatars(_)
            | Effect::ReadGitConfig { .. }
            | Effect::SetGitConfig { .. }
            | Effect::SaveSettings(_)
            | Effect::SaveRecent(_)
            | Effect::CheckRepoFolders(_)
            | Effect::SaveTabs { .. }
            | Effect::OpenUrl(_)
            | Effect::OpenTerminal(_)
            | Effect::OpenFolder(_)
            | Effect::OpenPath { .. }
            | Effect::RevealPath(_)
            | Effect::PickFolder { .. }
            | Effect::PickFile { .. }
            | Effect::ListSigningKeys { .. }
            | Effect::GenerateGpgKey { .. }
            | Effect::ExportPublicKey { .. }
            | Effect::Ssh(_)
            | Effect::Profile(_)
            | Effect::Watch(_)
            | Effect::AnswerPrompt { .. }
            | Effect::Terminal(_)
            | Effect::Patch(_)
            | Effect::Tool(_)
            | Effect::GitEnv(_)
            | Effect::Remote(_)
            | Effect::Clone(_)
            | Effect::Init(_)
            | Effect::Notify(_)
            | Effect::Ui(_) => None,
        }
    }
}

/// Effects that need the window, carried out by the UI shell.
#[derive(Debug, PartialEq)]
pub enum UiEffect {
    CopyText(String),
    SetTitle(String),
    /// A credential prompt opened: flash the taskbar entry if the window isn't focused.
    RequestAttention,
}
