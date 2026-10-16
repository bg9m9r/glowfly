//! The command palette's commands: which are available now, their labels, hidden keywords,
//! details and the global keys that do the same (from the binding table in `keymap.rs`).

use super::palette_kinds::StashAction;
use super::{AppState, Selection};
use crate::keymap::BindingId;
use std::path::{Path, PathBuf};
use trellis_core::repo::Snapshot;
use trellis_core::settings::{DiffLayout, ThemeChoice};

#[path = "palette_checkouts.rs"]
mod palette_checkouts;
#[path = "palette_gitflow.rs"]
mod palette_gitflow;
#[path = "palette_setup.rs"]
mod palette_setup;
#[path = "palette_shallow.rs"]
mod palette_shallow;
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
    StashDialog(trellis_core::stash_ops::StashScope),
    StageAll,
    UnstageAll,
    Undo,
    Redo,
    CreateBranch,
    /// Opens the New tag dialog at the selected commit or HEAD.
    CreateTag {
        annotated: bool,
    },
    /// Opens the Rename branch dialog for this (checked-out) branch.
    RenameBranch(String),
    SearchCommits,
    /// Open the Git LFS page and set LFS up once its status shows that it can.
    InitializeLfs,
    /// Open Preferences on the Git LFS page.
    ConfigureLfs,
    /// Open Preferences on the Git Flow page.
    ConfigureGitFlow,
    /// Open Preferences on the Sparse Checkout page.
    ConfigureSparse,
    /// Open the Start dialog for this kind.
    StartGitFlow(trellis_core::gitflow::FlowKind),
    /// Open the Finish dialog for this flow branch.
    FinishGitFlow(String),
    /// Show the Git Flow panel.
    OpenGitFlowPanel,
    /// Enter the issue sub-list for starting this kind; choosing an issue runs
    /// `StartGitFlowFromIssue`.
    ChooseGitFlowIssue(trellis_core::gitflow::FlowKind),
    /// Open the Start dialog for this kind with this name typed.
    StartGitFlowFromIssue {
        kind: trellis_core::gitflow::FlowKind,
        name: String,
    },
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
    CloseTab,
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
    /// `git checkout refs/tags/<name>`: the tagged commit, detached.
    CheckoutTag(String),
    OpenRecent(PathBuf),
    /// Enter the file finder; picking a file runs `OpenFileHistory`.
    FileHistory,
    OpenFileHistory(String),
    /// Enter the file finder; picking a file runs `OpenBlame`.
    Blame,
    /// Blame this file at HEAD.
    OpenBlame(String),
    /// A REMOTE section action (Add remote…, Fetch <remote>).
    Remote(crate::remote::RemoteMsg),
    /// A SUBMODULES action (Add submodule…).
    Submodule(crate::submodule::SubmoduleMsg),
    /// Show the trust prompt again for the open repository.
    ReviewTrust,
    /// Pick a patch file and apply it to the working tree.
    ApplyPatch,
    /// External editor actions (tools.rs).
    Tools(crate::tools::ToolsMsg),
    /// Enter the profile sub-list; choosing a profile runs SwitchProfile.
    ChooseProfile,
    /// Write this profile's identity to the global git config and make it current.
    SwitchProfile(trellis_core::settings::ProfileId),
}

impl PaletteCommand {
    /// The key that does the same thing, from the binding table.
    pub fn shortcut(&self) -> Option<&'static str> {
        self.binding().map(crate::keymap::hint)
    }

    /// The binding in `keymap.rs` that does the same thing.
    fn binding(&self) -> Option<BindingId> {
        use PaletteCommand as C;
        Some(match self {
            C::SearchCommits => BindingId::Search,
            C::CloseTab => BindingId::CloseTab,
            C::OpenPrefs => BindingId::Prefs,
            C::Undo => BindingId::Undo,
            C::Redo => BindingId::Redo,
            C::NewTab => BindingId::NewTab,
            C::ReopenClosedTab => BindingId::ReopenTab,
            C::Refresh => BindingId::Refresh,
            C::StageAll => BindingId::StageAll,
            C::UnstageAll => BindingId::UnstageAll,
            C::KeyboardShortcuts => BindingId::Sheet,
            C::ToggleTerminal => BindingId::ToggleTerminal,
            _ => return None,
        })
    }
}

pub struct Entry {
    pub command: PaletteCommand,
    pub label: String,
    /// Dim text at the right of the row.
    pub detail: Option<String>,
    /// Hidden words that also match the entry.
    pub keywords: &'static str,
    /// The global key that does the same, drawn dim at the far right.
    pub shortcut: Option<&'static str>,
}

pub(super) fn entry(command: PaletteCommand, label: impl Into<String>, keywords: &'static str) -> Entry {
    let shortcut = command.shortcut();
    Entry { command, label: label.into(), detail: None, keywords, shortcut }
}

pub(super) fn with_detail(mut e: Entry, detail: Option<String>) -> Entry {
    e.detail = detail;
    e
}

/// What Create branch and Create tag start at: the selected commit's short sha, else HEAD.
fn at_label(st: &AppState) -> String {
    match &st.selection {
        Selection::Commit(oid) => oid.to_string()[..7].to_owned(),
        _ => "HEAD".to_owned(),
    }
}

