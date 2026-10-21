//! The command palette's commands: which are available now, their labels, hidden keywords,
//! details and the global keys that do the same (their current keys come from the effective keymap, `keymap/active.rs`).

use super::AppState;
use super::palette_kinds::StashAction;
use glowfly_core::repo::Snapshot;
use glowfly_core::settings::{DiffLayout, ThemeChoice};
use std::path::{Path, PathBuf};

#[path = "palette_checkouts.rs"]
mod palette_checkouts;
#[path = "palette_commands_rebase.rs"]
pub mod palette_commands_rebase;
#[path = "palette_commands_repos.rs"]
mod palette_commands_repos;
#[path = "palette_commands_submodules.rs"]
mod palette_commands_submodules;
pub use palette_commands_repos::RepoListCommand;
#[path = "palette_commands_worktrees.rs"]
mod palette_commands_worktrees;
#[path = "palette_conflict.rs"]
mod palette_conflict;
#[path = "palette_gitflow.rs"]
mod palette_gitflow;
#[path = "palette_issues.rs"]
pub mod palette_issues;
#[path = "palette_link.rs"]
mod palette_link;
#[path = "palette_logs.rs"]
mod palette_logs;
#[path = "palette_pr.rs"]
mod palette_pr;
#[path = "palette_setup.rs"]
mod palette_setup;
#[path = "palette_shallow.rs"]
mod palette_shallow;
#[path = "palette_stack.rs"]
pub mod palette_stack;
#[path = "palette_stash.rs"]
mod palette_stash;

/// The label of the entry that shows the repository folder in the system file manager.
pub const SHOW_REPO_FOLDER: &str = if cfg!(windows) {
    "Show repository in Explorer"
} else if cfg!(target_os = "macos") {
    "Reveal repository in Finder"
} else {
    "Open repository folder"
};

