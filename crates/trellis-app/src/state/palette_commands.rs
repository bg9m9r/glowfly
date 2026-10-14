//! The command palette's commands: which are available now, their labels, hidden keywords,
//! details and the global keys that do the same (from the binding table in `keymap.rs`).

use super::palette_kinds::{InputKind, PickKind, StashAction};
use super::{AppState, Selection};
use crate::keymap::BindingId;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use trellis_core::repo::Snapshot;
use trellis_core::settings::{DiffLayout, ThemeChoice};

#[path = "palette_gitflow.rs"]
mod palette_gitflow;

/// Recent repositories offered by the palette.
const MAX_RECENT: usize = 10;

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
    /// Enter the stash message step; Enter runs `StashWithMessage`.
    StashMessage,
    /// Stash every change, untracked files included, under this message.
    StashWithMessage(String),
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
    /// Open the Start dialog for this kind.
    StartGitFlow(trellis_core::gitflow::FlowKind),
    /// Open the Finish dialog for this flow branch.
    FinishGitFlow(String),
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
    /// A REMOTE section action (Add remote…, Fetch <remote>).
    Remote(crate::remote::RemoteMsg),
}

impl PaletteCommand {
    /// The sub-list this command opens instead of running at once.
    pub fn pick_kind(&self) -> Option<PickKind> {
        match self {
            PaletteCommand::FileHistory => Some(PickKind::FileHistory),
            PaletteCommand::ChooseStash(a) => Some(PickKind::Stash(*a)),
            _ => None,
        }
    }

    /// The free-text step this command opens instead of running at once.
    pub fn input_kind(&self) -> Option<InputKind> {
        match self {
            PaletteCommand::StashMessage => Some(InputKind::StashMessage),
            _ => None,
        }
    }

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
fn folder_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
}

/// Every command available now, in the order shown for an empty query: repository commands,
/// app commands, local branch checkouts, recent repositories, then remote branch and tag
/// checkouts (last, so a repository with many refs doesn't push the rest past the row cap).
pub fn entries(st: &AppState) -> Vec<Entry> {
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
    if !snap.remote_urls.is_empty() {
        out.push(entry(C::Fetch, "Fetch all remotes", "remote update download"));
        out.push(entry(C::Pull, "Pull", "fast-forward merge update"));
    }
    super::remotes::palette_entries(snap, out);
    if snap.head_branch.is_some() {
        out.push(entry(C::Push, "Push", "upload publish"));
    }
    if snap.is_dirty() {
        out.push(entry(C::Stash, "Stash changes", "save shelve park untracked wip stash push"));
        out.push(entry(C::StashMessage, "Stash with message…", "save shelve park untracked wip stash push name describe message"));
    }
    if let Some(top) = snap.stashes.first() {
        out.push(with_detail(entry(C::PopStash, "Pop stash", "unstash apply restore stash"), Some(top.message.clone())));
    }
    if !snap.stashes.is_empty() {
        let len = snap.stashes.len();
        let n = format!("{len} stash{}", if len == 1 { "" } else { "es" });
        let choose = [
            (StashAction::Apply, "Apply stash…", "unstash restore keep stash"),
            (StashAction::Pop, "Pop stash…", "unstash apply restore remove stash"),
            (StashAction::Drop, "Delete stash…", "drop remove discard clear stash"),
        ];
        for (action, label, keywords) in choose {
            out.push(with_detail(entry(C::ChooseStash(action), label, keywords), Some(n.clone())));
        }
    }
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
    if !snap.commits.is_empty() && st.conflict.is_none() && st.history.is_none() && st.rebase.is_none() {
        out.push(entry(C::SearchCommits, "Search commits", "find message author sha"));
    }
    if super::palette::can_pick_file(st) {
        out.push(entry(C::FileHistory, "File history…", "find open file path log"));
    }
    out.push(entry(C::InitializeLfs, "Initialize Git LFS", "lfs large file storage setup install enable binary assets"));
    out.push(entry(
        C::ConfigureLfs,
        "Configure Git LFS…",
        "lfs large file storage track patterns gitattributes fetch pull push prune preferences settings",
    ));
    palette_gitflow::gitflow_entries(snap, out);
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
    out.push(entry(C::KeyboardShortcuts, "Keyboard shortcuts", "keys keyboard bindings hotkeys help reference list"));
    let labels = st.settings.toolbar_labels;
    let label = if labels { "Hide toolbar labels" } else { "Show toolbar labels" };
    out.push(entry(C::ToolbarLabels(!labels), label, "toolbar buttons text captions icons compact"));
    for (choice, name) in [(ThemeChoice::Dark, "Dark"), (ThemeChoice::Light, "Light"), (ThemeChoice::System, "Follow system")] {
        let e = entry(C::Theme(choice), format!("Theme: {name}"), "appearance colour color");
        out.push(with_detail(e, (st.settings.theme == choice).then(|| "current".to_owned())));
    }
}

