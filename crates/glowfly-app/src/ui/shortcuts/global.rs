//! The app's shortcuts, matched through the effective keymap: each frame's key presses are
//! turned into chords, looked up, and handled when what is open lets the binding through
//! (Preferences, the palette, a dialog, a credential prompt, a focused text field). A focused
//! terminal keeps the unshifted keys of the tab, palette, search and refresh actions for its
//! shell (`terminal_keeps`). The Repository commands keys run palette commands: a held key runs
//! its command once (repeats are swallowed), and a focused terminal keeps every press of theirs
//! that would reach its shell. Then the keys of custom commands (`custom_keys.rs`), only while
//! nothing covers the main window, then the menus' Alt+letter mnemonics (`ui/menu_bar/keys.rs`),
//! which the view drawing the menus opens. Quit works anywhere but under a credential prompt.
//! Presses no binding takes are left for the views; Esc goes on to the Esc chain in
//! `ui/central.rs`.

use super::chords::take_press;
use super::record;
use crate::custom_commands::{CustomCommandMsg, Target};
use crate::keymap::{BindingId, Chord, KeyAction, KeyName, runs_command};
use crate::msg::{
    AppMsg, BlameMsg, ConflictMsg, DiffMsg, HistoryMsg, JobMsg, LayoutMsg, Msg, PaletteMsg, PrefsMsg, RebaseMsg, RecentMsg, RepoMsg,
    SearchMsg, SelectionMsg, TabsMsg,
};
use crate::state::history::HistoryPane;
use crate::state::layout::Panel;
use crate::state::{AppState, Modal, PrefPage, Selection};
use crate::terminal::TerminalMsg;
use crate::ui::central;
use eframe::egui;
use git2::RepositoryState;
use glowfly_core::rebase::{QuickEdit, Resume};

/// Turns this frame's shortcut presses into messages (or view-state changes, for the search
/// bar). While Preferences > Keyboard records a key, it gets every key press instead.
pub(crate) fn input(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    if record::keys(ctx, st, msgs) {
        return;
    }
    let live = Live::now(ctx, st);
    while let Some((id, chord, repeat)) = ctx.input_mut(|i| take_press(i, &st.keymap, |id, shell| allowed(id, shell, &live))) {
        // A held key would queue the same fetch or push again and again.
        if repeat && runs_command(id) {
            continue;
        }
        fire(id, chord, st, msgs);
    }
    // Custom commands' keys, only over the main window itself.
    if !(live.prefs || live.askpass || live.palette || live.dialog || live.sheet || live.typing || live.terminal) {
        while let Some(command) = ctx.input_mut(|i| super::custom_keys::take(i, &st.custom_commands.keys)) {
            msgs.push(Msg::CustomCommand(CustomCommandMsg::Run { command, at: Target::Palette, confirmed: false }));
        }
    }
    // A menu's Alt+letter, after the bindings and custom keys (they win), while a menu bar is
    // reachable and nothing covers the window or holds the keyboard.
    st.view.menu_keys.request = None;
    let bar = st.view.chrome.drawn() || st.settings.show_menu_bar;
    if st.view.chrome.mnemonics() && bar && !(live.prefs || live.askpass || live.palette || live.dialog || live.sheet || live.terminal) {
        let marks = crate::ui::menu_bar::marks(st);
        st.view.menu_keys.request = ctx.input_mut(|i| crate::ui::menu_bar::take(i, &marks));
    }
    central::escape_key(ctx, st, msgs);
}

/// What is open this frame, for deciding which bindings apply.
struct Live {
    prefs: bool,
    palette: bool,
    askpass: bool,
    /// The shortcut sheet.
    sheet: bool,
    /// A dialog other than the sheet.
    dialog: bool,
    /// A text field has the keyboard (a focused WIP file list isn't typing).
    typing: bool,
    /// A working-tree diff is open, or a working-tree file is opening.
    working_diff: bool,
    /// A commit or comparison diff is open, with no conflict or blame view over it.
    commit_diff: bool,
    /// A diff or the conflict view is open (and no blame view over it).
    stepping: bool,
    /// A diff is open, with no conflict or blame view over it.
    diff: bool,
    /// A loaded blame view is open.
    blame: bool,
    /// The commit graph is the central view.
    graph: bool,
    /// The terminal has the keyboard.
    terminal: bool,
    /// A rebase is in progress.
    rebasing: bool,
}