#[derive(Clone, Debug, PartialEq)]
pub enum PaletteCommand {
    Fetch,
    Pull,
    /// Download the rest of a shallow clone's history.
    FetchFullHistory,
    /// Enter the commit count step; Enter runs `FetchMoreCommitsBy`.
    FetchMoreCommits,
    /// Download this many older commits of a shallow clone.
    FetchMoreCommitsBy(u32),
    Push,
    /// Open the create pull request dialog for this (checked-out) branch.
    CreatePr(String),
    /// Push this (checked-out) branch to the code host, then open the create dialog.
    PushAndCreatePr(String),
    /// Stash every change, untracked files included.
    Stash,
    /// Pop the most recent stash.
    PopStash,
    /// Enter the stash sub-list for this action; choosing a stash runs `StashOp`.
    ChooseStash(StashAction),
    /// Apply, pop or drop the stash with this commit, found again in the current snapshot.
    StashOp {
        action: StashAction,
        oid: git2::Oid,
    },
    /// Enter the stash sub-list for renaming; choosing a stash runs `RenameStash`.
    ChooseStashToRename,
    /// Open the Rename stash dialog for the stash with this commit, found again in the current
    /// snapshot.
    RenameStash {
        oid: git2::Oid,
    },
    /// Enter the stash message step; Enter runs `StashWithMessage`.
    StashMessage,
    /// Stash every change, untracked files included, under this message.
    StashWithMessage(String),
    /// Open the Stash dialog with this choice.
    StashDialog(glowfly_core::stash_ops::StashScope),
    StageAll,
    UnstageAll,
    /// Mark every conflicted file resolved (after the markers check, as the list's button).
    MarkAllResolved,
    /// Mark every resolved file that can be restored conflicted again (asks first).
    MarkAllConflicted,
    /// Mark this resolved file conflicted again (asks first).
    MarkConflicted(String),
    Undo,
    Redo,
    CreateBranch,
    /// Opens the New tag dialog at the selected commit or HEAD.
    CreateTag {
        annotated: bool,
    },
    /// Opens the Rename branch dialog for this (checked-out) branch.
    RenameBranch(String),
    /// Compare this commit with the working directory (palette_selected.rs).
    CompareWithWorkdir(git2::Oid),
    SearchCommits,
    /// Open the Git LFS page and set LFS up once its status shows that it can.
    InitializeLfs,
    /// Open Preferences on the Git LFS page.
    ConfigureLfs,
    /// Open Preferences on the Git Flow page.
    ConfigureGitFlow,
    /// Open Preferences on the Sparse Checkout page.
    ConfigureSparse,
    /// Open Preferences on the Terminal page.
    ConfigureTerminal,
    /// Open the Start dialog for this kind.
    StartGitFlow(glowfly_core::gitflow::FlowKind),
    /// Open the Finish dialog for this flow branch.
    FinishGitFlow(String),
    /// Run the steps left by a Finish stopped at a conflict, once its merge is committed.
    ContinueGitFlowFinish,
    /// Show the Git Flow panel.
    OpenGitFlowPanel,
    /// The operation journal (update/journal.rs).
    OpenJournal,
    /// Enter the issue sub-list for starting this kind; choosing an issue runs
    /// `StartGitFlowFromIssue`.
    ChooseGitFlowIssue(glowfly_core::gitflow::FlowKind),
    /// Open the Start dialog for this kind with this name typed.
    StartGitFlowFromIssue {
        kind: glowfly_core::gitflow::FlowKind,
        name: String,
    },
    /// Open the Create issue dialog.
    CreateIssue,
    /// Enter the issue sub-list; choosing an issue runs `ViewIssue`.
    ChooseIssueToView,
    /// Open the issue panel for the issue with this key.
    ViewIssue(String),
    Refresh,
    OpenTerminal,
    /// Show or hide the built-in terminal panel.
    ToggleTerminal,
    ShowInFileManager,
    /// Switch the diff to this layout.
    DiffLayout(DiffLayout),
    /// Hide (true) or show (false) whitespace changes in diffs.
    IgnoreWhitespace(bool),
    OpenRepo,
    NewTab,
    ReopenClosedTab,
    /// Reopen this tab from the recently closed list.
    ReopenClosed(PathBuf),
    CloseTab,
    /// Open the Rename tab dialog for the active tab.
    RenameTab,
    /// Move the active tab one place left (-1) or right (1).
    MoveTab(isize),
    OpenPrefs,
    /// Open the keyboard shortcut sheet.
    KeyboardShortcuts,
    /// Show (true) or hide (false) the text under toolbar buttons.
    ToolbarLabels(bool),
    Theme(ThemeChoice),
    Checkout(String),
    /// `git checkout --track <remote>/<branch>`: creates a local branch tracking it.
    CheckoutRemote {
        remote: String,
        branch: String,
    },
    /// `TagMsg::Checkout`: the tagged commit, detached.
    CheckoutTag(String),
    OpenRecent(PathBuf),
    /// Copy a trellis:// link to the repository or a branch.
    CopyLink(crate::deep_link::LinkTarget),
    /// Enter the file finder; picking a file runs `OpenFileHistory`, a folder `OpenFolderHistory`.
    FileHistory,
    OpenFileHistory(String),
    OpenFolderHistory(String),
    /// Enter the file finder; picking a file runs `OpenBlame`.
    Blame,
    /// Blame this file at HEAD.
    OpenBlame(String),
    /// A REMOTE section action (Add remote…, Fetch <remote>).
    Remote(crate::remote::RemoteMsg),
    /// A SUBMODULES action (Add submodule…).
    Submodule(crate::submodule::SubmoduleMsg),
    /// A WORKTREES action (open, create, remove, lock, unlock, move, prune).
    Worktree(Box<crate::worktree::WorktreeMsg>),
    /// Show the trust prompt again for the open repository.
    ReviewTrust,
    /// Pick a patch file and apply it to the working tree.
    ApplyPatch,
    /// A patch action (create or copy a patch).
    Patch(crate::patch::PatchMsg),
    /// External editor actions (tools.rs).
    Tools(crate::tools::ToolsMsg),
    /// A custom command from a commands.toml file (custom_commands.rs).
    Custom(crate::custom_commands::CustomCommandMsg),
    /// The logs (logs.rs).
    Logs(crate::logs::LogsMsg),
    /// Enter the profile sub-list; choosing a profile runs SwitchProfile.
    ChooseProfile,
    /// Write this profile's identity to the global git config and make it current.
    SwitchProfile(glowfly_core::settings::ProfileId),
    /// Zoom and panel visibility (update/layout.rs).
    Layout(crate::msg::LayoutMsg),
    /// The welcome and the tour (update/onboarding.rs).
    Onboarding(crate::onboarding::OnboardingMsg),
    /// Open the cross-repository dashboard.
    Dashboard,
    /// Release notes and Trellis's own updates (update/updates.rs).
    Updates(crate::msg::UpdateMsg),
    /// Favorites and recent repositories (palette_commands_repos.rs).
    RepoList(RepoListCommand),
    /// History edits and the rebase in progress (palette_commands_rebase.rs).
    Rebase(palette_commands_rebase::RebaseCommand),
    /// Pushing the checked-out branch's stack and creating its pull requests (palette_stack.rs).
    Stack(palette_stack::StackCommand),
}

