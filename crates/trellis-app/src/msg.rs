//! Everything that can happen: user intents from views and results from background effects.

use crate::effect::{FileOp, ForgeReply, LoadKind, UndoResult};
use crate::state::PrefPage;
use crate::state::lines::LineClick;
use crate::state::prefs::GitConfigValues;
use git2::Oid;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::forge::{Account, CreatedRepo, NewPullRequest};
use trellis_core::gitcli::args;
use trellis_core::highlight::DiffHighlight;
use trellis_core::rebase::{Outcome, RebaseAction, RebaseBase, RebasePlan};
use trellis_core::repo::{CommitDetail, ConflictFile, DiffTarget, FileDiff, FileEntry, FileHistory, HunkOp, RestoredFrom, Side, Snapshot};
use trellis_core::settings::{DiffLayout, DiffMode, FilterOp, ThemeChoice, TrackerChoice};

/// What a folder picker was opened for.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FolderPurpose {
    OpenRepo,
    InitRepo,
    CloneParent,
    HooksPath,
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
    PickRepo,
    Open(PathBuf),
    Refresh,
    WindowFocused,
    /// Once per frame: drives auto-fetch and expiring toasts.
    Tick(Instant),
    SelectWip,
    SelectCommit(Oid),
    /// Ctrl/Cmd-click on a graph row: add or remove the commit from the selection.
    ToggleCommit(Oid),
    /// Shift-click on a graph row: select every row from the anchor to this commit.
    SelectCommitRange(Oid),
    /// Esc: back to the one commit the multi-selection started from.
    CollapseSelection,
    /// Cherry-pick these commits onto HEAD, oldest first (the order is decided in update).
    CherryPick(Vec<Oid>),
    JumpTo(Oid),
    OpenDiff(DiffTarget),
    CloseDiff,
    /// Switch the diff between one column and side by side (saved in settings).
    SetDiffLayout(DiffLayout),
    /// Switch the diff between hunks only and the whole file (saved in settings).
    SetDiffMode(DiffMode),
    /// Diff header toggle: hide whitespace-only changes and reload the open diff.
    SetIgnoreWhitespace(bool),
    /// Scroll the diff to the next (1) or previous (-1) change; at either end, open the next or
    /// previous changed file of the list the diff came from (on its first or last change).
    JumpToChange(isize),
    /// A click on a whole-file "N unchanged lines" row: show the lines of the fold with this key
    /// in place.
    ExpandUnchanged(u32),
    /// A click on line `line` of hunk `hunk` in the open diff (line-level staging).
    SelectDiffLine {
        hunk: usize,
        line: usize,
        how: LineClick,
    },
    ClearLineSelection,
    SearchNext,
    /// The graph shows these commits and needs their line counts.
    WantStats(Vec<Oid>),
    /// Views asked for these authors' avatars (lowercased emails, each asked for once).
    WantAvatars(Vec<String>),

    // ---- conflicts
    OpenConflict(String),
    CloseConflict,
    /// Tick or untick a side of chunk `chunk` in the open conflict.
    ConflictToggle {
        chunk: usize,
        side: Side,
    },
    ConflictTakeAll(Side),
    ConflictReset,
    /// Make the next (1) or previous (-1) chunk current.
    ConflictStep(isize),
    /// Make chunk `i` current (a click on it) without scrolling.
    ConflictSelect(usize),
    /// Write the output to the file and stage it.
    SaveConflict,
    /// Binary or deleted files: resolve with one side's whole version.
    ResolveWithSide {
        path: String,
        side: Side,
    },
    ConflictLoaded {
        path: String,
        generation: u64,
        result: Result<ConflictFile, String>,
    },

    // ---- file history
    /// The commits that changed `path`, from HEAD (`at: None`) or as seen from commit `at`.
    OpenFileHistory {
        path: String,
        at: Option<Oid>,
    },
    CloseFileHistory,
    /// Select entry `i` of the open file history and show its diff.
    SelectHistoryEntry(usize),
    /// Continue the walk past the commit limit.
    MoreFileHistory,
    /// "View on graph": close the history and select the commit.
    ShowInGraph(Oid),
    FileHistoryLoaded {
        path: String,
        generation: u64,
        result: Result<FileHistory, String>,
    },

    // ---- interactive rebase
    /// Open the panel with the list for rebasing the current branch from `base`.
    OpenRebase(RebaseBase),
    CloseRebase,
    RebaseSetAction {
        index: usize,
        action: RebaseAction,
    },
    /// Move row `from` so it ends up at index `to` (removed, then inserted).
    RebaseMove {
        from: usize,
        to: usize,
    },
    /// Open the message editor on row `i`.
    RebaseEditMessage(usize),
    /// Update message: keep the editor's text and make the row a reword.
    RebaseSaveMessage,
    RebaseCancelMessage,
    /// The original order and actions (or, for a stale list, the list reloaded).
    RebaseReset,
    /// Start the rebase; drops ask for confirmation first (`confirmed: false`).
    StartRebase {
        confirmed: bool,
    },
    RebasePlanLoaded {
        generation: u64,
        result: Result<RebasePlan, String>,
    },
    /// The rebase job's result, for whichever tab it ran in (like `JobDone`).
    RebaseFinished {
        result: Result<Outcome, String>,
    },

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
    DiscardFile(String, bool),
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
    /// Delete the working-tree file `path`; `confirmed: false` opens the confirmation first.
    DeleteFile {
        path: String,
        confirmed: bool,
    },
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
    Push {
        force: bool,
    },
    /// Undo the active tab's most recent undoable operation.
    Undo,
    Redo,
    InitRepo,
    CloneRepo {
        url: String,
        parent: PathBuf,
        name: String,
    },

    // ---- branches
    /// Hide, show or solo a ref in the active repository's graph (saved per repository).
    GraphFilter(FilterOp),
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
    CloneDialog,
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
    RepoLoaded {
        generation: u64,
        kind: LoadKind,
        result: Result<Box<Snapshot>, String>,
    },
    StatusLoaded {
        generation: u64,
        result: Result<(Vec<FileEntry>, Vec<FileEntry>, git2::RepositoryState), String>,
    },
    DetailLoaded {
        oid: Oid,
        result: Result<CommitDetail, String>,
    },
    /// The files that differ from commit `from` (older) to `to` (newer).
    CompareLoaded {
        from: Oid,
        to: Oid,
        result: Result<Vec<FileEntry>, String>,
    },
    DiffLoaded {
        target: DiffTarget,
        reload: bool,
        generation: u64,
        result: Result<FileDiff, String>,
    },
    /// A HighlightDiff finished: exact spans for the hunks and/or the whole file of the diff shown
    /// at `generation`, each only where it was pending. Applied only while that diff is still on
    /// screen.
    DiffHighlighted {
        target: DiffTarget,
        generation: u64,
        hunks: Option<DiffHighlight>,
        whole: Option<DiffHighlight>,
    },
    LocalOpDone(Result<(), String>),
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
    /// A WorkFile effect finished. `Ok(true)` means git ignores the new path (always false for
    /// a deletion).
    WorkFileDone {
        op: FileOp,
        result: Result<bool, String>,
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
    StatsLoaded(Vec<(Oid, usize, usize, usize)>),
    /// Fetched avatars; `None` when an author has none (the view shows initials).
    AvatarsLoaded(Vec<(String, Option<trellis_core::avatars::Image>)>),
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
    /// Files changed outside Trellis (debounced). Sent wrapped in `Msg::Scoped` for the watched
    /// repository; `full` when HEAD, refs or a merge/rebase state changed; `last_event` is when
    /// the burst's last relevant event arrived.
    RepoChanged {
        full: bool,
        last_event: Instant,
    },
    /// The file watcher could not start or stopped working for `workdir`.
    WatchFailed {
        workdir: PathBuf,
        error: String,
    },
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
