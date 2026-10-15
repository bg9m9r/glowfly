//! Everything that can happen: user intents from views and results from background effects.

use crate::effect::{FileOp, ForgeReply, UndoResult, WorkFileOk};
use crate::state::PrefPage;
use crate::state::prefs::GitConfigValues;
use crate::submodule::SubmoduleMsg;
use crate::terminal::TerminalMsg;
use git2::Oid;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::forge::{Account, CreatedRepo, NewPullRequest};
use trellis_core::gitcli::args;
use trellis_core::repo::{HunkOp, RestoredFrom};
use trellis_core::settings::{ThemeChoice, TrackerChoice};

mod diff;
mod graph;
mod rebase;
mod repo;

pub use diff::{ConflictMsg, DiffMsg, HistoryMsg};
pub use graph::{AvatarMsg, GraphMsg, SelectionMsg};
pub use rebase::RebaseMsg;
pub use repo::{LoadMsg, RepoMsg, WatchMsg};

/// What a folder picker was opened for.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FolderPurpose {
    OpenRepo,
    InitRepo,
    CloneParent,
    HooksPath,
    WorktreeParent,
}

/// What a file picker was opened for.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FilePurpose {
    /// The signing program for the current format (gpg, ssh-keygen, gpgsm).
    SigningProgram,
    SshAllowedSigners,
}

pub enum Msg {
    // ---- repository
    /// Opening the repository, selecting a commit or the working tree, the open diff (update/repo.rs).
    Repo(RepoMsg),
    /// Once per frame: drives auto-fetch and expiring toasts.
    Tick(Instant),
    /// Selecting several commits, comparing and cherry-picking them (update/selection.rs).
    Selection(SelectionMsg),
    /// The open diff's layout, folds, line selection and highlighting (update/diff.rs).
    Diff(DiffMsg),
    /// Refresh results and loading more history (update/refresh.rs).
    Load(LoadMsg),
    /// Author avatars (update/avatars.rs).
    Avatars(AvatarMsg),

    // ---- conflicts
    /// The conflict view (update/conflict.rs).
    Conflict(ConflictMsg),

    // ---- file history
    /// A file's history (update/history.rs).
    History(HistoryMsg),

    // ---- interactive rebase
    /// The interactive rebase panel (update/rebase.rs).
    Rebase(RebaseMsg),

    // ---- tabs
    NewTab,
    SelectTab(usize),
    /// Select the tab this many places to the right (negative: left), wrapping around.
    CycleTab(isize),
    /// Close tab `i`; `usize::MAX` closes the active tab.
    CloseTab(usize),
    ReopenClosedTab,
    /// Move tab `from` so it ends up at index `to` (removed, then inserted); the active tab
    /// stays active.
    MoveTab {
        from: usize,
        to: usize,
    },
    /// Close every tab except tab `i`.
    CloseOtherTabs(usize),
    /// Close every tab to the right of tab `i`.
    CloseTabsToRight(usize),
    /// Tabs saved by the last session; only the active one is loaded now.
    RestoreTabs {
        paths: Vec<PathBuf>,
        active: usize,
    },

    // ---- working tree
    StageFile(String),
    UnstageFile(String),
    StageAll,
    UnstageAll,
    /// Revert tracked `path` to its index version (the view has confirmed).
    DiscardFile(String),
    /// Open the confirmation for discarding the untracked file `path`; it goes to the Recycle Bin / Trash or is deleted the way the preference says.
    DiscardUntracked(String),
    /// Stage, unstage or discard hunk `index` of the diff shown at `generation`
    /// (`AppState::hunk_ops_gen`); refused if the diff has changed since.
    Hunk {
        op: HunkOp,
        path: String,
        index: usize,
        generation: u64,
    },
    /// Stage, unstage or discard the selected diff lines. Reads the selection when handled,
    /// so a confirmation whose diff changed meanwhile does nothing.
    ApplyLines(HunkOp),
    /// A key from ui/shortcuts.rs: stage or unstage the open file or its selected lines, stage
    /// or unstage all, go to the commit message, or open or close the shortcut sheet. Checked
    /// and turned into the usual messages in update/shortcuts.rs.
    Shortcut(crate::keymap::KeyAction),
    /// Revert hunk `index` of the commit or comparison diff shown at `generation`
    /// (AppState::hunk_ops_gen) into the working tree. Refused if the diff changed since.
    RevertHunk {
        index: usize,
        generation: u64,
    },
    /// Write `path` as of commit `oid` into the working tree (its parent's version when `deleted`,
    /// i.e. the commit deleted it). `confirmed: false` opens the confirmation first.
    /// `overwrite` is set once the user was warned that the file's current contents will be lost;
    /// without it, core refuses to write over contents that discarding couldn't bring back.
    RestoreFile {
        oid: Oid,
        path: String,
        deleted: bool,
        confirmed: bool,
        overwrite: bool,
    },
    /// Open the New file dialog with this text in its field.
    NewFileDialog(String),
    /// Create an empty file at the typed path (checked and normalized in update).
    CreateFile(String),
    /// Open the rename dialog for the working-tree file `.0`.
    RenameFileDialog(String),
    /// Move the working-tree file `from` to the typed path `to`; the index is untouched, or with
    /// `stage`, through git mv so the index records the rename.
    RenameFile {
        from: String,
        to: String,
        stage: bool,
    },
    /// The rename dialog's "Stage the rename" checkbox (saved in settings).
    SetRenameStages(bool),
    /// Delete the working-tree file `path`: `confirmed: false` opens the confirmation, and
    /// `confirmed: true` deletes straight away the way the preference says.
    DeleteFile {
        path: String,
        confirmed: bool,
    },
    /// Run the delete-family op a confirmation agreed to (Delete, UnstageDelete, UnstageGone or Discard),
    /// exactly as the confirmation described it.
    DeleteConfirmed(FileOp),
    Commit,
    /// Ctrl+Shift+Enter in the commit message: stage everything (git add --all) and commit in one job.
    StageAllAndCommit,
    /// The commit box's "Push after committing" checkbox (saved in settings).
    SetPushAfterCommit(bool),
    PrefillAmend,
    AppendToDescription(String),

