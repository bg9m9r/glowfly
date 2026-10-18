//! The binding table: one row per action with its group, label, default keys and where it works,
//! and the stable ids the settings file names them by.

use super::chord::{Chord, KeyName};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Staging,
    Navigation,
    Diff,
    Tabs,
    Repositories,
    Palette,
    View,
    General,
}

/// The groups in the order the sheet and Preferences > Keyboard show them.
pub const GROUPS: [Group; 8] =
    [Group::Staging, Group::Navigation, Group::Diff, Group::Tabs, Group::Repositories, Group::Palette, Group::View, Group::General];

impl Group {
    pub fn title(self) -> &'static str {
        match self {
            Group::Staging => "Staging and committing",
            Group::Navigation => "Navigation",
            Group::Diff => "Diff and conflicts",
            Group::Tabs => "Tabs",
            Group::Repositories => "Repositories",
            Group::Palette => "Command palette",
            Group::View => "View",
            Group::General => "General",
        }
    }
}

/// Where a binding works.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum When {
    /// Anywhere in the main window, even while typing.
    Anywhere,
    /// Anywhere while no text field has focus.
    NotTyping,
    WorkingDiff,
    DiffOrConflict,
    Message,
    Lists,
    Search,
    InPalette,
    PaletteStep,
    Dialog,
    Blame,
}