/// A checkout entry for each local branch other than HEAD's.
fn checkout_entries(snap: &Snapshot, out: &mut Vec<Entry>) {
    for b in snap.locals.iter().filter(|b| !b.is_head) {
        let e = entry(PaletteCommand::Checkout(b.name.clone()), format!("Checkout {}", b.name), "switch branch");
        out.push(with_detail(e, b.upstream.clone()));
    }
}

/// A checkout entry for each remote branch that no local branch shares a name with or tracks
/// (its configured upstream counts even when the ref is gone): checking one out creates a local
/// branch tracking it.
fn remote_checkout_entries(snap: &Snapshot, out: &mut Vec<Entry>) {
    let local: HashSet<&str> = snap.locals.iter().map(|b| b.name.as_str()).collect();
    let mut tracked: HashSet<String> = HashSet::new();
    for b in &snap.locals {
        tracked.extend(b.upstream.clone());
        if let Some(t) = b.tracking.as_ref().filter(|t| t.is_remote()) {
            tracked.insert(t.display());
        }
    }
    for (remote, branches) in &snap.remotes {
        for (branch, _) in branches {
            if branch == "HEAD" || local.contains(branch.as_str()) || tracked.contains(&format!("{remote}/{branch}")) {
                continue;
            }
            let command = PaletteCommand::CheckoutRemote { remote: remote.clone(), branch: branch.clone() };
            let e = entry(command, format!("Checkout {remote}/{branch}"), "switch branch remote track tracking");
            out.push(with_detail(e, Some("creates local branch".to_owned())));
        }
    }
}

/// Hidden words for tag checkouts. "detached" comes first: after "release" its letters would
/// let a query such as "create" match every tag.
const TAG_KEYWORDS: &str = "detached switch tag release version";

/// A checkout entry for each tag, except one at the commit HEAD is already detached at.
fn tag_checkout_entries(snap: &Snapshot, out: &mut Vec<Entry>) {
    let detached_at = if snap.head_branch.is_none() { snap.head_oid } else { None };
    for (name, oid) in &snap.tags {
        if Some(*oid) == detached_at {
            continue;
        }
        let e = entry(PaletteCommand::CheckoutTag(name.clone()), format!("Checkout tag {name}"), TAG_KEYWORDS);
        out.push(with_detail(e, Some(format!("detached at {}", &oid.to_string()[..7]))));
    }
}

/// Recent repositories other than the active one.
fn recent_entries(st: &AppState, out: &mut Vec<Entry>) {
    let active = st.workdir();
    for path in st.recent.iter().take(MAX_RECENT) {
        if active.as_deref().is_some_and(|w| same_folder(w, path)) {
            continue;
        }
        let e = entry(PaletteCommand::OpenRecent(path.clone()), format!("Open {}", folder_name(path)), "recent repository");
        out.push(with_detail(e, Some(path.display().to_string())));
    }
}

/// Whether two paths name the same folder, compared by their text only (separators, trailing
/// separators and, on Windows, case don't matter). Runs every frame, so it never touches disk.
fn same_folder(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        let norm = |p: &Path| p.components().map(|c| c.as_os_str().to_string_lossy().to_lowercase()).collect::<Vec<_>>();
        norm(a) == norm(b)
    } else {
        a.components().eq(b.components())
    }
}