/// A folder's own name, or the whole path when it has none.
pub(super) fn folder_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
}

/// Every command available now, in the order shown for an empty query: repository commands,
/// app commands, local branch checkouts, recent repositories, then remote branch and tag
/// checkouts (last, so a repository with many refs doesn't push the rest past the row cap).
pub fn entries(st: &AppState) -> Vec<Entry> {
    use palette_checkouts::{checkout_entries, recent_entries, remote_checkout_entries, tag_checkout_entries};
    let mut out = Vec::new();
    if let Some(snap) = &st.snap {
        repo_entries(st, snap, &mut out);
    }
    app_entries(st, &mut out);
    if let Some(snap) = &st.snap {
        checkout_entries(snap, &mut out);
    }
    recent_entries(st, &mut out);
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
    palette_stash::stash_entries(snap, out);
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
    if snap.head_oid.is_some() {
        let at = at_label(st);
        out.push(entry(C::CreateBranch, format!("Create branch at {at}…"), "new branch"));
        out.push(entry(C::CreateTag { annotated: false }, format!("Create tag at {at}…"), "new tag release version label"));
        out.push(entry(
            C::CreateTag { annotated: true },
            format!("Create annotated tag at {at}…"),
            "new tag release version message annotate",
        ));
    }
    if let Some(b) = snap.head_info() {
        out.push(entry(C::RenameBranch(b.name.clone()), format!("Rename branch {}…", b.name), "rename move branch"));
    }
    if !snap.commits.is_empty() && st.conflict.is_none() && st.history.is_none() && st.blame.is_none() && st.rebase.is_none() {
        out.push(entry(C::SearchCommits, "Search commits", "find message author sha"));
    }
    if super::palette::can_pick_file(st) {
        out.push(entry(C::FileHistory, "File history…", "find open file path log"));
        out.push(entry(C::Blame, "Blame file…", "find annotate author line who changed history"));
    }
    super::patch::palette_entries(out);
    palette_setup::setup_entries(out);
    palette_gitflow::gitflow_entries(st, snap, out);
    // Here even without submodules: the SUBMODULES section (and its menu) only shows with some.
    let add = C::Submodule(crate::submodule::SubmoduleMsg::AddDialog);
    out.push(entry(add, "Add submodule…", "submodule add nested repository dependency vendor library"));
    out.push(entry(C::Refresh, "Refresh", "reload rescan reread status update"));
    out.push(entry(C::OpenTerminal, "Open in terminal", "shell console command line prompt cmd"));
    let toggle = if st.terminal.open { "Hide terminal panel" } else { "Show terminal panel" };
    out.push(entry(C::ToggleTerminal, toggle, "built-in integrated embedded shell console prompt command line toggle"));
    out.push(entry(C::ShowInFileManager, SHOW_REPO_FOLDER, "folder directory explorer finder files browse"));
    out.push(match st.settings.diff_layout {
        DiffLayout::Inline => entry(C::DiffLayout(DiffLayout::Split), "Use split diff layout", "side by side columns two diff view"),
        DiffLayout::Split => entry(C::DiffLayout(DiffLayout::Inline), "Use inline diff layout", "one column unified diff view"),
    });
    let ignoring = st.settings.diff_ignore_whitespace;
    let label = if ignoring { "Show whitespace changes" } else { "Ignore whitespace changes" };
    out.push(entry(C::IgnoreWhitespace(!ignoring), label, "whitespace spaces indentation trailing hide diff"));
}

/// Commands that work with or without a repository: tabs, Preferences and appearance.
fn app_entries(st: &AppState, out: &mut Vec<Entry>) {
    use PaletteCommand as C;
    out.push(entry(C::OpenRepo, "Open repository…", "folder add"));
    out.push(entry(C::NewTab, "New tab", "add open tab"));
    if let Some(last) = st.closed_tabs.last() {
        let e = entry(C::ReopenClosedTab, "Reopen closed tab", "restore undo closed recent tab");
        out.push(with_detail(e, Some(folder_name(last))));
    }
    if st.snap.is_some() || st.tabs.len() > 1 {
        out.push(entry(C::CloseTab, "Close tab", "close repository"));
    }
    out.push(entry(C::OpenPrefs, "Open Preferences", "settings options configure"));
    super::palette_profiles::profile_entries(st, out);
    out.push(entry(C::KeyboardShortcuts, "Keyboard shortcuts", "keys keyboard bindings hotkeys help reference list"));
    let labels = st.settings.toolbar_labels;
    let label = if labels { "Hide toolbar labels" } else { "Show toolbar labels" };
    out.push(entry(C::ToolbarLabels(!labels), label, "toolbar buttons text captions icons compact"));
    for (choice, name) in [(ThemeChoice::Dark, "Dark"), (ThemeChoice::Light, "Light"), (ThemeChoice::System, "Follow system")] {
        let e = entry(C::Theme(choice), format!("Theme: {name}"), "appearance colour color");
        out.push(with_detail(e, (st.settings.theme == choice).then(|| "current".to_owned())));
    }
    super::palette_tools::palette_entries(st, out);
}
