//! Side effects requested by `update`. The runner executes them off the UI thread and reports
//! back with `Msg`s; the few that need the window (`UiEffect`) are handled by the UI shell.

pub use crate::file_op::{DeleteMode, FileOp, WorkFileOk};
use crate::msg::{FilePurpose, FolderPurpose};
use git2::Oid;
use std::path::PathBuf;
use trellis_core::askpass::Reply;
use trellis_core::forge::{Account, Auth, Issue, NewPullRequest, NewRepo, PullRequest, Target};
use trellis_core::rebase::{RebaseBase, StartRequest};
use trellis_core::repo::{ConflictIds, DiffTarget, FileDiff, HistoryStart, Hunk, HunkOp, LoadOptions, Side};
use trellis_core::settings::Settings;
use trellis_core::undo::{Action, UndoKind};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LoadKind {
    Open,
    Refresh,
}

/// Quick index/working-tree edits (through libgit2, or git where filters must run).
#[derive(Debug, PartialEq)]
pub enum LocalOp {
    StageFile(String),
    StageAll,
    UnstagePaths(Vec<String>),
    /// Checks out the index version of a tracked file.
    DiscardFile(String),
    Hunk {
        op: HunkOp,
        path: String,
        index: usize,
        /// The hunk as displayed; the operation is refused if the file's hunk differs now.
        expected: Hunk,
    },
    /// Lines `lines` (sorted) of hunk `hunk`, whose header must still be `header`.
    Lines {
        op: HunkOp,
        path: String,
        hunk: usize,
        /// The hunk as displayed; the operation is refused if the file's hunk differs now.
        expected: Hunk,
        lines: Vec<usize>,
    },
    /// Writes a resolved conflict's output and stages it, if the conflict is still `expected`.
    ResolveConflict {
        path: String,
        contents: String,
        expected: ConflictIds,
    },
    /// Resolves a conflict with one side's whole version (or its deletion).
    ResolveWithSide {
        path: String,
        side: Side,
        expected: ConflictIds,
    },
}

/// What to ask a hosting service.
#[derive(Debug)]
pub enum ForgeCall {
    /// Pull requests (plus the default branch and, when the host is also the tracker, issues).
    Refresh {
        need_default: bool,
        with_issues: bool,
    },
    MyIssues,
    Issues(Vec<String>),
    Create(NewPullRequest),
}

/// A hosting-service failure, flagged when signing in again could fix it.
#[derive(Debug, Clone, PartialEq)]
pub struct ForgeError {
    pub message: String,
    pub auth: bool,
}

impl From<trellis_core::error::Error> for ForgeError {
    fn from(e: trellis_core::error::Error) -> Self {
        ForgeError { auth: e.is_auth(), message: e.to_string() }
    }
}

#[derive(Debug)]
pub enum ForgeResult {
    AuthFailed(String),
    Prs(Result<Vec<PullRequest>, ForgeError>),
    MyIssues(Result<Vec<Issue>, ForgeError>),
    Issues(Vec<String>, Result<Vec<Issue>, ForgeError>),
    DefaultBranch(Option<String>),
    Created(Result<PullRequest, ForgeError>),
}

#[derive(Debug)]
pub struct ForgeReply {
    pub target: Target,
    pub auth: Option<Auth>,
    pub results: Vec<ForgeResult>,
}

/// What a refresh compares against before loading everything again.
#[derive(Clone, Debug, PartialEq)]
pub struct KnownRepo {
    pub refs_key: u64,
    /// The snapshot has submodules: read them again even without a .gitmodules.
    pub submodules: bool,
    /// The file whose working-tree or index diff is open, for `Msg::RepoUnchanged::open_stamp`.
    pub open: Option<String>,
}

/// A repository for the file watcher.
#[derive(Clone, Debug, PartialEq)]
pub struct WatchTarget {
    pub workdir: PathBuf,
    pub git_dir: PathBuf,
}

#[derive(Debug)]
pub enum Effect {
    LoadRepo {
        path: PathBuf,
        opts: LoadOptions,
        generation: u64,
        kind: LoadKind,
        /// The snapshot on screen: when its `refs_key` still matches, only the status is read
        /// (`Msg::RepoUnchanged`).
        known: Option<KnownRepo>,
    },
    LoadStatus {
        workdir: PathBuf,
        generation: u64,
        /// The last load found submodules: read them again even without a .gitmodules.
        submodules: bool,
        /// The file whose working-tree or index diff is open, for `Msg::StatusLoaded::open_stamp`.
        open: Option<String>,
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
    /// doesn't wait on the disk. Reported as `Msg::CloneParentChecked`.
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
    /// Git LFS status for the preferences page: installed version, set up or not, tracked patterns.
    LoadLfs {
        workdir: PathBuf,
        generation: u64,
    },
    /// The commits that changed `path`, examining at most `limit` commits.
    LoadFileHistory {
        workdir: PathBuf,
        path: String,
        start: HistoryStart,
        limit: usize,
        generation: u64,
    },
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
    /// The open tabs (repositories) and which one is active, for the next start.
    SaveTabs {
        paths: Vec<PathBuf>,
        active: usize,
    },
    OpenUrl(String),
    OpenTerminal(PathBuf),
    OpenFolder(PathBuf),
    /// Opens an absolute path in the default program.
    OpenPath(PathBuf),
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
    /// The built-in terminal panel's shells; handled by Runner::run, never a worker.
    Terminal(crate::terminal::TerminalEffect),
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
            | Effect::LoadRebasePlan { workdir, .. }
            | Effect::LocalOp { workdir, .. }
            | Effect::RevertHunk { workdir, .. }
            | Effect::RestoreFile { workdir, .. }
            | Effect::WorkFile { workdir, .. }
            | Effect::Stats { workdir, .. } => Some(workdir),
            Effect::Forge { cwd, .. } => Some(cwd),
            _ => None,
        }
    }
}

/// What a git job means for the undo history.
#[derive(Debug)]
pub enum UndoResult {
    /// An undoable job ran; `None` when it changed nothing that can be undone.
    Recorded(Option<Action>),
    /// An undo or redo ran. `before` is the history entry and `after` the entry as it stands now
    /// (they differ only for a redone discard, which makes a new stash).
    Done { redo: bool, before: Action, after: Action },
    /// The reversing git commands failed; the entry stays.
    Failed { redo: bool },
    /// The undo or redo was refused before running anything; a stale history is cleared.
    Refused { redo: bool, stale: bool },
}

/// Effects that need the window, carried out by the UI shell.
#[derive(Debug, PartialEq)]
pub enum UiEffect {
    CopyText(String),
    SetTitle(String),
    /// A credential prompt opened: flash the taskbar entry if the window isn't focused.
    RequestAttention,
}