impl When {
    /// The note the sheet shows next to a binding that only works somewhere specific.
    pub fn note(self) -> Option<&'static str> {
        match self {
            When::Anywhere | When::NotTyping => None,
            When::WorkingDiff => Some("in a diff of uncommitted changes"),
            When::DiffOrConflict => Some("in a diff or the conflict view"),
            When::Message => Some("in the commit message"),
            When::Lists => Some("in the graph and file history"),
            When::Search => Some("in the commit search field"),
            When::InPalette => Some("in the command palette"),
            When::PaletteStep => Some("in a palette sub-list"),
            When::Dialog => Some("in a dialog"),
            When::Blame => Some("in the blame view"),
        }
    }

    /// Whether a key could reach bindings of both places at once, so they can't share it.
    /// The places that need no text field (`NotTyping`, the diff, the lists, blame) overlap one
    /// another; each place with its own text field (the commit message, the search field, the
    /// palette, a dialog) overlaps only itself, and the palette's two lists overlap.
    pub fn overlaps(self, other: When) -> bool {
        use When as W;
        let browsing = |w: When| matches!(w, W::NotTyping | W::WorkingDiff | W::DiffOrConflict | W::Lists | W::Blame);
        let palette = |w: When| matches!(w, W::InPalette | W::PaletteStep);
        self == other
            || self == W::Anywhere
            || other == W::Anywhere
            || (browsing(self) && browsing(other))
            || (palette(self) && palette(other))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum BindingId {
    Stage,
    Unstage,
    StageAll,
    UnstageAll,
    FocusMessage,
    Commit,
    StageAllAndCommit,
    SelectRow,
    NextChange,
    PrevChange,
    CloseView,
    NewTab,
    CloseTab,
    ReopenTab,
    NextTab,
    PrevTab,
    GoToTab,
    LastTab,
    Palette,
    PaletteMove,
    PaletteRun,
    PaletteBack,
    PaletteClose,
    Sheet,
    Search,
    SearchNext,
    SearchFilter,
    Prefs,
    Refresh,
    Undo,
    Redo,
    DialogSubmit,
    ToggleTerminal,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ToggleLeftPanel,
    ToggleDetailsPanel,
    ToggleToolbar,
    BlameNextRun,
    BlamePrevRun,
    OpenFavorite1,
    OpenFavorite2,
    OpenFavorite3,
    OpenFavorite4,
    OpenFavorite5,
    OpenFavorite6,
    OpenFavorite7,
    OpenFavorite8,
    OpenFavorite9,
}

use BindingId as I;

/// Every id with its stable name in the settings file. Renaming one loses users' changes to it.
const KEYS: [(BindingId, &str); 50] = [
    (I::Stage, "stage"),
    (I::Unstage, "unstage"),
    (I::StageAll, "stage-all"),
    (I::UnstageAll, "unstage-all"),
    (I::FocusMessage, "focus-message"),
    (I::Commit, "commit"),
    (I::StageAllAndCommit, "stage-all-and-commit"),
    (I::SelectRow, "select-row"),
    (I::NextChange, "next-change"),
    (I::PrevChange, "prev-change"),
    (I::CloseView, "close-view"),
    (I::NewTab, "new-tab"),
    (I::CloseTab, "close-tab"),
    (I::ReopenTab, "reopen-tab"),
    (I::NextTab, "next-tab"),
    (I::PrevTab, "prev-tab"),
    (I::GoToTab, "go-to-tab"),
    (I::LastTab, "last-tab"),
    (I::Palette, "palette"),
    (I::PaletteMove, "palette-move"),
    (I::PaletteRun, "palette-run"),
    (I::PaletteBack, "palette-back"),
    (I::PaletteClose, "palette-close"),
    (I::Sheet, "shortcut-sheet"),
    (I::Search, "search"),
    (I::SearchNext, "search-next"),
    (I::SearchFilter, "search-filter"),
    (I::Prefs, "preferences"),
    (I::Refresh, "refresh"),
    (I::Undo, "undo"),
    (I::Redo, "redo"),
    (I::DialogSubmit, "dialog-submit"),
    (I::ToggleTerminal, "toggle-terminal"),
    (I::ZoomIn, "zoom-in"),
    (I::ZoomOut, "zoom-out"),
    (I::ZoomReset, "zoom-reset"),
    (I::ToggleLeftPanel, "toggle-left-panel"),
    (I::ToggleDetailsPanel, "toggle-details-panel"),
    (I::ToggleToolbar, "toggle-toolbar"),
    (I::BlameNextRun, "blame-next-run"),
    (I::BlamePrevRun, "blame-prev-run"),
    (I::OpenFavorite1, "open-favorite-1"),
    (I::OpenFavorite2, "open-favorite-2"),
    (I::OpenFavorite3, "open-favorite-3"),
    (I::OpenFavorite4, "open-favorite-4"),
    (I::OpenFavorite5, "open-favorite-5"),
    (I::OpenFavorite6, "open-favorite-6"),
    (I::OpenFavorite7, "open-favorite-7"),
    (I::OpenFavorite8, "open-favorite-8"),
    (I::OpenFavorite9, "open-favorite-9"),
];

/// Open favorite repository 1 to 9, in favorite order.
pub const FAVORITE_BINDINGS: [BindingId; 9] = [
    I::OpenFavorite1,
    I::OpenFavorite2,
    I::OpenFavorite3,
    I::OpenFavorite4,
    I::OpenFavorite5,
    I::OpenFavorite6,
    I::OpenFavorite7,
    I::OpenFavorite8,
    I::OpenFavorite9,
];

impl BindingId {
    /// Every id, in declaration order (`id as usize` indexes it).
    pub const ALL: [BindingId; 50] = {
        let mut all = [I::Stage; 50];
        let mut n = 0;
        while n < 50 {
            all[n] = KEYS[n].0;
            n += 1;
        }
        all
    };

    /// The stable name in the settings file, e.g. "new-tab".
    pub fn key(self) -> &'static str {
        KEYS[self as usize].1
    }

    pub fn from_key(key: &str) -> Option<Self> {
        KEYS.iter().find(|(_, k)| *k == key).map(|(id, _)| *id)
    }

    /// Which favorite (0 for the first) an Open favorite repository binding opens.
    pub fn favorite_slot(self) -> Option<usize> {
        FAVORITE_BINDINGS.iter().position(|&id| id == self)
    }
}

pub struct Binding {
    pub id: BindingId,
    pub group: Group,
    pub label: &'static str,
    /// The keys it has until the user changes them; the first is the one hints show. Read the
    /// current keys from `Keymap`.
    pub defaults: &'static [Chord],
    pub when: When,
    /// Whether Preferences > Keyboard can change its keys.
    pub rebindable: bool,
}

const fn b(id: BindingId, group: Group, label: &'static str, defaults: &'static [Chord], when: When) -> Binding {
    Binding { id, group, label, defaults, when, rebindable: true }
}

