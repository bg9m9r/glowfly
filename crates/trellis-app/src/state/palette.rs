//! The command palette (Ctrl+P): whether it is open, what is typed, which row is selected, and
//! the commands it offers right now. App-wide, never parked with a tab. The view, `update` and
//! the tests all build the list through `entries` and `shown`.

use super::{AppState, Selection};
use std::path::{Path, PathBuf};
use trellis_core::fuzzy;
use trellis_core::settings::ThemeChoice;

/// Recent repositories offered by the palette.
const MAX_RECENT: usize = 10;

#[derive(Default)]
pub struct Palette {
    pub query: String,
    pub selected: usize,
    /// Set by update after a move; the view scrolls the row into view and clears it.
    pub scroll_to_selected: bool,
}

impl Palette {
    /// The selected row of a list of `len` rows, kept inside the list.
    pub fn selected_in(&self, len: usize) -> Option<usize> {
        (len > 0).then(|| self.selected.min(len - 1))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PaletteCommand {
    Fetch,
    Pull,
    Push,
    StageAll,
    UnstageAll,
    CreateBranch,
    SearchCommits,
    OpenRepo,
    CloseTab,
    OpenPrefs,
    Theme(ThemeChoice),
    Checkout(String),
    OpenRecent(PathBuf),
}

pub struct Entry {
    pub command: PaletteCommand,
    pub label: String,
    /// Dim text at the right of the row.
    pub detail: Option<String>,
    /// Hidden words that also match the entry.
    pub keywords: &'static str,
}

/// An entry as listed for the current query, with the label's matched char indexes.
pub struct Shown {
    pub entry: Entry,
    pub matched: Vec<usize>,
}

fn entry(command: PaletteCommand, label: impl Into<String>, keywords: &'static str) -> Entry {
    Entry { command, label: label.into(), detail: None, keywords }
}

/// Every command available now, in the order shown for an empty query.
pub fn entries(st: &AppState) -> Vec<Entry> {
    use PaletteCommand as C;
    let mut out = Vec::new();
    if let Some(snap) = &st.snap {
        if !snap.remote_urls.is_empty() {
            out.push(entry(C::Fetch, "Fetch all remotes", "remote update download"));
            out.push(entry(C::Pull, "Pull", "fast-forward merge update"));
        }
        if snap.head_branch.is_some() {
            out.push(entry(C::Push, "Push", "upload publish"));
        }
        if !snap.unstaged.is_empty() {
            out.push(entry(C::StageAll, "Stage all files", "add everything"));
        }
        if !snap.staged.is_empty() {
            out.push(entry(C::UnstageAll, "Unstage all files", "reset everything"));
        }
        if snap.head_oid.is_some() {
            let label = match &st.selection {
                Selection::Commit(oid) => format!("Create branch at {}…", &oid.to_string()[..7]),
                _ => "Create branch at HEAD…".to_owned(),
            };
            out.push(entry(C::CreateBranch, label, "new branch"));
        }
        if !snap.commits.is_empty() && st.conflict.is_none() && st.history.is_none() && st.rebase.is_none() {
            out.push(entry(C::SearchCommits, "Search commits", "find message author sha"));
        }
    }
    out.push(entry(C::OpenRepo, "Open repository…", "folder add"));
    if st.snap.is_some() || st.tabs.len() > 1 {
        out.push(entry(C::CloseTab, "Close tab", "close repository"));
    }
    out.push(entry(C::OpenPrefs, "Open Preferences", "settings options configure"));
    for (choice, name) in [(ThemeChoice::Dark, "Dark"), (ThemeChoice::Light, "Light"), (ThemeChoice::System, "Follow system")] {
        let mut e = entry(C::Theme(choice), format!("Theme: {name}"), "appearance colour color");
        e.detail = (st.settings.theme == choice).then(|| "current".to_owned());
        out.push(e);
    }
    if let Some(snap) = &st.snap {
        for b in snap.locals.iter().filter(|b| !b.is_head) {
            let mut e = entry(C::Checkout(b.name.clone()), format!("Checkout {}", b.name), "switch branch");
            e.detail = b.upstream.clone();
            out.push(e);
        }
    }
    let active = st.workdir();
    for path in st.recent.iter().take(MAX_RECENT) {
        if active.as_deref().is_some_and(|w| same_folder(w, path)) {
            continue;
        }
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
        let mut e = entry(C::OpenRecent(path.clone()), format!("Open {name}"), "recent repository");
        e.detail = Some(path.display().to_string());
        out.push(e);
    }
    out
}

/// `entries` filtered and ranked for `query`.
pub fn shown(st: &AppState, query: &str) -> Vec<Shown> {
    let mut all: Vec<Option<Entry>> = entries(st).into_iter().map(Some).collect();
    let items: Vec<(&str, &str)> = all.iter().flatten().map(|e| (e.label.as_str(), e.keywords)).collect();
    let ranked = fuzzy::rank(query, &items);
    ranked.into_iter().filter_map(|(i, m)| Some(Shown { entry: all[i].take()?, matched: m.positions })).collect()
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
