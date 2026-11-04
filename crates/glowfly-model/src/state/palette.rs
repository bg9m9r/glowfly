//! The command palette (Ctrl+P): whether it is open, what is typed, which row is selected, and
//! the commands it lists for the query. App-wide, never parked with a tab. The view and `update`
//! get the list through `listed_open`, which keeps it between frames (`cache`); the tests
//! build it from scratch through `entries` and `listed` (or `shown`). Both keep at
//! most `MAX_COMMAND_ROWS` of the ranked matches; the commands themselves are
//! in `commands`. Some commands open a second step: a list of items for that command
//! (`Mode::Pick`), such as tracked files, stashes, profiles or assigned issues, or a free-text field (`Mode::Input`), such
//! as a stash message. The steps' texts are in `kinds`, a sub-list's matching in `pick`. With an empty query the list is
//! grouped: recently run commands (`recent`), then the commands the state suggests
//! (`suggested`), then all the others, each under a heading (`Listed::groups`); with a
//! query, recency only breaks score ties and there are no headings.

pub mod bindings;
mod cache;
pub mod commands;
pub(crate) mod custom;
pub(crate) mod files;
pub(crate) mod kinds;
pub mod nav;
pub(crate) mod onboarding;
mod pick;
pub(crate) mod profiles;
pub(crate) mod recent;
pub(crate) mod selected;
pub(crate) mod suggested;
pub(crate) mod tabs;
pub(crate) mod tools;
pub(crate) mod updates;

pub use commands::{Entry, PaletteCommand, entries};
pub use kinds::{InputKind, PickKind, StashAction};

use super::AppState;
use super::forge::ForgeState;
use cache::{CommandCache, by_recency};
use glowfly_core::fuzzy;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::StashInfo;
use profiles::ProfileRow;
use suggested::grouped;

#[derive(Default)]
pub struct Palette {
    pub query: String,
    pub selected: usize,
    /// Set by update after a move; the view scrolls the row into view and clears it.
    pub scroll_to_selected: bool,
    /// The command list, or a command's sub-list.
    pub mode: Mode,
    /// The command list from the last frame (`listed_open`).
    pub commands: CommandCache,
}

/// What the palette lists.
#[derive(Default)]
pub enum Mode {
    #[default]
    Commands,
    Pick(Pick),
    /// The field takes free text for one command.
    Input(InputKind),
}

impl Palette {
    /// The selected row of a list of `len` rows, kept inside the list.
    pub fn selected_in(&self, len: usize) -> Option<usize> {
        (len > 0).then(|| self.selected.min(len - 1))
    }
}

/// Most rows a sub-list shows; typing more narrows the rest.
pub const MAX_PICK_ROWS: usize = 200;

/// Most rows the command list shows; typing more narrows the rest.
pub const MAX_COMMAND_ROWS: usize = MAX_PICK_ROWS;

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
    /// The stashes behind the items of a stash sub-list (empty for other kinds), as the
    /// snapshot listed them when the sub-list opened.
    stashes: Vec<StashInfo>,
    /// The profiles behind the items of the profile sub-list (empty for other kinds).
    profiles: Vec<ProfileRow>,
    /// The issues behind the items of an issue sub-list (empty for other kinds).
    issues: Vec<IssuePickRow>,
    /// The checkout entries behind the items of the Switch branch sub-list (empty for others).
    checkouts: Vec<Entry>,
    /// The ranking for the last query, kept until the query or the items change (boxed, so a
    /// sub-list keeps `Mode` small).
    ranked: Option<Box<Ranked>>,
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

/// One row of an issue sub-list: the issue's key, its state (shown dim, matched as a keyword)
/// and the branch name built from it.
struct IssuePickRow {
    key: String,
    detail: String,
    name: String,
}

impl IssuePickRow {
    fn of(i: &glowfly_core::forge::Issue) -> Self {
        IssuePickRow { key: i.key.clone(), detail: i.state.clone(), name: glowfly_core::forge::branch_name_for(i) }
    }
}

/// The issues the ISSUES section lists: assigned ones, then the shown filter's results, each once.
pub fn issues_to_view(f: &ForgeState) -> Vec<&glowfly_core::forge::Issue> {
    let mut out: Vec<&glowfly_core::forge::Issue> = Vec::new();
    for i in f.my_issues.iter().chain(f.filters.issues.iter().flat_map(|l| l.items.iter())) {
        if !out.iter().any(|o| o.key == i.key) {
            out.push(i);
        }
    }
    out
}

