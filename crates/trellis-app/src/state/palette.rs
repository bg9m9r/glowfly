//! The command palette (Ctrl+P): whether it is open, what is typed, which row is selected, and
//! the commands it offers right now. App-wide, never parked with a tab. The view, `update` and
//! the tests all build the list through `entries` and `shown`. Some commands open a second
//! step (`Mode::Pick`): a list of items for that command, such as tracked files. Commands that
//! also have a global key show it as a hint (`PaletteCommand::shortcut`).

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
    /// The command list, or a command's sub-list.
    pub mode: Mode,
}

/// What the palette lists.
#[derive(Default)]
pub enum Mode {
    #[default]
    Commands,
    Pick(Pick),
}

impl Palette {
    /// The selected row of a list of `len` rows, kept inside the list.
    pub fn selected_in(&self, len: usize) -> Option<usize> {
        (len > 0).then(|| self.selected.min(len - 1))
    }
}

/// Most rows a sub-list shows; typing more narrows the rest.
pub const MAX_PICK_ROWS: usize = 200;

/// A second step: the palette lists items for one command, with its own prompt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PickKind {
    FileHistory,
}

impl PickKind {
    /// The step label beside the search field.
    pub fn title(self) -> &'static str {
        match self {
            PickKind::FileHistory => "File history",
        }
    }

    /// The search field's hint.
    pub fn prompt(self) -> &'static str {
        match self {
            PickKind::FileHistory => "Find a file to show its history",
        }
    }

    pub fn loading(self) -> &'static str {
        match self {
            PickKind::FileHistory => "Loading files…",
        }
    }

    /// Shown when nothing matches the query.
    pub fn none_text(self) -> &'static str {
        match self {
            PickKind::FileHistory => "No matching files",
        }
    }

    /// Shown when there is nothing to pick at all.
    pub fn empty_text(self) -> &'static str {
        match self {
            PickKind::FileHistory => "This repository has no tracked files",
        }
    }

    pub fn footer(self) -> &'static str {
        match self {
            PickKind::FileHistory => "Up/Down to choose, Enter to show history, Backspace or Esc to go back",
        }
    }

    /// The line under a list cut short at `shown` of `total` matches.
    pub fn more_text(self, shown: usize, total: usize) -> String {
        match self {
            PickKind::FileHistory => format!("Showing {shown} of {total} files. Type more to narrow the list."),
        }
    }

    /// What picking `item` runs.
    pub fn command(self, item: &str) -> PaletteCommand {
        match self {
            PickKind::FileHistory => PaletteCommand::OpenFileHistory(item.to_owned()),
        }
    }
}

/// A sub-list's items, loaded in the background.
#[derive(Debug, PartialEq)]
pub enum Items {
    Loading,
    Failed(String),
    Ready(Vec<String>),
}

pub struct Pick {
    pub kind: PickKind,
    pub items: Items,
    /// The ranking for the last query, kept until the query or the items change.
    ranked: Option<Ranked>,
}

/// The items matching one query.
pub struct Ranked {
    pub query: String,
    /// Every matching item index, best first (to narrow the next, longer query).
    matches: Vec<usize>,
    /// The first `MAX_PICK_ROWS` of `matches`, with their matched char positions.
    pub hits: Vec<Hit>,
    /// How many items match.
    pub total: usize,
}

pub struct Hit {
    pub index: usize,
    /// Char indexes into the item's label.
    pub matched: Vec<usize>,
}

impl Pick {
    pub fn new(kind: PickKind) -> Self {
        Self { kind, items: Items::Loading, ranked: None }
    }

    pub fn set_items(&mut self, items: Items) {
        self.items = items;
        self.ranked = None;
    }

    /// The ranking from the last `refresh`; None while the items aren't ready.
    pub fn ranked(&self) -> Option<&Ranked> {
        self.ranked.as_ref()
    }

    /// The text of item `index`.
    pub fn label(&self, index: usize) -> &str {
        match &self.items {
            Items::Ready(v) => v.get(index).map_or("", String::as_str),
            _ => "",
        }
    }