impl Live {
    fn now(ctx: &egui::Context, st: &AppState) -> Self {
        let sheet = matches!(st.modal, Some(Modal::Shortcuts { .. }));
        Self {
            prefs: st.prefs.is_some(),
            palette: st.palette.is_some(),
            askpass: st.askpass.showing(),
            sheet,
            // The welcome counts as a dialog, except while Preferences hides it.
            dialog: (st.modal.is_some() && !sheet) || (st.onboarding.welcome.is_some() && st.prefs.is_none()),
            typing: ctx.wants_keyboard_input() && crate::ui::wip_keys::focused_list(ctx).is_none(),
            working_diff: st
                .view
                .diff_opening
                .as_ref()
                .or(st.session.diff.view.as_ref().map(|(t, _)| t))
                .is_some_and(|t| t.is_working_tree()),
            commit_diff: st.session.blame.view.is_none()
                && st.session.conflict.view.is_none()
                && st.session.diff.view.as_ref().is_some_and(|(t, _)| t.reverts_hunks()),
            stepping: st.session.blame.view.is_none() && (st.session.conflict.view.is_some() || st.session.diff.view.is_some()),
            diff: st.session.blame.view.is_none() && st.session.conflict.view.is_none() && st.session.diff.view.is_some(),
            blame: st.session.blame.view.as_ref().is_some_and(|v| v.data.is_some()),
            graph: st.graph_shown(),
            terminal: st.terminal.focused,
            rebasing: st.snap.as_ref().is_some_and(|s| {
                use RepositoryState::*;
                matches!(s.state, Rebase | RebaseInteractive | RebaseMerge | ApplyMailboxOrRebase)
            }),
        }
    }
}

/// The actions whose keys a focused terminal keeps for its shell when pressed without Shift,
/// Alt or Cmd (Ctrl+W, Ctrl+P, Ctrl+Q, F5…); their shifted keys still act.
fn terminal_keeps(id: BindingId) -> bool {
    use BindingId as I;
    matches!(id, I::NewTab | I::CloseTab | I::NextTab | I::GoToTab | I::LastTab | I::Palette | I::Search | I::Refresh | I::Quit)
}

/// Whether `id` applies with what is open; `shell` says whether the press would reach a focused
/// terminal's shell. Nothing reaches the main window's actions under Preferences, a credential
/// prompt, the palette or a dialog, except where noted; text fields keep plain keys and their
/// own undo.
fn allowed(id: BindingId, shell: bool, l: &Live) -> bool {
    use BindingId as I;
    if l.terminal && shell && terminal_keeps(id) {
        return false;
    }
    let covered = l.prefs || l.askpass || l.palette || l.dialog;
    match id {
        I::Palette => !l.prefs && !l.dialog && !l.sheet && !l.askpass,
        // Closes the sheet too.
        I::Sheet => !covered,
        I::FocusMessage => !covered && !l.sheet,
        I::StageAll | I::UnstageAll => !covered && !l.sheet && !l.typing,
        I::Stage | I::Unstage => !covered && !l.sheet && !l.typing && l.working_diff,
        I::RevertHunk => !covered && !l.sheet && !l.typing && l.commit_diff,
        I::ToggleTerminal | I::ToggleLeftPanel | I::ToggleDetailsPanel | I::ToggleToolbar => !covered && !l.sheet,
        // Zoom and full screen work over everything (recording a key takes every press before this).
        I::ZoomIn | I::ZoomOut | I::ZoomReset | I::ToggleFullscreen => true,
        I::NewTab | I::CloseTab | I::ReopenTab | I::NextTab | I::PrevTab | I::LastTab | I::MoveTabLeft | I::MoveTabRight | I::GoToTab => {
            !l.prefs && !l.palette
        }
        I::Undo | I::Redo => !covered && !l.sheet && !l.typing,
        I::NextChange | I::PrevChange => !covered && !l.sheet && !l.typing && l.stepping,
        I::OpenFold | I::CloseFold => !covered && !l.sheet && !l.typing && l.diff,
        I::BlameNextRun | I::BlamePrevRun => !covered && !l.sheet && !l.typing && l.blame,
        I::SelectAllCommits | I::SelectFocusedCommit => !covered && !l.sheet && !l.typing && l.graph && !l.terminal,
        // Not while typing: AltGr is Ctrl+Alt on Windows, and AltGr+digit types characters.
        I::OpenFavorite1
        | I::OpenFavorite2
        | I::OpenFavorite3
        | I::OpenFavorite4
        | I::OpenFavorite5
        | I::OpenFavorite6
        | I::OpenFavorite7
        | I::OpenFavorite8
        | I::OpenFavorite9 => !covered && !l.sheet && !l.typing,
        I::RewordCommit | I::SquashCommit | I::DropCommit => !covered && !l.sheet && !l.typing && l.graph,
        I::RebaseContinue | I::RebaseSkip => !covered && !l.sheet && !l.typing && l.rebasing,
        // Palette commands; a focused terminal keeps every key of theirs that reaches its shell.
        I::Fetch
        | I::Pull
        | I::Push
        | I::CreateBranch
        | I::CreateTag
        | I::CreateAnnotatedTag
        | I::StashChanges
        | I::StashWithMessage
        | I::PopStash
        | I::FileHistory
        | I::BlameFile
        | I::OpenJournal
        | I::OpenTerminal
        | I::ShowRepoFolder => !covered && !l.sheet && !l.typing && !(l.terminal && shell),
        I::Refresh => true,
        I::Search | I::SearchFilter => !l.palette && !l.prefs,
        I::Prefs => !l.prefs && !l.palette,
        I::Quit => !l.askpass,
        I::Commit
        | I::StageAllAndCommit
        | I::SelectRow
        | I::ExtendSelection
        | I::CloseView
        | I::PaletteMove
        | I::PaletteRun
        | I::PaletteBack
        | I::PaletteClose
        | I::SearchNext
        | I::DialogSubmit
        | I::SendPrText
        | I::RebaseStepSelect
        | I::RebaseStepMove
        | I::RebasePick
        | I::RebaseReword
        | I::RebaseSquash
        | I::RebaseFixup
        | I::RebaseDrop
        | I::RebaseEdit
        | I::RebaseAddExec
        | I::RebaseAddBreak
        | I::RebaseRemoveStep
        | I::RebaseEditStep => false,
    }
}

