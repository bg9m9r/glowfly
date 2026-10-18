//! The binding table: one row per action with its group, label, default keys and where it works.
//! The ids and their stable names in the settings file are in `ids.rs`.

use super::chord::{Chord, KeyName};
use super::ids::BindingId;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Staging,
    Navigation,
    Diff,
    History,
    Tabs,
    Repositories,
    Palette,
    View,
    General,
}

/// The groups in the order the sheet and Preferences > Keyboard show them.
pub const GROUPS: [Group; 9] = [
    Group::Staging,
    Group::Navigation,
    Group::Diff,
    Group::History,
    Group::Tabs,
    Group::Repositories,
    Group::Palette,
    Group::View,
    Group::General,
];

impl Group {
    pub fn title(self) -> &'static str {
        match self {
            Group::Staging => "Staging and committing",
            Group::Navigation => "Navigation",
            Group::Diff => "Diff and conflicts",
            Group::History => "Editing history",
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
    /// The commit graph is the central view.
    Graph,
    /// The interactive rebase panel is the central view.
    RebasePanel,
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
            When::Graph => Some("in the commit graph"),
            When::RebasePanel => Some("in the interactive rebase panel"),
        }
    }

    /// Whether a key could reach bindings of both places at once, so they can't share it.
    /// The places that need no text field (`NotTyping`, the diff, the lists, the graph, blame)
    /// overlap one another; each place with its own text field (the commit message, the search
    /// field, the palette, a dialog) overlaps only itself, and the palette's two lists overlap.
    /// The rebase panel is drawn only when no diff, list, graph or blame view is, so it
    /// overlaps only itself, `Anywhere` and `NotTyping`.
    pub fn overlaps(self, other: When) -> bool {
        use When as W;
        let browsing = |w: When| matches!(w, W::NotTyping | W::WorkingDiff | W::DiffOrConflict | W::Lists | W::Blame | W::Graph);
        let palette = |w: When| matches!(w, W::InPalette | W::PaletteStep);
        let panel = |a: When, b: When| a == W::RebasePanel && b == W::NotTyping;
        self == other
            || self == W::Anywhere
            || other == W::Anywhere
            || panel(self, other)
            || panel(other, self)
            || (browsing(self) && browsing(other))
            || (palette(self) && palette(other))
    }
}

use BindingId as I;

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
/// match themselves (dialogs, the palette, lists, the commit box, Esc in `ui/central.rs`, the
/// rebase panel's keys in `ui/rebase/keys.rs`), except Go to tab 1 to 8, matched with the
/// rebindable rows. Change a fixed key there, change it here.
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
    b(I::RewordCommit, G::History, "Reword the selected commit", &[C::plain(K::F(2))], W::Graph),
    b(
        I::SquashCommit,
        G::History,
        "Squash the selected commit into its parent (or the selected commits together)",
        &[C::alt(K::Letter('S'))],
        W::Graph,
    ),
    b(I::DropCommit, G::History, "Drop the selected commits", &[C::plain(K::Delete), C::cmd(K::Backspace)], W::Graph),
    b(I::RebaseContinue, G::History, "Continue the rebase", &[C::alt(K::Letter('C'))], W::NotTyping),
    b(I::RebaseSkip, G::History, "Skip the commit and continue the rebase", &[C::alt(K::Letter('K'))], W::NotTyping),
    fixed(I::RebaseStepSelect, G::History, "Select the step above or below", &[C::plain(K::Up), C::plain(K::Down)], W::RebasePanel),
    fixed(I::RebaseStepMove, G::History, "Move the selected step up or down", &[C::alt(K::Up), C::alt(K::Down)], W::RebasePanel),
    fixed(I::RebasePick, G::History, "Set the selected commit to pick", &[C::plain(K::Letter('P'))], W::RebasePanel),
    fixed(I::RebaseReword, G::History, "Set the selected commit to reword", &[C::plain(K::Letter('R'))], W::RebasePanel),
    fixed(I::RebaseSquash, G::History, "Set the selected commit to squash", &[C::plain(K::Letter('S'))], W::RebasePanel),
    fixed(I::RebaseFixup, G::History, "Set the selected commit to fixup", &[C::plain(K::Letter('F'))], W::RebasePanel),
    fixed(I::RebaseDrop, G::History, "Set the selected commit to drop", &[C::plain(K::Letter('D'))], W::RebasePanel),
    fixed(I::RebaseEdit, G::History, "Set the selected commit to edit", &[C::plain(K::Letter('E'))], W::RebasePanel),
    fixed(I::RebaseAddExec, G::History, "Add an exec step after the selected step", &[C::plain(K::Letter('X'))], W::RebasePanel),
    fixed(I::RebaseAddBreak, G::History, "Add a break step after the selected step", &[C::plain(K::Letter('B'))], W::RebasePanel),
    fixed(
        I::RebaseRemoveStep,
        G::History,
        "Remove the selected exec or break step",
        &[C::plain(K::Delete), C::plain(K::Backspace)],
        W::RebasePanel,
    ),
    fixed(I::RebaseEditStep, G::History, "Edit the selected commit's message or step's command", &[C::plain(K::Enter)], W::RebasePanel),
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
