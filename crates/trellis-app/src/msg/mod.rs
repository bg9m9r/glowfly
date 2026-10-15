//! Everything that can happen: user intents from views and results from background effects.
//!
//! `Msg` has one variant per area, each carrying that area's own enum (defined in this folder,
//! or in the area's module for `lfs`, `gitflow`, `worktree`, `remote`, `tag`, `submodule` and
//! `terminal`). Each area's enum goes to one handler in `update/`, which matches it
//! exhaustively, and says through `on_inactive` what its results still need when they arrive
//! for a tab that is no longer active. Only a few cross-cutting messages are top-level.

use crate::effect::{FileOp, WorkFileOk};
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
mod trust;
mod work;

pub use branch::{BranchMsg, CloneMsg};
pub use diff::{ConflictMsg, DiffMsg, HistoryMsg};
pub use files::{FilesMsg, WorkFileMsg, restore_failed};
pub use forge::{CredentialMsg, ForgeMsg, PublishMsg};
pub use graph::{AvatarMsg, GraphMsg, SelectionMsg};
pub use prefs::{FilePurpose, FolderPurpose, PaletteMsg, PrefsMsg};
pub use rebase::RebaseMsg;
pub use repo::{LoadMsg, RepoMsg, WatchMsg};
pub use tabs::TabsMsg;
pub use trust::TrustMsg;
pub use work::{CommitMsg, JobMsg, StageMsg};

pub enum Msg {
    /// Opening the repository, selecting a commit or the working tree, the open diff (update/repo.rs).
    Repo(RepoMsg),
    /// Refresh results and loading more history (update/refresh.rs).
    Load(LoadMsg),
    /// The file watcher's reports (update/watch.rs).
    Watch(WatchMsg),
    /// Selecting several commits, comparing and cherry-picking them (update/selection.rs).
    Selection(SelectionMsg),
    /// Hidden and soloed refs and the hovered ref (update/graph_filter.rs).
    Graph(GraphMsg),
    /// Author avatars (update/avatars.rs).
    Avatars(AvatarMsg),
    /// The open diff's layout, folds, line selection and highlighting (update/diff.rs).
    Diff(DiffMsg),
    /// The conflict view (update/conflict.rs).
    Conflict(ConflictMsg),
    /// A file's history (update/history.rs).
    History(HistoryMsg),
    /// The interactive rebase panel (update/rebase.rs).
    Rebase(RebaseMsg),
    /// The tab bar (update/tabs.rs).
    Tabs(TabsMsg),
    /// Staging, unstaging and discarding files, hunks and lines (update/ops.rs).
    Stage(StageMsg),
    /// The commit box (update/commit.rs).
    Commit(CommitMsg),
    /// Git jobs: commands, push, stash, undo and redo, and their end (update/ops.rs).
    Job(JobMsg),
    /// Restoring a file from a commit, opening or showing a working-tree file (update/files.rs).
    Files(FilesMsg),
    /// Creating, renaming, deleting and discarding working-tree files (update/work_files.rs).
    WorkFile(WorkFileMsg),
    /// Branch menus and dialogs (update/branches.rs).
    Branch(BranchMsg),
    /// The Clone dialog (update/clone.rs).
    Clone(CloneMsg),
    /// The New tag dialog and the TAGS section's remote push and delete (tag.rs).
    Tag(crate::tag::TagMsg),
    /// The REMOTE section and the Add/Edit remote dialog (remote.rs).
    Remote(crate::remote::RemoteMsg),
    /// The WORKTREES section and Create worktree dialog (worktree.rs).
    Worktree(crate::worktree::WorktreeMsg),
    /// The SUBMODULES section (submodule.rs).
    Submodule(SubmoduleMsg),
    /// The built-in terminal panel (terminal.rs).
    Terminal(TerminalMsg),
    /// Hosting services: pull requests, issues, connections (update/forge.rs).
    Forge(ForgeMsg),
    /// Publishing a repository with no remote (update/publish.rs).
    Publish(PublishMsg),
    /// Credential prompts from git and ssh (update/askpass.rs).
    Credentials(CredentialMsg),
    /// The Preferences window, pickers and signing keys (update/prefs.rs).
    Prefs(PrefsMsg),
    /// The Git LFS preferences page (lfs.rs).
    Lfs(crate::lfs::LfsMsg),
    /// The Git Flow page and Start dialog (gitflow.rs).
    GitFlow(crate::gitflow::GitFlowMsg),
    /// The command palette (update/palette.rs).
    Palette(PaletteMsg),
    /// The repository trust prompt (update/trust.rs).
    Trust(TrustMsg),
    /// A key from ui/shortcuts.rs: stage or unstage the open file or its selected lines, stage
    /// or unstage all, go to the commit message, or open or close the shortcut sheet. Checked
    /// and turned into the usual messages in update/shortcuts.rs.
    Shortcut(crate::keymap::KeyAction),