    // ---- git commands
    Git {
        label: String,
        cmds: Vec<Vec<String>>,
    },
    /// Applies, pops or drops the stash shown at `stash@{index}`, only if that entry still is
    /// commit `oid` (another tab, worktree or terminal may have pushed or dropped one since).
    Stash {
        action: crate::state::palette::StashAction,
        index: usize,
        oid: Oid,
    },
    Push {
        force: bool,
    },
    /// Undo the active tab's most recent undoable operation.
    Undo,
    Redo,
    /// Clone a repository into a folder (the Clone dialog's request, options included).
    CloneRepo(crate::state::clone::CloneRequest),

    // ---- branches
    /// Hidden and soloed refs and the hovered ref (update/graph_filter.rs).
    Graph(GraphMsg),
    /// Open the rename dialog for local branch `.0`.
    RenameBranchDialog(String),
    /// `git branch -m old new`.
    RenameBranch {
        old: String,
        new: String,
    },
    /// Open the upstream dialog for local branch `.0`.
    SetUpstreamDialog(String),
    /// Make `branch` track `name` on `remote` (which need not exist yet).
    SetUpstream {
        branch: String,
        remote: String,
        name: String,
    },
    /// Remove the upstream setting of local branch `.0`.
    UnsetUpstream(String),
    /// Fast-forward local branch `.0` to its upstream, without checking it out.
    FastForwardBranch(String),
    /// Delete local branch `.0` and then its upstream branch on the remote.
    DeleteBranchAndRemote(String),
    /// Push local `branch` to `remote`; None means its upstream, or the default remote with -u.
    PushBranch {
        branch: String,
        remote: Option<String>,
    },

    // ---- dialogs
    Confirm {
        title: String,
        body: String,
        button: String,
        then: Box<Msg>,
    },
    ConfirmAccepted,
    CloseModal,
    NewBranch {
        start: Option<String>,
        name: String,
    },
    /// Opens the New tag dialog at commit `at` (a full sha); `annotated` makes the message required.
    NewTag {
        at: String,
        annotated: bool,
    },
    /// Creates tag `name` at commit `at` (a full sha), annotated when `message` isn't empty,
    /// then pushes it to remote `push`.
    CreateTag {
        at: String,
        name: String,
        message: String,
        push: Option<String>,
    },
    /// The New tag dialog's push checkbox, saved in settings.
    SetPushNewTags(bool),
    CloneDialog,
    /// The Clone dialog's parent folder text changed: check it on disk after a short pause.
    CloneParentEdited,
    /// Whether the Clone dialog's parent folder (the trimmed text `parent`) is a folder.
    CloneParentChecked {
        parent: String,
        is_dir: bool,
    },
    PickFolder(FolderPurpose),

    // ---- outside the app
    Copy(String),
    OpenUrl(String),
    OpenTerminal,
    OpenFolder(PathBuf),
    /// Open a working-tree file (repo-relative path) in the OS default program.
    OpenWorkingFile(String),
    /// Select a working-tree file in the file manager (its folder where selecting isn't supported).
    ShowWorkingFile(String),
    /// The built-in terminal panel (terminal.rs).
    Terminal(TerminalMsg),
    /// The SUBMODULES section (submodule.rs).
    Submodule(SubmoduleMsg),
    /// The WORKTREES section and Create worktree dialog (worktree.rs).
    Worktree(crate::worktree::WorktreeMsg),
    /// The REMOTE section and the Add/Edit remote dialog (remote.rs).
    Remote(crate::remote::RemoteMsg),
    /// The TAGS section's remote push and delete (tag.rs).
    Tag(crate::tag::TagMsg),