const fn fixed(id: BindingId, group: Group, label: &'static str, defaults: &'static [Chord], when: When) -> Binding {
    Binding { id, group, label, defaults, when, rebindable: false }
}

use Chord as C;
use Group as G;
use KeyName as K;
use When as W;

/// Every binding, in display order within its group. Rebindable rows are matched through the
/// effective keymap (`ui/shortcuts/global.rs`); fixed rows document context keys their views
/// match themselves (dialogs, the palette, lists, the commit box, Esc in `ui/central.rs`), except
/// Go to tab 1 to 8, matched with the rebindable rows. Change a fixed key there, change it here.
pub const BINDINGS: &[Binding] = &[
    b(I::Stage, G::Staging, "Stage the file or the selected lines", &[C::plain(K::Letter('S'))], W::WorkingDiff),
    b(I::Unstage, G::Staging, "Unstage the file or the selected lines", &[C::plain(K::Letter('U'))], W::WorkingDiff),
    b(I::StageAll, G::Staging, "Stage all files", &[C::cmd_shift(K::Letter('S'))], W::NotTyping),
    b(I::UnstageAll, G::Staging, "Unstage all files", &[C::cmd_shift(K::Letter('U'))], W::NotTyping),
    b(I::FocusMessage, G::Staging, "Go to the commit message", &[C::cmd_shift(K::Letter('M'))], W::Anywhere),
    fixed(I::Commit, G::Staging, "Commit", &[C::cmd(K::Enter)], W::Message),
    fixed(I::StageAllAndCommit, G::Staging, "Stage all files and commit", &[C::cmd_shift(K::Enter)], W::Message),
    fixed(I::SelectRow, G::Navigation, "Select the row above or below", &[C::plain(K::Up), C::plain(K::Down)], W::Lists),
    b(I::Search, G::Navigation, "Show or hide commit search (find in the blame view)", &[C::cmd(K::Letter('F'))], W::Anywhere),
    fixed(I::SearchNext, G::Navigation, "Next search match", &[C::plain(K::Enter)], W::Search),
    b(I::SearchFilter, G::Navigation, "Show only matching commits", &[C::cmd_shift(K::Letter('F'))], W::Anywhere),
    b(I::BlameNextRun, G::Navigation, "Next commit in blame", &[C::plain(K::Letter('N'))], W::Blame),
    b(I::BlamePrevRun, G::Navigation, "Previous commit in blame", &[C::shift(K::Letter('N'))], W::Blame),
    b(I::NextChange, G::Diff, "Next change or conflict", &[C::plain(K::F(7)), C::alt(K::Down)], W::DiffOrConflict),
    b(I::PrevChange, G::Diff, "Previous change or conflict", &[C::shift(K::F(7)), C::alt(K::Up)], W::DiffOrConflict),
    fixed(
        I::CloseView,
        G::Diff,
        "Close the diff, file history, conflict view or panel (a line selection first)",
        &[C::plain(K::Escape)],
        W::NotTyping,
    ),
    b(I::NewTab, G::Tabs, "New tab", &[C::cmd(K::Letter('T'))], W::Anywhere),
    b(I::CloseTab, G::Tabs, "Close tab", &[C::cmd(K::Letter('W'))], W::Anywhere),
    b(I::ReopenTab, G::Tabs, "Reopen closed tab", &[C::cmd_shift(K::Letter('T'))], W::Anywhere),
    b(I::NextTab, G::Tabs, "Next tab", &[C::ctrl(K::Tab)], W::Anywhere),
    b(I::PrevTab, G::Tabs, "Previous tab", &[C::ctrl_shift(K::Tab)], W::Anywhere),
    fixed(I::GoToTab, G::Tabs, "Go to tab 1 to 8", &[C::cmd(K::DigitRange(1, 8))], W::Anywhere),
    b(I::LastTab, G::Tabs, "Go to the last tab", &[C::cmd(K::Digit(9))], W::Anywhere),
    b(I::OpenFavorite1, G::Repositories, "Open favorite repository 1", &[C::cmd_alt(K::Digit(1))], W::NotTyping),
    b(I::OpenFavorite2, G::Repositories, "Open favorite repository 2", &[C::cmd_alt(K::Digit(2))], W::NotTyping),
    b(I::OpenFavorite3, G::Repositories, "Open favorite repository 3", &[C::cmd_alt(K::Digit(3))], W::NotTyping),
    b(I::OpenFavorite4, G::Repositories, "Open favorite repository 4", &[C::cmd_alt(K::Digit(4))], W::NotTyping),
    b(I::OpenFavorite5, G::Repositories, "Open favorite repository 5", &[C::cmd_alt(K::Digit(5))], W::NotTyping),
    b(I::OpenFavorite6, G::Repositories, "Open favorite repository 6", &[C::cmd_alt(K::Digit(6))], W::NotTyping),
    b(I::OpenFavorite7, G::Repositories, "Open favorite repository 7", &[C::cmd_alt(K::Digit(7))], W::NotTyping),
    b(I::OpenFavorite8, G::Repositories, "Open favorite repository 8", &[C::cmd_alt(K::Digit(8))], W::NotTyping),
    b(I::OpenFavorite9, G::Repositories, "Open favorite repository 9", &[C::cmd_alt(K::Digit(9))], W::NotTyping),
    b(I::Palette, G::Palette, "Open or close the command palette", &[C::cmd(K::Letter('P')), C::cmd_shift(K::Letter('P'))], W::Anywhere),
    fixed(I::PaletteMove, G::Palette, "Move the selection", &[C::plain(K::Up), C::plain(K::Down)], W::InPalette),
    fixed(I::PaletteRun, G::Palette, "Run the selected command", &[C::plain(K::Enter)], W::InPalette),
    fixed(
        I::PaletteBack,
        G::Palette,
        "Back to the commands (Backspace with an empty field)",
        &[C::plain(K::Backspace), C::plain(K::Escape)],
        W::PaletteStep,
    ),
    fixed(I::PaletteClose, G::Palette, "Close the palette", &[C::plain(K::Escape)], W::InPalette),
    b(I::ZoomIn, G::View, "Zoom in", &[C::cmd(K::Equals), C::cmd(K::Plus)], W::Anywhere),
    b(I::ZoomOut, G::View, "Zoom out", &[C::cmd(K::Minus)], W::Anywhere),
    b(I::ZoomReset, G::View, "Reset zoom to 100%", &[C::cmd(K::Digit(0))], W::Anywhere),
    b(I::ToggleLeftPanel, G::View, "Show or hide the left panel", &[C::cmd_shift(K::Letter('L'))], W::Anywhere),
    b(I::ToggleDetailsPanel, G::View, "Show or hide the commit details panel", &[C::cmd_shift(K::Letter('D'))], W::Anywhere),
    b(I::ToggleToolbar, G::View, "Show or hide the toolbar", &[C::cmd_shift(K::Letter('B'))], W::Anywhere),
    b(I::ToggleTerminal, G::View, "Show or hide the terminal", &[C::ctrl(K::Backtick)], W::Anywhere),
    b(I::Sheet, G::General, "Keyboard shortcuts", &[C::cmd(K::Slash)], W::Anywhere),
    b(I::Prefs, G::General, "Preferences", &[C::cmd(K::Comma)], W::Anywhere),
    b(I::Refresh, G::General, "Refresh", &[C::plain(K::F(5))], W::Anywhere),
    b(I::Undo, G::General, "Undo", &[C::cmd(K::Letter('Z'))], W::NotTyping),
    b(I::Redo, G::General, "Redo", &[C::cmd(K::Letter('Y')), C::cmd_shift(K::Letter('Z'))], W::NotTyping),
    fixed(I::DialogSubmit, G::General, "Confirm a dialog's name field", &[C::plain(K::Enter)], W::Dialog),
];

/// The binding `id`. Every id has a row in `BINDINGS`.
pub fn binding(id: BindingId) -> &'static Binding {
    BINDINGS.iter().find(|b| b.id == id).expect("every BindingId has a row in BINDINGS")
}
