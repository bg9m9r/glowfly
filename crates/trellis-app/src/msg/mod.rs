//! Everything that can happen: user intents from views and results from background effects.

use crate::effect::ForgeReply;
use crate::state::PrefPage;
use crate::state::prefs::GitConfigValues;
use crate::submodule::SubmoduleMsg;
use crate::terminal::TerminalMsg;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::forge::{Account, CreatedRepo, NewPullRequest};
use trellis_core::gitcli::args;
use trellis_core::settings::{ThemeChoice, TrackerChoice};

mod branch;
mod diff;
mod files;
mod graph;
mod rebase;
mod repo;
mod tabs;
mod work;

pub use branch::{BranchMsg, CloneMsg};
pub use diff::{ConflictMsg, DiffMsg, HistoryMsg};
pub use files::{FilesMsg, WorkFileMsg};
pub use graph::{AvatarMsg, GraphMsg, SelectionMsg};
pub use rebase::RebaseMsg;
pub use repo::{LoadMsg, RepoMsg, WatchMsg};
pub use tabs::TabsMsg;
pub use work::{CommitMsg, JobMsg, StageMsg};

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
    /// The tab bar (update/tabs.rs).
    Tabs(TabsMsg),

    // ---- working tree
    /// Staging, unstaging and discarding files, hunks and lines (update/ops.rs).
    Stage(StageMsg),
    /// Creating, renaming, deleting and discarding working-tree files (update/work_files.rs).
    WorkFile(WorkFileMsg),
    /// A key from ui/shortcuts.rs: stage or unstage the open file or its selected lines, stage
    /// or unstage all, go to the commit message, or open or close the shortcut sheet. Checked
    /// and turned into the usual messages in update/shortcuts.rs.
    Shortcut(crate::keymap::KeyAction),
    /// Restoring a file from a commit, opening or showing a working-tree file (update/files.rs).
    Files(FilesMsg),
    /// The commit box (update/commit.rs).
    Commit(CommitMsg),

    // ---- git commands
    /// Git jobs: commands, push, stash, undo and redo, and their end (update/ops.rs).
    Job(JobMsg),
    /// The Clone dialog (update/clone.rs).
    Clone(CloneMsg),

    // ---- branches
    /// Hidden and soloed refs and the hovered ref (update/graph_filter.rs).
    Graph(GraphMsg),
    /// Branch menus and dialogs (update/branches.rs).
    Branch(BranchMsg),

    // ---- dialogs
    Confirm {
        title: String,
        body: String,
        button: String,
        then: Box<Msg>,
    },
    ConfirmAccepted,
    CloseModal,
    PickFolder(FolderPurpose),

    // ---- outside the app
    Copy(String),
    OpenUrl(String),
    OpenTerminal,
    OpenFolder(PathBuf),
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
        Msg::Job(JobMsg::Git { label: label.into(), cmds: vec![args(a)] })
    }

    pub fn confirm(title: &str, body: String, button: &str, then: Msg) -> Msg {
        Msg::Confirm { title: title.into(), body, button: button.into(), then: Box::new(then) }
    }
}