    /// Ranks the items for `query`, unless they already are. A query that extends the last one
    /// only searches the last matches: whatever matches the longer query matches the shorter.
    pub fn refresh(&mut self, query: &str) {
        let Items::Ready(items) = &self.items else { return };
        if self.ranked.as_ref().is_some_and(|r| r.query == query) {
            return;
        }
        let squash = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        let ranked = if squash(query).is_empty() {
            let hits = (0..items.len().min(MAX_PICK_ROWS)).map(|index| Hit { index, matched: Vec::new() }).collect();
            Ranked { query: query.to_owned(), matches: (0..items.len()).collect(), hits, total: items.len() }
        } else {
            let candidates: Vec<usize> = match self.ranked.take() {
                Some(r) if !squash(&r.query).is_empty() && squash(query).starts_with(&squash(&r.query)) => {
                    let mut m = r.matches;
                    // Back to item order, so ties keep ranking the earlier item first.
                    m.sort_unstable();
                    m
                }
                _ => (0..items.len()).collect(),
            };
            let pairs: Vec<(&str, &str)> = candidates.iter().map(|&i| (items[i].as_str(), "")).collect();
            let found = fuzzy::rank(query, &pairs);
            let total = found.len();
            let mut matches = Vec::with_capacity(total);
            let mut hits = Vec::new();
            for (pos, m) in found {
                let index = candidates[pos];
                matches.push(index);
                if hits.len() < MAX_PICK_ROWS {
                    hits.push(Hit { index, matched: m.positions });
                }
            }
            Ranked { query: query.to_owned(), matches, hits, total }
        };
        self.ranked = Some(ranked);
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
    /// Opens the New tag dialog at the selected commit or HEAD.
    CreateTag {
        annotated: bool,
    },
    SearchCommits,
    OpenRepo,
    CloseTab,
    OpenPrefs,
    Theme(ThemeChoice),
    Checkout(String),
    OpenRecent(PathBuf),
    /// Enter the file finder; picking a file runs `OpenFileHistory`.
    FileHistory,
    OpenFileHistory(String),
}

impl PaletteCommand {
    /// The sub-list this command opens instead of running at once.
    pub fn pick_kind(&self) -> Option<PickKind> {
        match self {
            PaletteCommand::FileHistory => Some(PickKind::FileHistory),
            _ => None,
        }
    }

    /// The global binding in `ui/mod.rs` that does the same thing; keep the two in step.
    pub fn shortcut(&self) -> Option<&'static str> {
        match self {
            PaletteCommand::SearchCommits => Some("Ctrl+F"),
            PaletteCommand::CloseTab => Some("Ctrl+W"),
            PaletteCommand::OpenPrefs => Some("Ctrl+,"),
            _ => None,
        }
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

/// An entry as listed for the current query, with the label's matched char indexes.
pub struct Shown {
    pub entry: Entry,
    pub matched: Vec<usize>,
}

fn entry(command: PaletteCommand, label: impl Into<String>, keywords: &'static str) -> Entry {
    let shortcut = command.shortcut();
    Entry { command, label: label.into(), detail: None, keywords, shortcut }
}

/// What Create branch and Create tag start at: the selected commit's short sha, else HEAD.
fn at_label(st: &AppState) -> String {
    match &st.selection {
        Selection::Commit(oid) => oid.to_string()[..7].to_owned(),
        _ => "HEAD".to_owned(),
    }
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
            let at = at_label(st);
            out.push(entry(C::CreateBranch, format!("Create branch at {at}…"), "new branch"));
            out.push(entry(C::CreateTag { annotated: false }, format!("Create tag at {at}…"), "new tag release version label"));
            out.push(entry(
                C::CreateTag { annotated: true },
                format!("Create annotated tag at {at}…"),
                "new tag release version message annotate",
            ));
        }
        if !snap.commits.is_empty() && st.conflict.is_none() && st.history.is_none() && st.rebase.is_none() {
            out.push(entry(C::SearchCommits, "Search commits", "find message author sha"));
        }
        if can_pick_file(st) {
            out.push(entry(C::FileHistory, "File history…", "find open file path log"));
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

/// Whether the file finder can be offered: a repository with a born HEAD and a working tree,
/// and no conflict view, interactive rebase panel or file history open.
pub fn can_pick_file(st: &AppState) -> bool {
    st.snap.as_ref().is_some_and(|s| s.head_oid.is_some())
        && st.workdir().is_some()
        && st.conflict.is_none()
        && st.rebase.is_none()
        && st.history.is_none()
}

/// The number of rows the palette lists now (ranking the sub-list for the query if needed).
pub fn row_count(st: &mut AppState) -> usize {
    let Some(p) = st.palette.as_ref() else { return 0 };
    let query = p.query.clone();
    if matches!(p.mode, Mode::Commands) {
        return shown(st, &query).len();
    }
    let Some(Palette { mode: Mode::Pick(pick), .. }) = st.palette.as_mut() else { return 0 };
    pick.refresh(&query);
    pick.ranked().map_or(0, |r| r.hits.len())
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
