//! Everything that can happen: user intents from views and results from background effects.

use crate::submodule::SubmoduleMsg;
use crate::terminal::TerminalMsg;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::gitcli::args;

mod branch;
mod diff;
mod files;
mod forge;
mod graph;
mod prefs;
mod rebase;
mod repo;
mod tabs;
mod work;

pub use branch::{BranchMsg, CloneMsg};
pub use diff::{ConflictMsg, DiffMsg, HistoryMsg};
pub use files::{FilesMsg, WorkFileMsg};
pub use forge::{CredentialMsg, ForgeMsg, PublishMsg};
pub use graph::{AvatarMsg, GraphMsg, SelectionMsg};
pub use prefs::{FilePurpose, FolderPurpose, PaletteMsg, PrefsMsg};
pub use rebase::RebaseMsg;
pub use repo::{LoadMsg, RepoMsg, WatchMsg};
pub use tabs::TabsMsg;
pub use work::{CommitMsg, JobMsg, StageMsg};

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
    /// The Preferences window, pickers and signing keys (update/prefs.rs).
    Prefs(PrefsMsg),

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
    /// Hosting services: pull requests, issues, connections (update/forge.rs).
    Forge(ForgeMsg),
    /// Publishing a repository with no remote (update/publish.rs).
    Publish(PublishMsg),

    // ---- preferences
    /// The Git LFS preferences page (lfs.rs).
    Lfs(crate::lfs::LfsMsg),
    /// The Git Flow page and Start dialog (gitflow.rs).
    GitFlow(crate::gitflow::GitFlowMsg),

    // ---- command palette
    /// The command palette (update/palette.rs).
    Palette(PaletteMsg),

    // ---- credential prompts (state/askpass.rs)
    /// Credential prompts from git and ssh (update/askpass.rs).
    Credentials(CredentialMsg),

    // ---- results from effects
    /// A result for the repository at `workdir`, applied only if that is still the active tab.
    Scoped {
        workdir: PathBuf,
        msg: Box<Msg>,
    },
    /// The file watcher's reports (update/watch.rs).
    Watch(WatchMsg),
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