pub struct Entry {
    pub command: PaletteCommand,
    pub label: String,
    /// Dim text at the right of the row.
    pub detail: Option<String>,
    /// Hidden words that also match the entry.
    pub keywords: &'static str,
}

impl Entry {
    /// The key that does the same as this entry, as `keys` has it now; drawn dim at the far
    /// right of the row.
    pub fn shortcut<'k>(&self, keys: &'k crate::keymap::Keymap) -> Option<&'k str> {
        self.command.binding().and_then(|id| keys.hint(id))
    }
}

pub(super) fn entry(command: PaletteCommand, label: impl Into<String>, keywords: &'static str) -> Entry {
    Entry { command, label: label.into(), detail: None, keywords }
}

pub(super) fn with_detail(mut e: Entry, detail: Option<String>) -> Entry {
    e.detail = detail;
    e
}

/// A folder's own name, or the whole path when it has none.
pub(super) fn folder_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
}

/// Every command available now, in the order shown for an empty query: repository commands,
/// app commands, favorite and recent list commands, local branch checkouts, favorite and recent
/// repositories, then remote branch and tag checkouts (last, so a repository with many refs
/// doesn't push the rest past the row cap).
pub fn entries(st: &AppState) -> Vec<Entry> {
    use palette_checkouts::{checkout_entries, remote_checkout_entries, tag_checkout_entries};
    let mut out = Vec::new();
    if let Some(snap) = &st.snap {
        repo_entries(st, snap, &mut out);
    }
    app_entries(st, &mut out);
    palette_commands_repos::list_entries(st, &mut out);
    if let Some(snap) = &st.snap {
        checkout_entries(snap, &mut out);
    }
    palette_commands_repos::repo_entries(st, &mut out);
    if let Some(snap) = &st.snap {
        remote_checkout_entries(snap, &mut out);
        tag_checkout_entries(snap, &mut out);
    }
    out
}