/// Does what `id` does; `chord` is the key pressed (its digit picks the tab for Go to tab).
fn fire(id: BindingId, chord: Chord, st: &mut AppState, msgs: &mut Vec<Msg>) {
    use BindingId as I;
    // A history's File pane has no diff on screen: a folder history steps to the entry's next
    // or previous file instead.
    let file_pane =
        |st: &AppState| st.session.blame.view.is_none() && st.session.history.view.as_ref().is_some_and(|v| v.pane == HistoryPane::File);
    let step = |d: isize, st: &AppState| {
        if st.session.conflict.view.is_some() {
            Msg::Conflict(ConflictMsg::ConflictStep(d))
        } else if file_pane(st) {
            Msg::History(HistoryMsg::StepHistoryFile(d))
        } else {
            Msg::Diff(DiffMsg::JumpToChange(d))
        }
    };
    let msg = match id {
        I::Stage => Msg::Shortcut(KeyAction::Stage),
        I::Unstage => Msg::Shortcut(KeyAction::Unstage),
        I::RevertHunk => Msg::Shortcut(KeyAction::RevertHunk),
        I::StageAll => Msg::Shortcut(KeyAction::StageAll),
        I::UnstageAll => Msg::Shortcut(KeyAction::UnstageAll),
        I::FocusMessage => Msg::Shortcut(KeyAction::FocusMessage),
        I::Sheet => Msg::Shortcut(KeyAction::ToggleSheet),
        I::Palette => Msg::Palette(PaletteMsg::TogglePalette),
        I::Prefs => Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)),
        I::Quit => Msg::App(AppMsg::Quit),
        I::Refresh => Msg::Repo(RepoMsg::Refresh),
        I::Undo => Msg::Job(JobMsg::Undo),
        I::Redo => Msg::Job(JobMsg::Redo),
        I::ToggleTerminal => Msg::Terminal(TerminalMsg::Toggle),
        I::ZoomIn => Msg::Layout(LayoutMsg::ZoomIn),
        I::ZoomOut => Msg::Layout(LayoutMsg::ZoomOut),
        I::ZoomReset => Msg::Layout(LayoutMsg::ZoomReset),
        I::ToggleLeftPanel => Msg::Layout(LayoutMsg::TogglePanel(Panel::Left)),
        I::ToggleDetailsPanel => Msg::Layout(LayoutMsg::TogglePanel(Panel::Details)),
        I::ToggleToolbar => Msg::Layout(LayoutMsg::TogglePanel(Panel::Toolbar)),
        I::ToggleFullscreen => Msg::Layout(LayoutMsg::ToggleFullscreen),
        I::NewTab => Msg::Tabs(TabsMsg::NewTab),
        I::CloseTab => Msg::Tabs(TabsMsg::CloseTab(usize::MAX)),
        I::ReopenTab => Msg::Tabs(TabsMsg::ReopenClosedTab),
        I::NextTab => Msg::Tabs(TabsMsg::CycleTab(1)),
        I::PrevTab => Msg::Tabs(TabsMsg::CycleTab(-1)),
        I::LastTab => Msg::Tabs(TabsMsg::SelectTab(st.tabs.len().saturating_sub(1))),
        I::MoveTabLeft => Msg::Tabs(TabsMsg::MoveActiveTab(-1)),
        I::MoveTabRight => Msg::Tabs(TabsMsg::MoveActiveTab(1)),
        I::GoToTab => match chord.key {
            KeyName::Digit(d) => Msg::Tabs(TabsMsg::SelectTab(usize::from(d).saturating_sub(1))),
            _ => return,
        },
        I::OpenFold => Msg::Diff(DiffMsg::OpenFoldAtTop),
        I::CloseFold => Msg::Diff(DiffMsg::CloseFoldAtTop),
        I::NextChange => step(1, st),
        I::PrevChange => step(-1, st),
        I::BlameNextRun => Msg::Blame(BlameMsg::StepRun(1)),
        I::BlamePrevRun => Msg::Blame(BlameMsg::StepRun(-1)),
        I::SelectAllCommits => Msg::Selection(SelectionMsg::SelectAll),
        I::SelectFocusedCommit => Msg::Selection(SelectionMsg::BackToFocus),
        I::OpenFavorite1
        | I::OpenFavorite2
        | I::OpenFavorite3
        | I::OpenFavorite4
        | I::OpenFavorite5
        | I::OpenFavorite6
        | I::OpenFavorite7
        | I::OpenFavorite8
        | I::OpenFavorite9 => match id.favorite_slot() {
            Some(n) => Msg::Recent(RecentMsg::OpenFavorite(n)),
            None => return,
        },
        // The same messages as the graph's menu, so blocked edits explain themselves.
        I::RewordCommit => match &st.selection {
            Selection::Commit(o) => Msg::Rebase(RebaseMsg::Edit(QuickEdit::Reword(*o))),
            _ => return,
        },
        I::SquashCommit => match &st.selection {
            Selection::Commit(o) => Msg::Rebase(RebaseMsg::Edit(QuickEdit::SquashIntoParent(*o))),
            Selection::Commits(m) => Msg::Rebase(RebaseMsg::Edit(QuickEdit::Squash(m.oids.clone()))),
            _ => return,
        },
        I::DropCommit => match &st.selection {
            Selection::Commit(o) => Msg::Rebase(RebaseMsg::Edit(QuickEdit::Drop(vec![*o]))),
            Selection::Commits(m) => Msg::Rebase(RebaseMsg::Edit(QuickEdit::Drop(m.oids.clone()))),
            _ => return,
        },
        // The palette's own entry runs, only while the palette would list it.
        I::Fetch
        | I::Pull
        | I::Push
        | I::CreateBranch
        | I::CreateTag
        | I::CreateAnnotatedTag
        | I::StashChanges
        | I::StashWithMessage
        | I::PopStash
        | I::FileHistory
        | I::BlameFile
        | I::OpenJournal
        | I::OpenTerminal
        | I::ShowRepoFolder => match crate::state::palette_bindings::command_for(id) {
            Some(cmd) => Msg::Palette(PaletteMsg::RunBoundCommand(cmd)),
            None => return,
        },
        I::RebaseContinue => Msg::Rebase(RebaseMsg::Resume(Resume::Continue)),
        I::RebaseSkip => Msg::Rebase(RebaseMsg::Resume(Resume::Skip)),
        // In the blame view it finds in the file.
        I::Search if st.session.blame.view.is_some() => Msg::Blame(BlameMsg::FocusFind),
        // In a history's File pane it finds in the file shown.
        I::Search if file_pane(st) => Msg::History(HistoryMsg::FocusFileFind),
        // The search bar's visibility is view state.
        I::Search => {
            st.view.show_search = !st.view.show_search;
            return;
        }
        I::SearchFilter => {
            st.view.show_search = true;
            Msg::Search(SearchMsg::ToggleFilter)
        }
        I::Commit
        | I::StageAllAndCommit
        | I::SelectRow
        | I::ExtendSelection
        | I::CloseView
        | I::PaletteMove
        | I::PaletteRun
        | I::PaletteBack
        | I::PaletteClose
        | I::SearchNext
        | I::DialogSubmit
        | I::SendPrText
        | I::RebaseStepSelect
        | I::RebaseStepMove
        | I::RebasePick
        | I::RebaseReword
        | I::RebaseSquash
        | I::RebaseFixup
        | I::RebaseDrop
        | I::RebaseEdit
        | I::RebaseAddExec
        | I::RebaseAddBreak
        | I::RebaseRemoveStep
        | I::RebaseEditStep => return,
    };
    msgs.push(msg);
}
