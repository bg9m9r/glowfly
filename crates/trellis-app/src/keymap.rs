//! The keyboard binding table: every key Trellis answers to, grouped, labelled, and with the
//! place it works in. The shortcut sheet, the palette's hints and button tooltips read it, and
//! the newer keys (`ui/shortcuts.rs`) are matched through it. No egui types here.

use std::sync::LazyLock;

/// Modifiers of a chord. `command` is Ctrl, or Cmd on macOS; `ctrl` is always Ctrl (Ctrl+Tab).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Mods {
    pub command: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyName {
    Letter(char),
    Digit(u8),
    /// Any digit from the first to the second (one row in the sheet, never matched).
    DigitRange(u8, u8),
    F(u8),
    Enter,
    Escape,
    Tab,
    Backspace,
    Up,
    Down,
    Comma,
    Slash,
    Backtick,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Chord {
    pub mods: Mods,
    pub key: KeyName,
}

const NONE: Mods = Mods { command: false, ctrl: false, shift: false, alt: false };

impl Chord {
    pub const fn plain(key: KeyName) -> Self {
        Self { mods: NONE, key }
    }
    pub const fn shift(key: KeyName) -> Self {
        Self { mods: Mods { shift: true, ..NONE }, key }
    }
    pub const fn alt(key: KeyName) -> Self {
        Self { mods: Mods { alt: true, ..NONE }, key }
    }
    pub const fn cmd(key: KeyName) -> Self {
        Self { mods: Mods { command: true, ..NONE }, key }
    }
    pub const fn cmd_shift(key: KeyName) -> Self {
        Self { mods: Mods { command: true, shift: true, ..NONE }, key }
    }
    pub const fn ctrl(key: KeyName) -> Self {
        Self { mods: Mods { ctrl: true, ..NONE }, key }
    }
    pub const fn ctrl_shift(key: KeyName) -> Self {
        Self { mods: Mods { ctrl: true, shift: true, ..NONE }, key }
    }

    /// The modifier and key names in display order, e.g. ["Ctrl", "Shift", "S"].
    pub fn parts(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.mods.command {
            out.push(if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" });
        }
        if self.mods.ctrl {
            out.push("Ctrl");
        }
        if self.mods.shift {
            out.push("Shift");
        }
        if self.mods.alt {
            out.push("Alt");
        }
        out.push(key_name(self.key));
        out
    }

    /// The chord as text, e.g. "Ctrl+Shift+S".
    pub fn text(&self) -> String {
        self.parts().join("+")
    }
}

fn key_name(k: KeyName) -> &'static str {
    const LETTERS: [&str; 26] =
        ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z"];
    const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
    const FKEYS: [&str; 12] = ["F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12"];
    match k {
        KeyName::Letter(c) => LETTERS[(c.to_ascii_uppercase() as usize).saturating_sub('A' as usize).min(25)],
        KeyName::Digit(n) => DIGITS[(n as usize).min(9)],
        // The only range in the table; a new one needs its own name here.
        KeyName::DigitRange(..) => "1–8",
        KeyName::F(n) => FKEYS[(n as usize).clamp(1, 12) - 1],
        KeyName::Enter => "Enter",
        KeyName::Escape => "Esc",
        KeyName::Tab => "Tab",
        KeyName::Backspace => "Backspace",
        KeyName::Up => "Up",
        KeyName::Down => "Down",
        KeyName::Comma => ",",
        KeyName::Slash => "/",
        KeyName::Backtick => "`",
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Staging,
    Navigation,
    Diff,
    Tabs,
    Palette,
    General,
}

/// The groups in the order the sheet shows them.
pub const GROUPS: [Group; 6] = [Group::Staging, Group::Navigation, Group::Diff, Group::Tabs, Group::Palette, Group::General];

impl Group {
    pub fn title(self) -> &'static str {
        match self {
            Group::Staging => "Staging and committing",
            Group::Navigation => "Navigation",
            Group::Diff => "Diff and conflicts",
            Group::Tabs => "Tabs",
            Group::Palette => "Command palette",
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
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    Prefs,
    Refresh,
    Undo,
    Redo,
    DialogSubmit,
    ToggleTerminal,
}

pub struct Binding {
    pub id: BindingId,
    pub group: Group,
    pub label: &'static str,
    /// Alternatives; the first is the one hints show.
    pub chords: &'static [Chord],
    pub when: When,
}

/// What the keys matched in `ui/shortcuts.rs` ask `update` to do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyAction {
    Stage,
    Unstage,
    StageAll,
    UnstageAll,
    FocusMessage,
    ToggleSheet,
}

const fn b(id: BindingId, group: Group, label: &'static str, chords: &'static [Chord], when: When) -> Binding {
    Binding { id, group, label, chords, when }
}

use BindingId as I;
use Chord as C;
use Group as G;
use KeyName as K;
use When as W;

/// Every binding, in display order within its group. Only Stage, Unstage, StageAll,
/// UnstageAll, FocusMessage and Sheet are matched from this table (`ui/shortcuts.rs`); the
/// others document handlers that match their own keys: `ui/mod.rs` (tabs, undo/redo, F5,
/// Ctrl+F, Ctrl+comma, F7/Alt+arrows, Esc), `ui/commit_box.rs`, `ui/graph/select.rs`,
/// `ui/history.rs`, `ui/palette.rs`, `ui/terminal/` and the dialogs. Change a key there, change it here.
pub const BINDINGS: &[Binding] = &[
    b(I::Stage, G::Staging, "Stage the file or the selected lines", &[C::plain(K::Letter('S'))], W::WorkingDiff),
    b(I::Unstage, G::Staging, "Unstage the file or the selected lines", &[C::plain(K::Letter('U'))], W::WorkingDiff),
    b(I::StageAll, G::Staging, "Stage all files", &[C::cmd_shift(K::Letter('S'))], W::NotTyping),
    b(I::UnstageAll, G::Staging, "Unstage all files", &[C::cmd_shift(K::Letter('U'))], W::NotTyping),
    b(I::FocusMessage, G::Staging, "Go to the commit message", &[C::cmd_shift(K::Letter('M'))], W::Anywhere),
    b(I::Commit, G::Staging, "Commit", &[C::cmd(K::Enter)], W::Message),
    b(I::StageAllAndCommit, G::Staging, "Stage all files and commit", &[C::cmd_shift(K::Enter)], W::Message),
    b(I::SelectRow, G::Navigation, "Select the row above or below", &[C::plain(K::Up), C::plain(K::Down)], W::Lists),
    b(I::Search, G::Navigation, "Show or hide commit search", &[C::cmd(K::Letter('F'))], W::Anywhere),
    b(I::SearchNext, G::Navigation, "Next search match", &[C::plain(K::Enter)], W::Search),
    b(I::NextChange, G::Diff, "Next change or conflict", &[C::plain(K::F(7)), C::alt(K::Down)], W::DiffOrConflict),
    b(I::PrevChange, G::Diff, "Previous change or conflict", &[C::shift(K::F(7)), C::alt(K::Up)], W::DiffOrConflict),
    b(
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
    b(I::GoToTab, G::Tabs, "Go to tab 1 to 8", &[C::cmd(K::DigitRange(1, 8))], W::Anywhere),
    b(I::LastTab, G::Tabs, "Go to the last tab", &[C::cmd(K::Digit(9))], W::Anywhere),
    b(I::Palette, G::Palette, "Open or close the command palette", &[C::cmd(K::Letter('P')), C::cmd_shift(K::Letter('P'))], W::Anywhere),
    b(I::PaletteMove, G::Palette, "Move the selection", &[C::plain(K::Up), C::plain(K::Down)], W::InPalette),
    b(I::PaletteRun, G::Palette, "Run the selected command", &[C::plain(K::Enter)], W::InPalette),
    b(
        I::PaletteBack,
        G::Palette,
        "Back to the commands (Backspace with an empty field)",
        &[C::plain(K::Backspace), C::plain(K::Escape)],
        W::PaletteStep,
    ),
    b(I::PaletteClose, G::Palette, "Close the palette", &[C::plain(K::Escape)], W::InPalette),
    b(I::Sheet, G::General, "Keyboard shortcuts", &[C::cmd(K::Slash)], W::Anywhere),
    b(I::Prefs, G::General, "Preferences", &[C::cmd(K::Comma)], W::Anywhere),
    b(I::Refresh, G::General, "Refresh", &[C::plain(K::F(5))], W::Anywhere),
    b(I::ToggleTerminal, G::General, "Show or hide the terminal", &[C::ctrl(K::Backtick)], W::Anywhere),
    b(I::Undo, G::General, "Undo", &[C::cmd(K::Letter('Z'))], W::NotTyping),
    b(I::Redo, G::General, "Redo", &[C::cmd(K::Letter('Y')), C::cmd_shift(K::Letter('Z'))], W::NotTyping),
    b(I::DialogSubmit, G::General, "Confirm a dialog's name field", &[C::plain(K::Enter)], W::Dialog),
];

/// The binding `id`. Every id has a row in `BINDINGS`.
pub fn binding(id: BindingId) -> &'static Binding {
    BINDINGS.iter().find(|b| b.id == id).expect("every BindingId has a row in BINDINGS")
}

static HINTS: LazyLock<Vec<(BindingId, String)>> = LazyLock::new(|| BINDINGS.iter().map(|b| (b.id, b.chords[0].text())).collect());

/// The first chord of `id` as text, e.g. "Ctrl+Shift+S", for hints and tooltips.
pub fn hint(id: BindingId) -> &'static str {
    HINTS.iter().find(|(i, _)| *i == id).map(|(_, t)| t.as_str()).expect("every BindingId has a row in BINDINGS")
}

/// Every chord of `b` as text, joined with " or ".
pub fn keys_text(b: &Binding) -> String {
    b.chords.iter().map(Chord::text).collect::<Vec<_>>().join(" or ")
}

/// The bindings matching `query`, by group in `GROUPS` order, empty groups left out. Every
/// whitespace-separated term must appear (ignoring case) in the label, the note, the group's
/// title or the key text.
pub fn filter(query: &str) -> Vec<(Group, Vec<&'static Binding>)> {
    let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let matches = |b: &Binding| {
        let hay = format!("{} {} {} {}", b.label, b.when.note().unwrap_or(""), b.group.title(), keys_text(b)).to_lowercase();
        terms.iter().all(|t| hay.contains(t.as_str()))
    };
    GROUPS
        .iter()
        .map(|&g| (g, BINDINGS.iter().filter(|b| b.group == g && matches(b)).collect::<Vec<_>>()))
        .filter(|(_, rows)| !rows.is_empty())
        .collect()
}

#[cfg(test)]
mod tests;