    // ---- hosting services
    ForgeRefresh {
        interactive: bool,
    },
    /// Verify and save a connection; `token: None` signs in through Git Credential Manager.
    Connect {
        account: Account,
        user: Option<String>,
        token: Option<String>,
    },
    Disconnect(Account),
    /// Disconnect everything; `gcm` also signs Git Credential Manager out of those hosts.
    ForgetTokens {
        gcm: bool,
    },
    SetTracker(TrackerChoice),
    CreatePrDialog(String),
    CreatePr(NewPullRequest),
    /// Offer to create a GitHub repository for a repository with no remote, then push to it.
    PublishDialog,
    PublishAccount(usize),
    PublishOwners {
        account: Account,
        result: Result<Vec<String>, String>,
    },
    Publish,
    Published {
        workdir: PathBuf,
        branch: String,
        result: Result<CreatedRepo, String>,
    },

    // ---- preferences
    /// The Git LFS preferences page (lfs.rs).
    Lfs(crate::lfs::LfsMsg),
    /// The Git Flow page and Start dialog (gitflow.rs).
    GitFlow(crate::gitflow::GitFlowMsg),
    OpenPrefs(PrefPage),
    ClosePrefs,
    SelectIntegration(trellis_core::forge::Provider),
    GitConfig {
        global: bool,
        key: String,
        value: Option<String>,
    },
    /// Settings were edited in place by a view; persist them.
    SettingsEdited,
    LoadSigningKeys,
    GenerateGpgKey,
    CopyPublicKey,
    PickFile(FilePurpose),
    /// Switch the colour theme (saved in settings).
    SetTheme(ThemeChoice),
    /// Show or hide the text under toolbar buttons (saved in settings).
    SetToolbarLabels(bool),

    // ---- command palette
    TogglePalette,
    ClosePalette,
    /// Move the palette selection by this many rows, wrapping.
    PaletteMove(isize),
    /// Close the palette and run this command (or, for a two-step command, open its sub-list).
    RunPaletteCommand(crate::state::palette::PaletteCommand),
    /// Leave a palette sub-list for the command list.
    PaletteBack,
    TrackedPathsLoaded {
        generation: u64,
        result: Result<Vec<String>, String>,
    },

    // ---- credential prompts (state/askpass.rs)
    /// git or ssh asks for input through the askpass helper (from the listener; unscoped).
    CredentialPrompt {
        id: u64,
        prompt: trellis_core::askpass::Prompt,
    },
    /// Prompt `id` was not answered in time and has been closed.
    CredentialPromptExpired(u64),
    /// Sign in / OK / Unlock / Yes: answer the shown prompt from the dialog's text buffers.
    AnswerCredentials,
    /// Cancel, No or Esc.
    CancelCredentials,

    // ---- results from effects
    /// A LocalOp finished; a discard that can be undone carries its undo action.
    LocalOpDone(Result<Option<trellis_core::undo::Action>, String>),
    /// A RevertHunk finished; `path` is the file it was for.
    HunkReverted {
        path: String,
        result: Result<(), String>,
    },
    /// A RestoreFile finished.
    FileRestored {
        path: String,
        oid: Oid,
        result: Result<RestoredFrom, String>,
    },
    /// A RestoreFile without `overwrite` found contents at `path` that only the working tree
    /// holds (`reason` says why), so it wrote nothing.
    RestoreWouldOverwrite {
        path: String,
        oid: Oid,
        reason: String,
    },
    /// A WorkFile effect finished.
    WorkFileDone {
        op: FileOp,
        result: Result<WorkFileOk, String>,
    },
    /// The trash refused `op`'s file; nothing changed.
    TrashRefused {
        op: FileOp,
        reason: String,
    },
    JobDone {
        ok: bool,
        output: String,
        undo: Option<UndoResult>,
    },
    ForgeReplied(ForgeReply),
    Connected {
        account: Account,
        method: String,
        token: Option<(Option<String>, String)>,
        result: Result<String, String>,
    },
    GitConfigLoaded(GitConfigValues),
    FilePicked {
        purpose: FilePurpose,
        path: Option<PathBuf>,
    },
    SigningKeysLoaded(Result<Vec<trellis_core::signing::SigningKey>, String>),
    GpgKeyGenerated(Result<String, String>),
    PublicKeyExported(Result<String, String>),
    FolderPicked {
        purpose: FolderPurpose,
        path: Option<PathBuf>,
    },
    /// A result for the repository at `workdir`, applied only if that is still the active tab.
    Scoped {
        workdir: PathBuf,
        msg: Box<Msg>,
    },
    /// The file watcher's reports (update/watch.rs).
    Watch(WatchMsg),
    /// The settings file couldn't be read at startup (it was backed up); shown once.
    SettingsUnreadable(String),
    /// The activity log's Clear button.
    ClearLog,
    /// A line for the activity log from a background effect.
    Logged {
        ok: bool,
        text: String,
    },
}

impl Msg {
    pub fn git(label: &str, a: &[&str]) -> Msg {
        Msg::Git { label: label.into(), cmds: vec![args(a)] }
    }

    pub fn confirm(title: &str, body: String, button: &str, then: Msg) -> Msg {
        Msg::Confirm { title: title.into(), body, button: button.into(), then: Box::new(then) }
    }
}