/// An entry as listed for the current query, with the label's matched char indexes.
pub struct Shown {
    pub entry: Entry,
    pub matched: Vec<usize>,
}

/// A heading over part of the command list for an empty query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    /// Recently run commands, most recent first.
    Recent,
    /// What the repository's state suggests (`suggested`).
    Suggested,
    /// Every other command.
    All,
}

impl Group {
    pub fn title(self, t: Lang) -> &'static str {
        match self {
            Self::Recent => t.tr("palette.group.recent"),
            Self::Suggested => t.tr("palette.group.suggested"),
            Self::All => t.tr("palette.group.all"),
        }
    }
}

/// The command list for a query: the first `MAX_COMMAND_ROWS` matches, and how many match.
pub struct Listed {
    pub rows: Vec<Shown>,
    pub total: usize,
    /// Each group's heading and the index of its first row. Only for an empty query, and only
    /// when more than one group has rows; headings aren't rows.
    pub groups: Vec<(Group, usize)>,
}

/// `entries` filtered and ranked for `query`, cut to `MAX_COMMAND_ROWS`.
pub fn listed(st: &AppState, query: &str) -> Listed {
    let entries = entries(st);
    let items: Vec<(&str, &str)> = entries.iter().map(|e| (e.label.as_str(), e.keywords)).collect();
    let mut ranked = fuzzy::rank(query, &items);
    let command = |i: usize| entries.get(i).map(|e| &e.command);
    by_recency(st, command, &mut ranked);
    let total = ranked.len();
    let mut groups = Vec::new();
    if query.trim().is_empty() {
        (ranked, groups) = grouped(st, command, ranked);
    }
    let mut all: Vec<Option<Entry>> = entries.into_iter().map(Some).collect();
    let rows = ranked
        .into_iter()
        .take(MAX_COMMAND_ROWS)
        .filter_map(|(i, m)| Some(Shown { entry: all[i].take()?, matched: m.positions }))
        .collect();
    groups.retain(|&(_, first)| first < MAX_COMMAND_ROWS);
    Listed { rows, total, groups }
}

/// The open palette's command list for its query: `listed`, kept between frames by the
/// palette's `CommandCache`. Empty while the palette is closed.
pub fn listed_open(st: &mut AppState) -> Listed {
    let Some(p) = st.palette.as_mut() else { return Listed { rows: Vec::new(), total: 0, groups: Vec::new() } };
    let (query, mut cache) = (p.query.clone(), std::mem::take(&mut p.commands));
    let listed = cache.listed(st, &query);
    if let Some(p) = st.palette.as_mut() {
        p.commands = cache;
    }
    listed
}

/// The rows of `listed`.
pub fn shown(st: &AppState, query: &str) -> Vec<Shown> {
    listed(st, query).rows
}

/// Whether the file finder can be offered: a repository with a born HEAD and a working tree,
/// and no conflict view, interactive rebase panel, file history or blame view open.
pub fn can_pick_file(st: &AppState) -> bool {
    st.snap.as_ref().is_some_and(|s| s.head_oid.is_some())
        && st.workdir().is_some()
        && st.session.conflict.view.is_none()
        && st.session.rebase.view.is_none()
        && st.session.history.view.is_none()
        && st.session.blame.view.is_none()
}

/// Whether a stash sub-list can be offered: a repository with at least one stash.
pub fn can_pick_stash(st: &AppState) -> bool {
    st.snap.as_ref().is_some_and(|s| !s.stashes.is_empty())
}

/// Whether "Stash with message…" can be offered: a repository with changes to stash.
pub fn can_stash(st: &AppState) -> bool {
    st.snap.as_ref().is_some_and(|s| s.is_dirty())
}

/// The number of rows the palette lists now (ranking the sub-list for the query if needed).
pub fn row_count(st: &mut AppState) -> usize {
    let Some(p) = st.palette.as_ref() else { return 0 };
    let query = p.query.clone();
    match p.mode {
        Mode::Commands => return listed_open(st).rows.len(),
        Mode::Input(_) => return 0,
        Mode::Pick(_) => {}
    }
    let Some(Palette { mode: Mode::Pick(pick), .. }) = st.palette.as_mut() else { return 0 };
    pick.refresh(&query);
    pick.ranked().map_or(0, |r| r.hits.len())
}
