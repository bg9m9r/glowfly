//! The app's shortcuts, matched through the effective keymap: each frame's key presses are
//! turned into chords, looked up, and handled when what is open lets the binding through
//! (Preferences, the palette, a dialog, a credential prompt, a focused text field). A focused
//! terminal keeps the unshifted keys of the tab, palette, search and refresh actions for its
//! shell (`terminal_keeps`). The Repository commands keys run palette commands: a held key runs
//! its command once (repeats are swallowed), and a focused terminal keeps every press of theirs
//! that would reach its shell. Then the keys of custom commands (`custom_keys.rs`), only while
//! nothing covers the main window, then the menus' Alt+letter mnemonics (`ui/menu_bar/keys.rs`),
//! which the view drawing the menus opens. Quit works anywhere but under a credential prompt.
//! A focused file list takes its keys first (`ui/wip_keys.rs`, `ui/file_list_keys.rs`).
//! Presses no binding takes are left for the views; Esc goes on to the Esc chain in
//! `ui/central.rs`. While a menu is open, its keys (those without Ctrl, Alt or Cmd) are set
//! aside for it first (`ui/menu_keys`), so no binding or view behind it sees them.

use super::chords::take_press;
use super::live::{Live, allowed};
use super::record;
use crate::custom_commands::{CustomCommandMsg, Target};
use crate::keymap::{BindingId, Chord, KeyAction, KeyName, runs_command};
use crate::msg::{
    AppMsg, BlameMsg, ConflictMsg, DiffMsg, HistoryMsg, JobMsg, LayoutMsg, Msg, PaletteMsg, PrefsMsg, RebaseMsg, RecentMsg, RepoMsg,
    SearchMsg, SelectionMsg, TabsMsg,
};
use crate::state::graph_rows::GraphKey;
use crate::state::history::HistoryPane;
use crate::state::layout::{Pane, Panel};
use crate::state::{AppState, PrefPage, Selection};
use crate::ui::central;
use eframe::egui;
use glowfly_core::rebase::{QuickEdit, Resume};
use glowfly_model::terminal::TerminalMsg;

/// Turns this frame's shortcut presses into messages (or view-state changes, for the search
/// bar). While Preferences > Keyboard records a key, it gets every key press instead.
pub fn input(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    // An open menu's keys are its own: no binding sees them.
    crate::ui::menu_keys::capture(ctx);
    if record::keys(ctx, st, msgs) {
        return;
    }
    let live = Live::now(ctx, st);
    // A focused file list takes its own keys before any binding.
    let covered = live.prefs || live.askpass || live.palette || live.dialog || live.sheet;
    crate::ui::wip_keys::input(ctx, covered, msgs);
    crate::ui::file_list_keys::input(ctx, covered, msgs);
    // Ctrl+C reaches egui as a Copy event: a binding of it (Copy commit IDs) gets it as a key,
    // while the graph takes the keys and no label text (a commit message) is selected.
    if copy_is_the_graphs(ctx) {
        ctx.input_mut(|i| super::chords::copy_as_key(i, &st.keymap, |id, shell| allowed(id, shell, &live)));
    }
    while let Some((id, chord, repeat)) = ctx.input_mut(|i| take_press(i, &st.keymap, |id, shell| allowed(id, shell, &live))) {
        // A held key would queue the same fetch or push again and again.
        if repeat && runs_command(id) {
            continue;
        }
        fire(id, chord, live.pane, st, msgs);
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

/// Does what `id` does; `chord` is the key pressed (its digit picks the tab for Go to tab), and
/// `pane` the pane with the keyboard focus (F6 goes on from it).
fn fire(id: BindingId, chord: Chord, pane: Option<Pane>, st: &mut AppState, msgs: &mut Vec<Msg>) {
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
        I::PrefsSave => Msg::Prefs(PrefsMsg::SavePage),
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
        I::FocusSidebar => Msg::Shortcut(KeyAction::FocusPane(Pane::Sidebar)),
        I::FocusCentral => Msg::Shortcut(KeyAction::FocusPane(Pane::Central)),
        I::FocusFiles => Msg::Shortcut(KeyAction::FocusPane(Pane::Files)),
        I::FocusDiff => Msg::Shortcut(KeyAction::FocusPane(Pane::Diff)),
        I::FocusCommitMessage => Msg::Shortcut(KeyAction::FocusPane(Pane::Message)),
        I::NextPane | I::PrevPane => {
            let step = if id == I::NextPane { 1 } else { -1 };
            Msg::Shortcut(KeyAction::FocusPane(Pane::cycle(pane, step, |p| crate::ui::pane_focus::drawn(st, p))))
        }
        I::CopyCommitIds => Msg::Selection(SelectionMsg::CopyIds),
        // The graph applies these with its rows.
        I::SelectHead | I::PrevRefCommit | I::NextRefCommit => {
            st.view.graph_key = Some(match id {
                I::SelectHead => GraphKey::Head,
                _ => GraphKey::Ref { up: id == I::PrevRefCommit },
            });
            return;
        }
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
        I::RewordCommit => match &st.session.selection {
            Selection::Commit(o) => Msg::Rebase(RebaseMsg::Edit(QuickEdit::Reword(*o))),
            _ => return,
        },
        I::SquashCommit => match &st.session.selection {
            Selection::Commit(o) => Msg::Rebase(RebaseMsg::Edit(QuickEdit::SquashIntoParent(*o))),
            Selection::Commits(m) => Msg::Rebase(RebaseMsg::Edit(QuickEdit::Squash(m.oids.clone()))),
            _ => return,
        },
        I::DropCommit => match &st.session.selection {
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
        | I::ShowRepoFolder
        | I::CherryPick
        | I::RevertCommit => match crate::state::palette_bindings::command_for(id) {
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
        | I::RebaseEditStep
        | I::PrefsRevert
        | I::WipStepFile
        | I::WipExtendFiles
        | I::WipEdgeFile
        | I::WipPageFile
        | I::WipToggleFiles
        | I::WipOpenFile
        | I::WipDiscardFile
        | I::WipOtherList
        | I::FileStepFile
        | I::FilePageFile
        | I::FileEdgeFile
        | I::GraphPageRow
        | I::GraphEdgeRow
        | I::OpenCommitDetails => return,
    };
    msgs.push(msg);
}

/// Whether this frame's Copy is the graph's: it has the keyboard focus or nothing has, and no
/// label text is selected (that selection copies its own text).
fn copy_is_the_graphs(ctx: &egui::Context) -> bool {
    crate::ui::pane_focus::graph_active(ctx) && !egui::text_selection::LabelSelectionState::load(ctx).has_selection()
}