    // ---- Cross-cutting: not one area's, so handled in update/mod.rs.
    /// Once per frame: drives auto-fetch and expiring toasts.
    Tick(Instant),
    /// A result for the repository at `workdir`, applied only if that is still the active tab
    /// (otherwise see `on_inactive`). Wraps any area's result.
    Scoped {
        workdir: PathBuf,
        msg: Box<Msg>,
    },
    /// Ask before running `then`, from any area (`Msg::confirm`).
    Confirm {
        title: String,
        body: String,
        button: String,
        then: Box<Msg>,
    },
    ConfirmAccepted,
    /// Close whichever dialog is open.
    CloseModal,
    // The outside world, from menus all over the app.
    Copy(String),
    OpenUrl(String),
    OpenTerminal,
    OpenFolder(PathBuf),
    /// The activity log's Clear button.
    ClearLog,
    /// A line for the activity log from a background effect of any area.
    Logged {
        ok: bool,
        text: String,
    },
}

/// What a result for a tab that is no longer active still needs (`Msg::on_inactive`).
pub enum Inactive {
    /// A failure for the activity log.
    Log(String),
    /// An undoable operation: its undo belongs to the history of the tab it ran in.
    Undo(trellis_core::undo::Action),
    /// A failure for the activity log that still left an undoable change (both of the above).
    LogAndUndo { text: String, undo: trellis_core::undo::Action },
    /// A working-tree file operation's result: a dialog may still be waiting for it.
    WorkFile { op: FileOp, result: Result<WorkFileOk, String> },
}

impl Msg {
    /// What a result for a tab that is no longer active still needs (`Msg::Scoped`); `None`
    /// drops it, and the tab reloads when it is selected again. Each area decides for each of
    /// its messages.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Msg::Repo(m) => m.on_inactive(),
            Msg::Load(m) => m.on_inactive(),
            Msg::Watch(m) => m.on_inactive(),
            Msg::Selection(m) => m.on_inactive(),
            Msg::Graph(m) => m.on_inactive(),
            Msg::Avatars(m) => m.on_inactive(),
            Msg::Diff(m) => m.on_inactive(),
            Msg::Conflict(m) => m.on_inactive(),
            Msg::History(m) => m.on_inactive(),
            Msg::Rebase(m) => m.on_inactive(),
            Msg::Tabs(m) => m.on_inactive(),
            Msg::Stage(m) => m.on_inactive(),
            Msg::Commit(m) => m.on_inactive(),
            Msg::Job(m) => m.on_inactive(),
            Msg::Files(m) => m.on_inactive(),
            Msg::WorkFile(m) => m.on_inactive(),
            Msg::Branch(m) => m.on_inactive(),
            Msg::Clone(m) => m.on_inactive(),
            Msg::Tag(m) => m.on_inactive(),
            Msg::Remote(m) => m.on_inactive(),
            Msg::Worktree(m) => m.on_inactive(),
            Msg::Submodule(m) => m.on_inactive(),
            Msg::Terminal(m) => m.on_inactive(),
            Msg::Forge(m) => m.on_inactive(),
            Msg::Publish(m) => m.on_inactive(),
            Msg::Credentials(m) => m.on_inactive(),
            Msg::Prefs(m) => m.on_inactive(),
            Msg::Lfs(m) => m.on_inactive(),
            Msg::GitFlow(m) => m.on_inactive(),
            Msg::Palette(m) => m.on_inactive(),
            Msg::Trust(m) => m.on_inactive(),
            Msg::Tick(_)
            | Msg::Shortcut(_)
            | Msg::Confirm { .. }
            | Msg::ConfirmAccepted
            | Msg::CloseModal
            | Msg::Copy(_)
            | Msg::OpenUrl(_)
            | Msg::OpenTerminal
            | Msg::OpenFolder(_)
            | Msg::Scoped { .. }
            | Msg::ClearLog
            | Msg::Logged { .. } => None,
        }
    }

    pub fn git(label: &str, a: &[&str]) -> Msg {
        Msg::Job(JobMsg::Git { label: label.into(), cmds: vec![args(a)] })
    }

    pub fn confirm(title: &str, body: String, button: &str, then: Msg) -> Msg {
        Msg::Confirm { title: title.into(), body, button: button.into(), then: Box::new(then) }
    }
}