/// Commands for the open repository.
fn repo_entries(st: &AppState, snap: &Snapshot, out: &mut Vec<Entry>) {
    use PaletteCommand as C;
    super::trust::palette_entries(st, out);
    if !snap.remote_urls.is_empty() {
        out.push(entry(C::Fetch, "Fetch all remotes", "remote update download"));
        out.push(entry(C::Pull, "Pull", "fast-forward merge update"));
    }
    palette_shallow::shallow_entries(snap, out);
    super::remotes::palette_entries(snap, &st.settings.connections, out);
    if snap.head_branch.is_some() {
        out.push(entry(C::Push, "Push", "upload publish"));
    }
    palette_pr::pr_entries(st, snap, out);
    palette_stash::stash_entries(snap, out);
    palette_conflict::conflict_entries(st, snap, out);
    palette_commands_rebase::entries(st, snap, out);
    if !snap.unstaged.is_empty() {
        out.push(entry(C::StageAll, "Stage all files", "add everything"));
    }
    if !snap.staged.is_empty() {
        out.push(entry(C::UnstageAll, "Unstage all files", "reset everything"));
    }
    if let Some(a) = st.undo.undo.last() {
        out.push(with_detail(entry(C::Undo, "Undo", "revert back step history"), Some(a.describe())));
    }
    if let Some(a) = st.undo.redo.last() {
        out.push(with_detail(entry(C::Redo, "Redo", "again forward step history"), Some(a.describe())));
    }
    super::palette_selected::entries(st, snap, out);
    if let Some(b) = snap.head_info() {
        out.push(entry(C::RenameBranch(b.name.clone()), format!("Rename branch {}…", b.name), "rename move branch"));
    }
    if !snap.commits.is_empty() && st.conflict.is_none() && st.history.is_none() && st.blame.is_none() && st.rebase.is_none() {
        out.push(entry(C::SearchCommits, "Search commits", "find message author sha"));
    }
    if super::palette::can_pick_file(st) {
        out.push(entry(C::FileHistory, "File history…", "find open file path log folder directory"));
        out.push(entry(C::Blame, "Blame file…", "find annotate author line who changed history"));
    }
    super::patch::palette_entries(st, snap, out);
    palette_setup::setup_entries(out);
    palette_gitflow::gitflow_entries(st, snap, out);
    let journal = "journal reflog history timeline states restore recover go back time travel undo before";
    out.push(entry(C::OpenJournal, "Open operation journal", journal));
    palette_commands_worktrees::worktree_entries(st, snap, out);
    palette_issues::issue_entries(st, out);
    palette_commands_submodules::submodule_entries(st, snap, out);
    super::palette_custom::entries(st, snap, out);
    out.push(entry(C::Refresh, "Refresh", "reload rescan reread status update"));
    out.push(entry(C::OpenTerminal, "Open in terminal", "shell console command line prompt cmd"));
    let toggle = if st.terminal.open { "Hide terminal panel" } else { "Show terminal panel" };
    out.push(entry(C::ToggleTerminal, toggle, "built-in integrated embedded shell console prompt command line toggle"));
    out.push(entry(C::ShowInFileManager, SHOW_REPO_FOLDER, "folder directory explorer finder files browse"));
    palette_link::link_entries(snap, out);
    out.push(match st.settings.diff_layout {
        DiffLayout::Inline => entry(C::DiffLayout(DiffLayout::Split), "Use split diff layout", "side by side columns two diff view"),
        DiffLayout::Split => entry(C::DiffLayout(DiffLayout::Inline), "Use inline diff layout", "one column unified diff view"),
    });
    let ignoring = st.settings.diff_ignore_whitespace;
    let label = if ignoring { "Show whitespace changes" } else { "Ignore whitespace changes" };
    out.push(entry(C::IgnoreWhitespace(!ignoring), label, "whitespace spaces indentation trailing hide diff"));
}

/// Commands that work with or without a repository: tabs, the dashboard, Preferences and
/// appearance.
fn app_entries(st: &AppState, out: &mut Vec<Entry>) {
    use PaletteCommand as C;
    out.push(entry(C::OpenRepo, "Open repository…", "folder add"));
    super::palette_tabs::tab_entries(st, out);
    out.push(entry(C::Dashboard, "Open dashboard", "pull requests issues reviews assigned across repositories overview inbox"));
    out.push(entry(C::OpenPrefs, "Open preferences", "settings options configure"));
    super::palette_profiles::profile_entries(st, out);
    out.push(entry(C::KeyboardShortcuts, "Keyboard shortcuts", "keys keyboard bindings hotkeys help reference list"));
    super::palette_onboarding::entries(st, out);
    super::palette_updates::entries(st, out);
    let labels = st.settings.toolbar_labels;
    let label = if labels { "Hide toolbar labels" } else { "Show toolbar labels" };
    out.push(entry(C::ToolbarLabels(!labels), label, "toolbar buttons text captions icons compact"));
    super::layout::palette_entries(st, out);
    let files =
        st.themes.catalog.files.iter().map(|f| (ThemeChoice::Custom(f.id.clone()), f.name.clone(), "appearance colour color custom"));
    let built_in = ThemeChoice::BUILT_IN.into_iter().map(|t| (t.clone(), t.palette_label(), "appearance colour color"));
    for (choice, name, words) in built_in.chain(files) {
        let current = st.settings.theme == choice;
        let e = entry(C::Theme(choice), format!("Theme: {name}"), words);
        out.push(with_detail(e, current.then(|| "current".to_owned())));
    }
    super::palette_tools::palette_entries(st, out);
    palette_logs::palette_entries(st, out);
}
